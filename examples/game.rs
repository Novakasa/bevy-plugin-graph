//! A small Bevy app with a nested plugin structure and a sub-app, dumped as Mermaid
//! and JSON.
//!
//! Run with `cargo run --example game`. It never calls `App::run()` — the graphs are
//! complete once the plugins have finished building.
//!
//! Three things worth noticing in the output:
//!
//! - `MinimalPlugins` is added with plain `add_plugins`, so it is absent. Only what
//!   you record is recorded.
//! - `WeaponPlugin` is deliberately defined in the `ui` module while being added by
//!   `CombatPlugin`. In the rendered graph it is the one node whose stroke colour
//!   differs from its neighbours — the shape a refactor candidate takes.
//! - The render sub-app produces a *separate* graph with its own root, because one
//!   graph corresponds to one Bevy `World`.

use bevy::MinimalPlugins;
use bevy::app::{AppLabel, SubApp};
use bevy::prelude::*;
use bevy_plugin_graph::PluginGraphExt;

#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct RenderApp;

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_graph("Main");
    app.add_owned(GamePlugin);

    // A sub-app is its own world, so it records its own graph under its own root.
    let mut render = SubApp::new();
    render.init_graph("RenderApp");
    render.add_owned(render::RenderPlugin);
    app.insert_sub_app(RenderApp, render);

    // `build()` ran synchronously inside each add, so both graphs are already
    // complete — no `finish()`, no runner. To write files instead of printing:
    // `app.dump_graph("main.mmd")` — one explicit path per world.
    let main_graph = app.graph().unwrap();
    println!("{}", main_graph.to_mermaid());
    println!("{}", main_graph.to_json());

    let render_graph = app.sub_app(RenderApp).graph().unwrap();
    println!("{}", render_graph.to_mermaid());
}

struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(core::CorePlugin);
        app.add_owned(combat::CombatPlugin);
        app.add_owned(ui::UiPlugin);
    }
}

mod core {
    use super::*;

    pub struct CorePlugin;

    impl Plugin for CorePlugin {
        fn build(&self, app: &mut App) {
            app.add_owned(SavePlugin);
            app.add_owned(SettingsPlugin);
        }
    }

    pub struct SavePlugin;

    impl Plugin for SavePlugin {
        fn build(&self, _app: &mut App) {}
    }

    pub struct SettingsPlugin;

    impl Plugin for SettingsPlugin {
        fn build(&self, _app: &mut App) {}
    }
}

mod combat {
    use super::*;

    pub struct CombatPlugin;

    impl Plugin for CombatPlugin {
        fn build(&self, app: &mut App) {
            app.add_owned(DamagePlugin);
            // Lives in `ui`, but is wired in here. This is the divergence the colour
            // overlay is for — a question to answer, not an error.
            app.add_owned(super::ui::WeaponPlugin);
        }
    }

    pub struct DamagePlugin;

    impl Plugin for DamagePlugin {
        fn build(&self, _app: &mut App) {}
    }
}

mod ui {
    use super::*;

    pub struct UiPlugin;

    impl Plugin for UiPlugin {
        fn build(&self, app: &mut App) {
            app.add_owned(HudPlugin);
        }
    }

    pub struct HudPlugin;

    impl Plugin for HudPlugin {
        fn build(&self, _app: &mut App) {}
    }

    pub struct WeaponPlugin;

    impl Plugin for WeaponPlugin {
        fn build(&self, _app: &mut App) {}
    }
}

mod render {
    use super::*;

    pub struct RenderPlugin;

    impl Plugin for RenderPlugin {
        fn build(&self, app: &mut App) {
            // Inside a sub-app's plugin, `app` is the sub-app's world — nesting is
            // recorded into that sub-app's graph, not the main one.
            app.add_owned(ExtractPlugin);
            app.add_owned(QueuePlugin);
        }
    }

    pub struct ExtractPlugin;

    impl Plugin for ExtractPlugin {
        fn build(&self, _app: &mut App) {}
    }

    pub struct QueuePlugin;

    impl Plugin for QueuePlugin {
        fn build(&self, _app: &mut App) {}
    }
}
