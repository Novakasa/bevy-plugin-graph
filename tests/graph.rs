//! Integration tests against a real `App` with a nested plugin structure.

use bevy_app::{App, AppLabel, Plugin, SubApp};
use bevy_plugin_graph::{AddOwned, Format, PluginGraph, PluginGraphPlugin, ROOT};

struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(CombatPlugin);
        app.add_owned(UiPlugin);
    }
}

struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(DamagePlugin);
        // Deliberately not recorded: plain `add_plugins` stays out of the graph.
        app.add_plugins(UnrecordedPlugin);
    }
}

struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, _app: &mut App) {}
}

struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, _app: &mut App) {}
}

struct UnrecordedPlugin;

impl Plugin for UnrecordedPlugin {
    fn build(&self, _app: &mut App) {}
}

mod deeper {
    use super::*;

    pub struct NestedPlugin;

    impl Plugin for NestedPlugin {
        fn build(&self, _app: &mut App) {}
    }
}

fn built_app() -> App {
    let mut app = App::new();
    app.add_owned(GamePlugin);
    app
}

fn graph_of(app: &App) -> &PluginGraph {
    bevy_plugin_graph::graph(app).expect("graph was recorded")
}

#[test]
fn records_the_nesting() {
    let app = built_app();
    let graph = graph_of(&app);

    let game = graph.find::<GamePlugin>().expect("GamePlugin recorded");
    let combat = graph.find::<CombatPlugin>().expect("CombatPlugin recorded");
    let damage = graph.find::<DamagePlugin>().expect("DamagePlugin recorded");
    let ui = graph.find::<UiPlugin>().expect("UiPlugin recorded");

    assert_eq!(game.parents, vec![ROOT]);
    assert_eq!(combat.parents, vec![game.id]);
    assert_eq!(damage.parents, vec![combat.id]);
    assert_eq!(ui.parents, vec![game.id]);
}

#[test]
fn unrecorded_plugins_are_absent() {
    let app = built_app();
    let graph = graph_of(&app);

    assert!(graph.find::<UnrecordedPlugin>().is_none());
    // root + Game + Combat + Damage + Ui
    assert_eq!(graph.nodes().len(), 5);
}

#[test]
fn an_app_with_no_recording_has_no_graph() {
    let app = App::new();
    assert!(bevy_plugin_graph::graph(&app).is_none());
}

#[test]
fn nodes_carry_short_names_crates_and_modules() {
    let mut app = App::new();
    app.add_owned(GamePlugin);
    app.add_owned(deeper::NestedPlugin);
    let graph = graph_of(&app);

    let combat = graph.find::<CombatPlugin>().unwrap();
    assert_eq!(combat.name, "CombatPlugin");
    assert!(combat.path.ends_with("CombatPlugin"));
    assert_eq!(combat.krate.as_deref(), Some("graph"));
    assert_eq!(combat.module.as_deref(), Some("graph"));

    let nested = graph.find::<deeper::NestedPlugin>().unwrap();
    assert_eq!(nested.module.as_deref(), Some("graph::deeper"));

    // The synthetic root is not defined anywhere.
    assert_eq!(graph.nodes()[ROOT.0].module, None);
}

#[test]
fn every_node_but_the_root_has_a_parent() {
    let app = built_app();
    let graph = graph_of(&app);

    assert_eq!(graph.edges().count(), graph.nodes().len() - 1);
    for node in graph.nodes().iter().skip(1) {
        assert!(!node.parents.is_empty(), "{} has no parent", node.path);
    }
}

