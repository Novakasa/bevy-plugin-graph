//! Integration tests against a real `App` with a nested plugin structure.

use bevy_app::{App, Plugin};
use bevy_plugin_graph::{AddOwned, Format, PluginGraphPlugin, ROOT};

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

fn built_app() -> App {
    let mut app = App::new();
    app.add_owned(GamePlugin);
    app
}

#[test]
fn records_the_nesting() {
    let app = built_app();
    let graph = bevy_plugin_graph::graph(&app).expect("graph was recorded");

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
    let graph = bevy_plugin_graph::graph(&app).unwrap();

    assert!(graph.find::<UnrecordedPlugin>().is_none());
    // root + Game + Combat + Damage + Ui
    assert_eq!(graph.nodes().len(), 5);
}

#[test]
fn nodes_carry_short_names_and_crates() {
    let app = built_app();
    let graph = bevy_plugin_graph::graph(&app).unwrap();
    let combat = graph.find::<CombatPlugin>().unwrap();

    assert_eq!(combat.name, "CombatPlugin");
    assert!(combat.path.ends_with("CombatPlugin"));
    assert_eq!(combat.krate.as_deref(), Some("graph"));
}

#[test]
fn every_node_but_the_root_has_a_parent() {
    let app = built_app();
    let graph = bevy_plugin_graph::graph(&app).unwrap();

    assert_eq!(graph.edges().count(), graph.nodes().len() - 1);
    for node in graph.nodes().iter().skip(1) {
        assert!(!node.parents.is_empty(), "{} has no parent", node.path);
    }
}

#[test]
fn mermaid_renders_every_node_and_edge() {
    let app = built_app();
    let mermaid = bevy_plugin_graph::to_mermaid(&app);

    assert!(mermaid.starts_with("flowchart TD\n"));
    assert!(mermaid.contains(r#"n0["App"]"#));
    assert!(mermaid.contains(r#"["CombatPlugin"]"#));

    let graph = bevy_plugin_graph::graph(&app).unwrap();
    let arrows = mermaid.lines().filter(|line| line.contains("-->")).count();
    assert_eq!(arrows, graph.edges().count());
}

#[test]
fn json_is_parseable_and_complete() {
    let app = built_app();
    let json: serde_json::Value = serde_json::from_str(&bevy_plugin_graph::to_json(&app)).unwrap();

    assert_eq!(json["root"], 0);
    assert_eq!(json["nodes"][0]["name"], "App");
    assert_eq!(json["nodes"][0]["crate"], serde_json::Value::Null);

    let graph = bevy_plugin_graph::graph(&app).unwrap();
    assert_eq!(json["nodes"].as_array().unwrap().len(), graph.nodes().len());
    assert_eq!(
        json["edges"].as_array().unwrap().len(),
        graph.edges().count()
    );
}

#[test]
fn an_app_with_no_recording_still_has_a_root() {
    let app = App::new();
    let mermaid = bevy_plugin_graph::to_mermaid(&app);

    assert!(bevy_plugin_graph::graph(&app).is_none());
    assert_eq!(mermaid, "flowchart TD\n    n0[\"App\"]\n");
}

#[test]
fn the_plugin_writes_the_file_it_is_told_to() {
    let path = std::env::temp_dir().join("bevy_plugin_graph_test.json");
    let _ = std::fs::remove_file(&path);

    let mut app = App::new();
    app.add_plugins(PluginGraphPlugin::to(&path).format(Format::Json));
    app.add_owned(GamePlugin);
    app.finish();

    let written = std::fs::read_to_string(&path).expect("graph was written");
    let json: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert!(json["nodes"].as_array().unwrap().len() >= 5);

    let _ = std::fs::remove_file(&path);
}

mod deeper {
    use super::*;

    pub struct NestedPlugin;

    impl Plugin for NestedPlugin {
        fn build(&self, _app: &mut App) {}
    }
}

#[test]
fn nodes_carry_their_defining_module() {
    let mut app = App::new();
    app.add_owned(GamePlugin);
    app.add_owned(deeper::NestedPlugin);

    let graph = bevy_plugin_graph::graph(&app).unwrap();
    assert_eq!(
        graph.find::<CombatPlugin>().unwrap().module.as_deref(),
        Some("graph")
    );
    assert_eq!(
        graph
            .find::<deeper::NestedPlugin>()
            .unwrap()
            .module
            .as_deref(),
        Some("graph::deeper")
    );
    // The synthetic root is not defined anywhere.
    assert_eq!(graph.nodes()[0].module, None);
}

#[test]
fn json_carries_the_module() {
    let app = built_app();
    let json: serde_json::Value = serde_json::from_str(&bevy_plugin_graph::to_json(&app)).unwrap();

    assert_eq!(json["nodes"][0]["module"], serde_json::Value::Null);
    assert_eq!(json["nodes"][1]["module"], "graph");
}

#[test]
fn a_single_module_gets_no_legend_or_colours() {
    // Every plugin in `built_app` lives in the same module, so there is nothing to
    // contrast and the overlay stays out of the way.
    let mermaid = bevy_plugin_graph::to_mermaid(&built_app());

    assert!(!mermaid.contains("classDef"));
    assert!(!mermaid.contains("subgraph legend"));
}

#[test]
fn two_modules_get_a_legend_and_one_class_each() {
    let mut app = App::new();
    app.add_owned(GamePlugin);
    app.add_owned(deeper::NestedPlugin);

    let mermaid = bevy_plugin_graph::to_mermaid(&app);

    assert!(mermaid.contains("subgraph legend[\"modules\"]"));
    assert!(mermaid.contains(r#"l0["graph"]"#));
    assert!(mermaid.contains(r#"l1["graph::deeper"]"#));

    // Biggest module takes the first slot, so its stroke is palette slot 1.
    assert!(mermaid.contains("classDef m0 stroke:#3987e5,stroke-width:2px"));
    assert!(mermaid.contains("classDef m1 stroke:#d95926,stroke-width:2px"));
    assert_eq!(mermaid.matches("classDef").count(), 2);

    // Every non-root node is classed exactly once, plus its legend swatch.
    let graph = bevy_plugin_graph::graph(&app).unwrap();
    let classed: usize = mermaid
        .lines()
        .filter(|line| line.trim_start().starts_with("class "))
        .map(|line| line.split(',').count())
        .sum();
    assert_eq!(classed, graph.nodes().len() - 1 + 2);
}
