//! Records which Bevy plugin added which plugin, and renders the result.
//!
//! **Added is the only thing we can unambiguously check.** An edge `A -> B` means
//! `A`'s `build()` added `B`. Nothing about actual data flow is inferred or claimed —
//! Bevy's `World` is one flat, globally reachable namespace, so the graph shows
//! intent, not enforcement.
//!
//! Bevy keeps `App`'s plugin registry and build depth private, so there is no public
//! API that exposes the plugin tree. Recording is therefore explicit: swap
//! `add_plugins` for [`add_owned`](AddOwned::add_owned) at the call sites you want in
//! the graph. Plugins added with plain `add_plugins` are simply absent.
//!
//! ```
//! use bevy_app::{App, Plugin};
//! use bevy_plugin_graph::{AddOwned, PluginGraphPlugin};
//!
//! struct CombatPlugin;
//! impl Plugin for CombatPlugin {
//!     fn build(&self, app: &mut App) {
//!         app.add_owned(DamagePlugin);
//!     }
//! }
//!
//! struct DamagePlugin;
//! impl Plugin for DamagePlugin {
//!     fn build(&self, _app: &mut App) {}
//! }
//!
//! let mut app = App::new();
//! app.add_plugins(PluginGraphPlugin::new());
//! app.add_owned(CombatPlugin);
//!
//! println!("{}", bevy_plugin_graph::to_mermaid(&app));
//! ```

mod graph;
mod render;

pub use graph::{NodeId, PluginGraph, PluginNode, ROOT};
pub use render::Format;

use bevy_app::{App, Plugin};
use std::path::{Path, PathBuf};

/// Environment variable read by [`PluginGraphPlugin`] for the output path.
pub const ENV_OUTPUT: &str = "BEVY_PLUGIN_GRAPH";

/// Adds a plugin and records who added it.
pub trait AddOwned {
    /// Add `plugin`, recording an edge from whatever is currently building.
    ///
    /// Equivalent to [`App::add_plugins`] with a single plugin, plus the bookkeeping.
    /// Because `build()` runs synchronously inside this call, nesting is captured
    /// without traits, macros or `unsafe`.
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self;
}

impl AddOwned for App {
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(PluginGraph::new)
            .begin::<P>();

        self.add_plugins(plugin);

        if let Some(mut graph) = self.world_mut().get_resource_mut::<PluginGraph>() {
            graph.end();
        }
        self
    }
}

/// The recorded graph, if anything has been recorded yet.
pub fn graph(app: &App) -> Option<&PluginGraph> {
    app.world().get_resource::<PluginGraph>()
}

/// Render the app's graph as JSON. Apps with nothing recorded render as a lone root.
pub fn to_json(app: &App) -> String {
    with_graph(app, render::to_json)
}

/// Render the app's graph as a Mermaid `flowchart TD`.
pub fn to_mermaid(app: &App) -> String {
    with_graph(app, render::to_mermaid)
}

fn with_graph(app: &App, render: impl Fn(&PluginGraph) -> String) -> String {
    match graph(app) {
        Some(graph) => render(graph),
        None => render(&PluginGraph::new()),
    }
}

/// Write the app's graph to `path`.
///
/// This is the whole of the output path — [`PluginGraphPlugin`] is a thin wrapper
/// that calls it from `finish()`. Call it directly to dump a graph without ever
/// running the app.
pub fn dump(app: &App, path: impl AsRef<Path>, format: Format) -> std::io::Result<()> {
    let rendered = match format {
        Format::Json => to_json(app),
        Format::Mermaid => to_mermaid(app),
    };
    std::fs::write(path, rendered)
}

/// Writes the graph out once the app has finished building.
///
/// Does nothing unless an output path is set, either with [`PluginGraphPlugin::to`]
/// or via the [`ENV_OUTPUT`] environment variable.
#[derive(Debug, Default, Clone)]
pub struct PluginGraphPlugin {
    output: Option<PathBuf>,
    format: Option<Format>,
    exit_after_dump: bool,
}

impl PluginGraphPlugin {
    /// Take the output path from the [`ENV_OUTPUT`] environment variable.
    pub fn new() -> Self {
        Self::default()
    }

    /// Write to `path`, ignoring [`ENV_OUTPUT`]. The format is inferred from the
    /// extension unless [`PluginGraphPlugin::format`] overrides it.
    pub fn to(path: impl Into<PathBuf>) -> Self {
        Self {
            output: Some(path.into()),
            ..Self::default()
        }
    }

    /// Force a format instead of inferring one from the path.
    pub fn format(mut self, format: Format) -> Self {
        self.format = Some(format);
        self
    }

    /// Exit the process once the graph is written, before the runner starts.
    ///
    /// Iterating on the graph of a windowed app is otherwise a loop of opening and
    /// closing a window.
    pub fn exit_after_dump(mut self) -> Self {
        self.exit_after_dump = true;
        self
    }

    fn path(&self) -> Option<PathBuf> {
        self.output
            .clone()
            .or_else(|| std::env::var_os(ENV_OUTPUT).map(PathBuf::from))
    }
}

impl Plugin for PluginGraphPlugin {
    fn build(&self, app: &mut App) {
        // So that an app with no `add_owned` calls still emits a root-only graph
        // rather than nothing at all.
        app.world_mut()
            .get_resource_or_insert_with(PluginGraph::new);
    }

    fn finish(&self, app: &mut App) {
        let Some(path) = self.path() else {
            return;
        };
        let format = self.format.unwrap_or_else(|| Format::from_path(&path));

        let code = match dump(app, &path, format) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!(
                    "bevy_plugin_graph: could not write {}: {error}",
                    path.display()
                );
                1
            }
        };

        if self.exit_after_dump {
            std::process::exit(code);
        }
    }
}