#[test]
fn mermaid_renders_every_node_and_edge() {
    let app = built_app();
    let graph = graph_of(&app);
    let mermaid = graph.to_mermaid();

    assert!(mermaid.starts_with("flowchart TD\n"));
    assert!(mermaid.contains(r#"["CombatPlugin"]"#));

    let arrows = mermaid.lines().filter(|line| line.contains("-->")).count();
    assert_eq!(arrows, graph.edges().count());
}

#[test]
fn json_is_parseable_and_complete() {
    let app = built_app();
    let graph = graph_of(&app);
    let json: serde_json::Value = serde_json::from_str(&graph.to_json()).unwrap();

    assert_eq!(json["root"], 0);
    assert_eq!(json["nodes"][0]["module"], serde_json::Value::Null);
    assert_eq!(json["nodes"][1]["module"], "graph");
    assert_eq!(json["nodes"].as_array().unwrap().len(), graph.nodes().len());
    assert_eq!(
        json["edges"].as_array().unwrap().len(),
        graph.edges().count()
    );
}

#[test]
fn a_single_module_gets_no_colours_or_notes() {
    // Every plugin in `built_app` lives in the same module, so there is nothing to
    // contrast and the overlay stays out of the way.
    let mermaid = graph_of(&built_app()).to_mermaid();

    assert!(!mermaid.contains("classDef"));
    assert!(!mermaid.contains("<small>"));
}

#[test]
fn two_modules_get_module_notes_and_one_class_each() {
    let mut app = App::new();
    app.add_owned(GamePlugin);
    app.add_owned(deeper::NestedPlugin);
    let graph = graph_of(&app);
    let mermaid = graph.to_mermaid();

    // Identity rides inside the node, not in a legend.
    assert!(!mermaid.contains("subgraph"));
    assert!(mermaid.contains(r#"["CombatPlugin<br/><small>graph</small>"]"#));
    assert!(mermaid.contains(r#"["NestedPlugin<br/><small>graph::deeper</small>"]"#));
    // The synthetic root has no module and stays a plain label.
    assert!(mermaid.contains(r#"n0["App"]"#));

    // Biggest module takes the first slot, so its stroke is palette slot 1.
    assert!(mermaid.contains("classDef m0 stroke:#3987e5,stroke-width:2px"));
    assert!(mermaid.contains("classDef m1 stroke:#d95926,stroke-width:2px"));
    assert_eq!(mermaid.matches("classDef").count(), 2);

    // Every non-root node is classed exactly once.
    let classed: usize = mermaid
        .lines()
        .filter(|line| line.trim_start().starts_with("class "))
        .map(|line| line.split(',').count())
        .sum();
    assert_eq!(classed, graph.nodes().len() - 1);
}

#[test]
fn the_plugin_names_the_root() {
    let mut app = App::new();
    app.add_plugins(PluginGraphPlugin::new("MyGame"));
    app.add_owned(GamePlugin);

    let graph = graph_of(&app);
    assert_eq!(graph.root_name(), "MyGame");
    assert!(graph.to_mermaid().contains(r#"n0["MyGame"]"#));
}

#[test]
fn naming_works_whichever_order_the_plugin_is_added() {
    let mut app = App::new();
    app.add_owned(GamePlugin);
    app.add_plugins(PluginGraphPlugin::new("MyGame"));

    assert_eq!(graph_of(&app).root_name(), "MyGame");
}

#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct Render;

struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(DamagePlugin);
    }
}

fn app_with_sub_app() -> App {
    let mut app = App::new();
    app.add_plugins(PluginGraphPlugin::new("Main"));
    app.add_owned(GamePlugin);

    let mut sub = SubApp::new();
    sub.add_plugins(PluginGraphPlugin::new("Render"));
    sub.add_owned(RenderPlugin);
    app.insert_sub_app(Render, sub);
    app
}

#[test]
fn sub_apps_record_a_separate_graph() {
    let app = app_with_sub_app();

    let main = graph_of(&app);
    let sub = bevy_plugin_graph::graph_in(app.sub_app(Render).world()).expect("sub-app graph");

    assert_eq!(main.root_name(), "Main");
    assert_eq!(sub.root_name(), "Render");

    // The sub-app's plugins are in the sub-app's graph and nowhere else.
    assert!(sub.find::<RenderPlugin>().is_some());
    assert!(main.find::<RenderPlugin>().is_none());
    assert!(main.find::<GamePlugin>().is_some());
    assert!(sub.find::<GamePlugin>().is_none());

    // Nesting inside the sub-app is recorded against the sub-app's root.
    let render = sub.find::<RenderPlugin>().unwrap();
    let damage = sub.find::<DamagePlugin>().unwrap();
    assert_eq!(render.parents, vec![ROOT]);
    assert_eq!(damage.parents, vec![render.id]);
}

#[test]
fn each_root_writes_its_own_file() {
    let dir = std::env::temp_dir().join("bevy_plugin_graph_roots");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let base = dir.join("graph.json");

    let mut app = App::new();
    app.add_plugins(
        PluginGraphPlugin::new("Main")
            .to(&base)
            .format(Format::Json),
    );
    app.add_owned(GamePlugin);

    let mut sub = SubApp::new();
    sub.add_plugins(
        PluginGraphPlugin::new("Render")
            .to(&base)
            .format(Format::Json),
    );
    sub.add_owned(RenderPlugin);
    app.insert_sub_app(Render, sub);

    app.finish();

    let main: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("graph.Main.json")).unwrap())
            .unwrap();
    let render: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("graph.Render.json")).unwrap())
            .unwrap();

    assert_eq!(main["nodes"][0]["name"], "Main");
    assert_eq!(render["nodes"][0]["name"], "Render");
    assert_eq!(render["nodes"].as_array().unwrap().len(), 3);

    let _ = std::fs::remove_dir_all(&dir);
}
