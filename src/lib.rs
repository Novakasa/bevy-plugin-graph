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
//! One graph corresponds to one Bevy `World`. An app and each of its sub-apps record
//! separately and dump separately, into files named after their own roots.
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
//! app.add_plugins(PluginGraphPlugin::new("Main"));
//! app.add_owned(CombatPlugin);
//!
//! println!("{}", bevy_plugin_graph::graph(&app).unwrap().to_mermaid());
//! ```

mod graph;
mod render;

pub use graph::{NodeId, PluginGraph, PluginNode, ROOT};
pub use render::Format;

use bevy_app::{App, Plugin, SubApp};
use bevy_ecs::world::World;
use std::path::{Path, PathBuf};

/// Environment variable read by [`PluginGraphPlugin`] for the output path.
pub const ENV_OUTPUT: &str = "BEVY_PLUGIN_GRAPH";

/// Adds a plugin and records who added it.
pub trait AddOwned {
    /// Add `plugin`, recording an edge from whatever is currently building.
    ///
    /// Equivalent to `add_plugins` with a single plugin, plus the bookkeeping.
    /// Because `build()` runs synchronously inside this call, nesting is captured
    /// without traits, macros or `unsafe`.
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self;
}

impl AddOwned for App {
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        begin::<P>(self.world_mut());
        self.add_plugins(plugin);
        end(self.world_mut());
        self
    }
}

impl AddOwned for SubApp {
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        begin::<P>(self.world_mut());
        self.add_plugins(plugin);
        end(self.world_mut());
        self
    }
}

fn begin<P: Plugin>(world: &mut World) {
    world
        .get_resource_or_insert_with(PluginGraph::new)
        .begin_owned::<P>();
}

fn end(world: &mut World) {
    if let Some(mut graph) = world.get_resource_mut::<PluginGraph>() {
        graph.end_owned();
    }
}

/// The graph recorded in an app's main world, if anything has been recorded yet.
pub fn graph(app: &App) -> Option<&PluginGraph> {
    graph_in(app.world())
}

/// The graph recorded in a specific world — use this to reach a sub-app's graph.
pub fn graph_in(world: &World) -> Option<&PluginGraph> {
    world.get_resource::<PluginGraph>()
}

impl PluginGraph {
    /// Render as JSON. This is the interface; every other format consumes it.
    pub fn to_json(&self) -> String {
        render::to_json(self)
    }

    /// Render as a Mermaid `flowchart TD`.
    pub fn to_mermaid(&self) -> String {
        render::to_mermaid(self)
    }

    pub fn render(&self, format: Format) -> String {
        match format {
            Format::Json => self.to_json(),
            Format::Mermaid => self.to_mermaid(),
        }
    }

    /// Write the graph to `path` exactly as given, with no name interpolation.
    pub fn write(&self, path: impl AsRef<Path>, format: Format) -> std::io::Result<()> {
        std::fs::write(path, self.render(format))
    }
}

/// Insert the root name into a base path's file stem: `graph.mmd` for root `RenderApp`
/// becomes `graph.RenderApp.mmd`.
///
/// Every root in an app lands in one directory, side by side, with the extension left
/// alone so format inference still works.
pub fn output_path(base: &Path, root: &str) -> PathBuf {
    let sanitized: String = root
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect();

    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("graph");

    let file = match base.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => format!("{stem}.{sanitized}.{ext}"),
        None => format!("{stem}.{sanitized}"),
    };

    base.with_file_name(file)
}

/// Writes one world's graph out once its plugins have finished building.
///
/// Add it to an [`App`] or to a [`SubApp`]; each names its own root and writes its own
/// file. Does nothing unless an output path is set, either with
/// [`PluginGraphPlugin::to`] or via the [`ENV_OUTPUT`] environment variable.
#[derive(Debug, Clone)]
pub struct PluginGraphPlugin {
    root: String,
    output: Option<PathBuf>,
    format: Option<Format>,
}

impl PluginGraphPlugin {
    /// Name this world's root. The name labels the root node and selects the output
    /// file, so it must be distinct from any sub-app's.
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            output: None,
            format: None,
        }
    }

    /// Write relative to `path`, ignoring [`ENV_OUTPUT`]. The root name is still
    /// inserted into the file stem; see [`output_path`].
    pub fn to(mut self, path: impl Into<PathBuf>) -> Self {
        self.output = Some(path.into());
        self
    }

    /// Force a format instead of inferring one from the path's extension.
    pub fn format(mut self, format: Format) -> Self {
        self.format = Some(format);
        self
    }

    fn base_path(&self) -> Option<PathBuf> {
        self.output
            .clone()
            .or_else(|| std::env::var_os(ENV_OUTPUT).map(PathBuf::from))
    }
}

impl Plugin for PluginGraphPlugin {
    fn build(&self, app: &mut App) {
        // Inserted on demand, so this works whether or not any `add_owned` call has
        // already run.
        let mut graph = app
            .world_mut()
            .get_resource_or_insert_with(PluginGraph::new);
        graph.set_root_name(self.root.clone());
    }

    fn finish(&self, app: &mut App) {
        let Some(base) = self.base_path() else {
            return;
        };
        let path = output_path(&base, &self.root);
        let format = self.format.unwrap_or_else(|| Format::from_path(&path));

        let Some(graph) = graph(app) else {
            return;
        };
        if let Err(error) = graph.write(&path, format) {
            eprintln!(
                "bevy_plugin_graph: could not write {}: {error}",
                path.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_the_root_into_the_stem() {
        assert_eq!(
            output_path(Path::new("out/graph.mmd"), "RenderApp"),
            PathBuf::from("out/graph.RenderApp.mmd")
        );
        assert_eq!(
            output_path(Path::new("graph"), "Main"),
            PathBuf::from("graph.Main")
        );
    }

    #[test]
    fn sanitizes_root_names_for_the_filesystem() {
        assert_eq!(
            output_path(Path::new("g.json"), "Render App/2"),
            PathBuf::from("g.Render_App_2.json")
        );
    }
}
