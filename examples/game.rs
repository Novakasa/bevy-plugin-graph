//! A small Bevy app with a nested plugin structure, dumped as Mermaid and JSON.
//!
//! Run with `cargo run --example game`. It never calls `App::run()` — the graph is
//! complete as soon as the plugins have finished building.
//!
//! Two things worth noticing in the output:
//!
//! - `MinimalPlugins` is added with plain `add_plugins`, so it is absent. Only what
//!   you record is recorded.
//! - `WeaponPlugin` is deliberately defined in the `ui` module while being added by
//!   `CombatPlugin`. In the rendered graph it is the one node whose stroke colour
//!   differs from its neighbours — the shape a refactor candidate takes.

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
