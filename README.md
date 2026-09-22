# bevy_plugin_graph

Records which Bevy plugin added which plugin, and renders the result as JSON or Mermaid.

**Added is the only thing we can unambiguously check.** An edge `A → B` means `A`'s
`build()` added `B`. Nothing about actual data flow is inferred or claimed.

## Usage

Name the root, then swap `add_plugins` for `add_owned` at the call sites you want in
the graph:

```rust
use bevy::prelude::*;
use bevy_plugin_graph::{AddOwned, PluginGraphPlugin};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(PluginGraphPlugin::new("Main"));
    app.add_owned(GamePlugin);
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

Then point it at a file:

```sh
BEVY_PLUGIN_GRAPH=graph.mmd cargo run     # Mermaid, from the extension
BEVY_PLUGIN_GRAPH=graph.json cargo run    # JSON
```

To dump without the plugin — and without ever calling `run()` — reach the graph
directly. See `examples/game.rs`:

```rust
bevy_plugin_graph::graph(&app).unwrap().write("graph.mmd", Format::Mermaid)?;
```

## Sub-apps

One graph corresponds to one Bevy `World`, so each sub-app records and dumps its own:

```rust
let mut render = SubApp::new();
render.add_plugins(PluginGraphPlugin::new("RenderApp"));
render.add_owned(RenderPlugin);
app.insert_sub_app(RenderApp, render);
```

The root name is inserted into the output file's stem, so the roots land side by side
and never overwrite each other:

```
BEVY_PLUGIN_GRAPH=graph.mmd  →  graph.Main.mmd
                                graph.RenderApp.mmd
```

Reach a sub-app's graph with `bevy_plugin_graph::graph_in(app.sub_app(Label).world())`.

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

That divergence is a question, not an error. Plugins and modules are orthogonal —
modules govern who can name what at compile time, plugins govern what gets wired in
at runtime — and there are good reasons to define several plugins in one module. The
graph points at candidates; you decide.

JSON is the real interface; Mermaid is one consumer of it.

## What it does not do

- **Plugins added with plain `add_plugins` are invisible.** Third-party plugins and
  `DefaultPlugins` do not appear. Only what you record is recorded.
- **No data-flow analysis, no linting, no violation report.** Bevy keeps per-system
  data access private (`SystemWithAccess::access` is `pub(crate)` with no public
  getter), so "who actually *uses* this component" is not answerable from outside.

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

`DESIGN.md` has the reasoning: why instrumentation rather than tracing or static
analysis, and what is deliberately out of scope.

## License

[MIT](LICENSE).
