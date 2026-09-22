//! Records which Bevy plugin added which plugin, and renders the result.
//!
//! **Added is the only thing we can unambiguously check.** An edge `A -> B` means
//! `A`'s `build()` added `B`. Nothing about actual data flow is inferred or claimed —
//! Bevy's `World` is one flat, globally reachable namespace, so the graph shows
//! intent, not enforcement.
//!
//! Bevy keeps `App`'s plugin registry and build depth private, so there is no public
//! API that exposes the plugin tree. Recording is therefore explicit, twice over:
//! [`init_graph`](PluginGraphExt::init_graph) a world to record at all, then swap
//! `add_plugins` for [`add_owned`](PluginGraphExt::add_owned) at the call sites you want in
//! the graph. Plugins added with plain `add_plugins` are simply absent, and on a
//! world that was never initialized `add_owned` degrades to plain `add_plugins`.
//!
//! One graph corresponds to one Bevy `World`. An app and each of its sub-apps record
//! separately and dump separately, each to a path of your choosing.
//!
//! Dumping is just as explicit. `build()` runs synchronously inside each add, so the
//! graph is complete as soon as the last add in `main` returns — reach it with
//! [`graph`](PluginGraphExt::graph) and write it before (or instead of) `App::run()`.
//! When to trigger a dump — a CLI flag, an environment variable, a dedicated binary
//! that never calls `run()` — is the caller's policy, not the crate's.
//!
//! ```
//! use bevy_app::{App, Plugin};
//! use bevy_plugin_graph::PluginGraphExt;
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
//! app.init_graph("Main");
//! app.add_owned(CombatPlugin);
//!
//! println!("{}", app.graph().unwrap().to_mermaid());
//! ```

mod graph;
mod render;

pub use graph::{NodeId, PluginGraph, PluginNode, ROOT};
pub use render::Format;

use bevy_app::{App, Plugin, SubApp};
use bevy_ecs::world::World;
use std::path::Path;

/// The crate's API, as an extension trait on [`App`] and [`SubApp`].
///
/// One trait rather than two (or free functions) because everything here is used
/// together, and Bevy provides no shared trait over `App` and `SubApp` to hang it
/// on. Holding only a bare [`World`]? The graph is an ordinary public resource:
/// `world.get_resource::<PluginGraph>()`.
pub trait PluginGraphExt {
    /// Add `plugin`, recording an edge from whatever is currently building —
    /// provided [`init_graph`](PluginGraphExt::init_graph) has created a graph in
    /// this world first. On an uninitialized world this is exactly `add_plugins`
    /// with a single plugin.
    ///
    /// Because `build()` runs synchronously inside this call, nesting is captured
    /// without traits, macros or `unsafe`.
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self;

    /// Start recording in this world and name the graph's root.
    ///
    /// Recording is opt-in: [`add_owned`](PluginGraphExt::add_owned) only records
    /// into a graph this call created, so call it **before** the adds you want
    /// recorded — adds on an uninitialized world behave exactly like `add_plugins`.
    /// Call it once per world: an app and each of its sub-apps record separately,
    /// and the name labels that world's root node.
    fn init_graph(&mut self, root: impl Into<String>);

    /// The graph recorded in this world, if
    /// [`init_graph`](PluginGraphExt::init_graph) has been called on it.
    fn graph(&self) -> Option<&PluginGraph>;

    /// Write this world's graph to `path` exactly as given, with the format
    /// inferred from the extension; see [`PluginGraph::dump`]. In a multi-world
    /// app, give each world its own path.
    ///
    /// Errors if the world has no graph, i.e.
    /// [`init_graph`](PluginGraphExt::init_graph) was never called on it.
    fn dump_graph(&self, path: impl AsRef<Path>) -> std::io::Result<()>;
}

impl PluginGraphExt for App {
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        begin::<P>(self.world_mut());
        self.add_plugins(plugin);
        end(self.world_mut());
        self
    }

    fn init_graph(&mut self, root: impl Into<String>) {
        init_graph_in(self.world_mut(), root);
    }

    fn graph(&self) -> Option<&PluginGraph> {
        self.world().get_resource::<PluginGraph>()
    }

    fn dump_graph(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        dump_graph_in(self.graph(), path.as_ref())
    }
}

impl PluginGraphExt for SubApp {
    fn add_owned<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        begin::<P>(self.world_mut());
        self.add_plugins(plugin);
        end(self.world_mut());
        self
    }

    fn init_graph(&mut self, root: impl Into<String>) {
        init_graph_in(self.world_mut(), root);
    }

    fn graph(&self) -> Option<&PluginGraph> {
        self.world().get_resource::<PluginGraph>()
    }

    fn dump_graph(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        dump_graph_in(self.graph(), path.as_ref())
    }
}

fn init_graph_in(world: &mut World, root: impl Into<String>) {
    world
        .get_resource_or_insert_with(PluginGraph::new)
        .set_root_name(root);
}

fn dump_graph_in(graph: Option<&PluginGraph>, path: &Path) -> std::io::Result<()> {
    let Some(graph) = graph else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no plugin graph in this world: init_graph was never called",
        ));
    };
    graph.dump(path)
}

// Recording is deliberately not `get_resource_or_insert_with`: only `init_graph`
// creates the graph, so an uninitialized world stays free of it and `add_owned`
// degrades to plain `add_plugins`.
fn begin<P: Plugin>(world: &mut World) {
    if let Some(mut graph) = world.get_resource_mut::<PluginGraph>() {
        graph.begin_owned::<P>();
    }
}

fn end(world: &mut World) {
    if let Some(mut graph) = world.get_resource_mut::<PluginGraph>() {
        graph.end_owned();
    }
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

    /// Write the graph to `path` in the given format.
    pub fn write(&self, path: impl AsRef<Path>, format: Format) -> std::io::Result<()> {
        std::fs::write(path, self.render(format))
    }

    /// [`write`](PluginGraph::write) with the format inferred from the path's
    /// extension: `.mmd`, `.mermaid` and `.md` render Mermaid, anything else JSON.
    pub fn dump(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        self.write(path, Format::from_path(path))
    }
}
