//! A small Bevy app with a nested plugin structure, dumped as Mermaid and JSON.
//!
//! Run with `cargo run --example game`. It never calls `App::run()` — the graph is
//! complete as soon as the plugins have finished building.
//!
//! Note that `MinimalPlugins` is added with plain `add_plugins` and so does not
//! appear in the output. Only what you record is recorded.

use bevy::MinimalPlugins;
use bevy::prelude::*;
use bevy_plugin_graph::{AddOwned, PluginGraphPlugin};

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // Writes to $BEVY_PLUGIN_GRAPH if it is set; does nothing otherwise.
    app.add_plugins(PluginGraphPlugin::new());
    app.add_owned(GamePlugin);

    // `finish()` is where PluginGraphPlugin writes. Calling it here rather than
    // `run()` means the runner never starts.
    app.finish();

    println!("{}", bevy_plugin_graph::to_mermaid(&app));
    println!("{}", bevy_plugin_graph::to_json(&app));
}

struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(CorePlugin);
        app.add_owned(CombatPlugin);
        app.add_owned(UiPlugin);
    }
}

struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(SavePlugin);
    }
}

struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(DamagePlugin);
        app.add_owned(WeaponPlugin);
    }
}

struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(HudPlugin);
    }
}

macro_rules! leaf_plugins {
    ($($name:ident),* $(,)?) => {
        $(
            struct $name;
            impl Plugin for $name {
                fn build(&self, _app: &mut App) {}
            }
        )*
    };
}

leaf_plugins!(SavePlugin, DamagePlugin, WeaponPlugin, HudPlugin);
