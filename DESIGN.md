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

Nesting inside a sub-app needs no special handling: `Plugin::build` there receives a temporary `App`
wrapping the sub-app, so the `App` implementation already writes into the right world.

## Output

**JSON is the interface**; every other format is a consumer of it. v1 also ships a Mermaid renderer,
because it displays in an editor and on GitHub with no toolchain installed.

Mermaid nodes are stroked by module, with a legend naming each one. The palette is
validated for colour-vision deficiency and for contrast against both a light and a
dark surface; strokes rather than fills, because a `classDef` is static and cannot
carry a theme swap. Modules past the eighth fold into one neutral bucket rather than
getting a generated hue.

Colour rather than nested subgraphs: grouping would lay nodes out by module and so
distort the shape of the wiring tree, which is the thing the module identity is meant
to be compared against. Divergence between the two is a question, not an error — the
renderer surfaces it and says nothing about it.

Emission is a method on the graph. A plugin wraps it to fire from `Plugin::finish()`, with the
output path taken from an environment variable. Every world dumps itself: the root name is inserted
into the configured path's file stem, so `graph.mmd` becomes `graph.Main.mmd` and
`graph.RenderApp.mmd`, side by side and never overwriting each other.

There is deliberately no "dump and exit" option. `App::finish()` finishes the main app's plugins
*before* its sub-apps', so exiting from the main app would kill the process before any sub-app had
a chance to write.

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
