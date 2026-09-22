# bevy-plugin-graph — Design

A Bevy crate that records which plugin added which plugin, and renders the result.

## Thesis

**Added is the only thing we can unambiguously check.**

The graph is a record of plugin *containment*: `A → B` means `A`'s `build()` added `B`. Nothing more
is claimed and nothing is inferred.

The reason to build it is an architectural question: *does "whoever adds a plugin is the only thing
that interacts with it" hold as a discipline in a real Bevy app?* Bevy does not enforce it — the
`World` is one flat, globally-reachable namespace — so the graph shows intent, not enforcement.
Seeing the intent drawn out is the point; judging it is left to the reader.

The graph is not the architecture — it is *evidence about where the architecture
boundaries should go*. Rust modules, not plugins, are what actually enforce an API
boundary: modules govern who can name what at compile time, plugins govern what gets
wired in at runtime. The two axes are orthogonal, and the tool exists to show where
they diverge so a human can judge whether the divergence is deliberate.

## Why instrumentation, not inference

Three ways to learn the plugin structure:

| Approach | Verdict |
|---|---|
| Explicit `add_owned` call sites | **Chosen.** Ground truth, and a typed API leaves room to attach per-plugin metadata later. |
| `tracing` layer over Bevy's `plugin build` spans | Deferred. Zero annotation cost, but a span carries a `&str` — it can never carry a declared API. Additive later. |
| Static analysis (`syn`) over source | Rejected. Reads code rather than the app that actually got built, and misses feature flags and conditional adds. `bevy_xray` already occupies this lane. |

Bevy keeps `App::plugin_registry` and `plugin_build_depth` private, so there is no public API that
exposes the plugin tree. Instrumentation is not a shortcut; it is the only honest option.

## Model

- A **node** is a plugin, identified by `TypeId`, appearing once per graph.
- Each node also records the **module** it is defined in. Descriptive only: the crate
  has no opinion on whether plugins and modules line up one to one, and there are
  good reasons to define several plugins in one module.
- An **edge** is "added by". Edges are recorded by the instrumentation, never inferred.
- One graph corresponds to one Bevy `World`. An app and each of its sub-apps record
  separately, because `SubApp::add_plugins` swaps the sub-app into a temporary `App`,
  so the recording resource naturally lands in that sub-app's own world.
- A synthetic **root** anchors the top-level plugins. Each world names its own, and
  that name both labels the root node and selects the output file.

Because every edge is a real add, the v1 graph is a tree — a genuine cycle would recurse inside
Bevy and crash long before it reached the graph. The data model nonetheless stores parents as a
list, so multi-parent nodes can be introduced later without reworking consumers.

## Surface

```rust
app.add_owned(CombatPlugin);
```

An extension method taking a single plugin, implemented for both `App` and `SubApp`. It pushes a
node, delegates to `add_plugins`, and pops. `build()` runs synchronously in between, so nesting is
captured without traits, macros, or `unsafe`. Plugins added with plain `add_plugins` are simply
absent from the graph.

Recording is opt-in, not ambient: `init_graph(root)` creates the graph and names its root, and
`add_owned` records only into a graph that already exists — on an uninitialized world it degrades
to plain `add_plugins`. Nothing is inserted implicitly, so a world that never opted in carries no
resource and no recording cost, and the one place a graph can come from is visible in the caller's
`main`.

The whole surface is one extension trait, `PluginGraphExt` (`add_owned`, `init_graph`, `graph`,
`dump_graph`), implemented for `App` and `SubApp` — needed because Bevy provides no shared trait
over the two, and one trait rather than several because every consumer uses the methods together:
a single import, no free functions with `_in` variants per receiver type. A bare `World` is served
by the `PluginGraph` resource being public rather than by a third impl.

Nesting inside a sub-app needs no special handling: `Plugin::build` there receives a temporary `App`
wrapping the sub-app, so the `App` implementation already writes into the right world.

## Output

**JSON is the interface**; every other format is a consumer of it. v1 also ships a Mermaid renderer,
because it displays in an editor and on GitHub with no toolchain installed.

Mermaid nodes are stroked by module, with the module named in a small note inside
each node — identity is never carried by colour alone, and an in-node note costs
no layout space, where a legend subgraph does. The palette is
validated for colour-vision deficiency and for contrast against both a light and a
dark surface; strokes rather than fills, because a `classDef` is static and cannot
carry a theme swap. Modules past the eighth fold into one neutral bucket rather than
getting a generated hue.

Colour rather than nested subgraphs: grouping would lay nodes out by module and so
distort the shape of the wiring tree, which is the thing the module identity is meant
to be compared against. Divergence between the two is a question, not an error — the
renderer surfaces it and says nothing about it.

Emission is a method on the graph, called from `main`. Because `build()` runs synchronously inside
each add, the graph is complete the moment the last add in `main` returns — before `run()`, before
`finish()`, before any schedule. There is deliberately no plugin, no `finish()` hook and no
environment variable: recording is explicit, and so is dumping. When to trigger it — a CLI flag, an
env var check, a dedicated binary that dumps and returns instead of calling `run()` — is the
caller's policy, not the crate's. This also sidesteps the `App::finish()` ordering trap that a
hook-based dump would have (the main app finishes before its sub-apps, so a dump-and-exit hook
would kill the process before any sub-app had written).

`dump` writes exactly the path it is given, inferring only the format from the extension — no name
interpolation: in a multi-world app, each world is dumped by an explicit call, so each call names
its own file. The root name labels the root node and nothing else; sub-apps are named the same way
they are recorded: explicitly, while being built.

## Scope

Bevy 0.19, tracking the latest Bevy release. Built as a personal tool first; publishing is a
question for after it proves useful.

**In v1:** the containment graph for an app and each of its sub-apps, JSON and Mermaid output.

**Not in v1:** shared/multi-parent plugins, any form of linting or violation reporting, foreign and
un-instrumented plugins, DOT output, an in-app UI.

## Beyond v1

Two directions, in rough order of interest:

1. **Declared plugin API** — a plugin states which components, messages, resources and schedules it
   considers public, rendered into its node. This is what the typed approach was chosen to keep
   open.
2. **Tracing-based structure** — recovering the tree with no annotations at all, including third-party
   plugins, and treating the difference between the observed and the declared tree as a result in
   itself.

Worth knowing about the first: `bevy_ecs` 0.19 keeps per-system data access private
(`SystemWithAccess::access` is `pub(crate)` with no public getter), so automatically determining
which plugin *uses* a given type is not possible against the public API. Only declarations can be
compared against each other.
