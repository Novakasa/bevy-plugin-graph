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
//! separately and dump separately, into files named after their own roots.
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
use std::path::{Path, PathBuf};

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
    /// Call it once per world: an app and each of its sub-apps record separately.
    /// The name labels the root node and selects the output file in
    /// [`dump_graph`](PluginGraphExt::dump_graph), so it must be distinct from any
    /// sub-app's.
    fn init_graph(&mut self, root: impl Into<String>);

    /// The graph recorded in this world, if
    /// [`init_graph`](PluginGraphExt::init_graph) has been called on it.
    fn graph(&self) -> Option<&PluginGraph>;

    /// Dump this world's graph to `base`, with the root name inserted into the file
    /// stem and the format inferred from the extension; see [`PluginGraph::dump`].
    ///
    /// Errors if the world has no graph, i.e.
    /// [`init_graph`](PluginGraphExt::init_graph) was never called on it.
    fn dump_graph(&self, base: impl AsRef<Path>) -> std::io::Result<()>;
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

    fn dump_graph(&self, base: impl AsRef<Path>) -> std::io::Result<()> {
        dump_graph_in(self.graph(), base.as_ref())
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

    fn dump_graph(&self, base: impl AsRef<Path>) -> std::io::Result<()> {
        dump_graph_in(self.graph(), base.as_ref())
    }
}

fn init_graph_in(world: &mut World, root: impl Into<String>) {
    world
        .get_resource_or_insert_with(PluginGraph::new)
        .set_root_name(root);
}

fn dump_graph_in(graph: Option<&PluginGraph>, base: &Path) -> std::io::Result<()> {
    let Some(graph) = graph else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no plugin graph in this world: init_graph was never called",
        ));
    };
    graph.dump(base)
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

    /// Write the graph to `path` exactly as given, with no name interpolation.
    pub fn write(&self, path: impl AsRef<Path>, format: Format) -> std::io::Result<()> {
        std::fs::write(path, self.render(format))
    }

    /// Write to `base` with the root name inserted into the file stem and the format
    /// inferred from the extension: a root named `Main` dumped to `graph.mmd` lands
    /// in `graph.Main.mmd`.
    ///
    /// Dump every world against the same base and the roots land side by side,
    /// never overwriting each other; see [`output_path`].
    pub fn dump(&self, base: impl AsRef<Path>) -> std::io::Result<()> {
        let path = output_path(base.as_ref(), self.root_name());
        self.write(&path, Format::from_path(&path))
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
