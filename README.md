# bevy_plugin_graph

Records which Bevy plugin added which plugin, and renders the result as JSON or Mermaid.

The whole API is one extension trait on `App` and `SubApp`: `PluginGraphExt`, with
`init_graph`, `add_owned`, `graph`, and `dump_graph`.

## Usage

Name the root, swap `add_plugins` for `add_owned` at the call sites you want in
the graph, then dump before `run()`:

```rust
use bevy::prelude::*;
use bevy_plugin_graph::PluginGraphExt;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.init_graph("Main");
    app.add_owned(GamePlugin);

    app.dump_graph("graph.mmd").unwrap();
    app.run();
}

struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_owned(CombatPlugin);
        app.add_owned(UiPlugin);
    }
}
```

Then dump the graph like so:

```rust
if std::env::var_os("DUMP_GRAPH").is_some() {
    app.dump_graph("graph.mmd").unwrap();
    return;
}
app.run();
```

For more control, reach the graph itself:
```rust
app.graph().unwrap().write("out.json", Format::Json)
```

## Sub-apps

One graph corresponds to one Bevy `World`, so each sub-app records its own, named
while you build it:

```rust
let mut render = SubApp::new();
render.init_graph("RenderApp");
render.add_owned(RenderPlugin);
app.insert_sub_app(RenderApp, render);
```

## Output

Nodes are stroked by the module they are defined in, and each node carries its
module as a small note. Here `WeaponPlugin` is defined in `game::ui` but wired in
by `CombatPlugin`, so it is the one node whose colour differs from its neighbours:

```mermaid
flowchart TD
    n0["Main"]
    n1["GamePlugin<br/><small>game</small>"]
    n2["CorePlugin<br/><small>game::core</small>"]
    n3["SavePlugin<br/><small>game::core</small>"]
    n4["SettingsPlugin<br/><small>game::core</small>"]
    n5["CombatPlugin<br/><small>game::combat</small>"]
    n6["DamagePlugin<br/><small>game::combat</small>"]
    n7["WeaponPlugin<br/><small>game::ui</small>"]
    n8["UiPlugin<br/><small>game::ui</small>"]
    n9["HudPlugin<br/><small>game::ui</small>"]
    n0 --> n1
    n1 --> n2
    n2 --> n3
    n2 --> n4
    n1 --> n5
    n5 --> n6
    n5 --> n7
    n1 --> n8
    n8 --> n9
    classDef m0 stroke:#3987e5,stroke-width:2px
    classDef m1 stroke:#d95926,stroke-width:2px
    classDef m2 stroke:#199e70,stroke-width:2px
    classDef m3 stroke:#c98500,stroke-width:2px
    class n2,n3,n4 m0
    class n7,n8,n9 m1
    class n5,n6 m2
    class n1 m3
```

## Compatibility

| `bevy_plugin_graph` | `bevy` |
|---|---|
| 0.1 | 0.19 |

## Development

A Nix flake provides the toolchain:

```sh
nix develop      # or `direnv allow`, the .envrc is there
cargo test
cargo run --example game
```

Refer to `DESIGN.md` for design reasoning.

## License

[MIT](LICENSE).
