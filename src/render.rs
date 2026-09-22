//! Renderers. JSON is the interface; Mermaid is one consumer of it.

use crate::graph::PluginGraph;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Categorical stroke colours, assigned to modules in fixed order.
///
/// Validated against both a light (`#fcfcfb`) and a dark (`#1a1a19`) surface: every
/// slot clears the lightness band, chroma floor, colour-vision-deficiency separation
/// (worst adjacent pair ΔE 8.4) and normal-vision separation (worst ΔE 19.3) in both.
/// Slot 4 sits marginally under 3:1 contrast on the light surface, which is why every
/// node carries its module name as a visible label rather than relying on colour alone.
///
/// Order is the safety mechanism, not decoration — do not shuffle or extend it.
const PALETTE: [&str; 8] = [
    "#3987e5", // blue
    "#d95926", // orange
    "#199e70", // aqua
    "#c98500", // yellow
    "#d55181", // magenta
    "#008300", // green
    "#9085e9", // violet
    "#e66767", // red
];

/// Modules past the eighth fold into one neutral bucket rather than getting a
/// generated hue, which would collide with the validated set.
const OTHER: &str = "#8a8a85";

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// The canonical representation.
    #[default]
    Json,
    /// A `flowchart TD`, renderable by GitHub and most editors with no toolchain.
    Mermaid,
}

impl Format {
    /// Guess from a file extension, defaulting to [`Format::Json`].
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("mmd" | "mermaid" | "md") => Format::Mermaid,
            _ => Format::Json,
        }
    }
}

#[derive(Serialize)]
struct WireGraph<'a> {
    root: usize,
    nodes: Vec<WireNode<'a>>,
    edges: Vec<WireEdge>,
}

#[derive(Serialize)]
struct WireNode<'a> {
    id: usize,
    path: &'a str,
    name: &'a str,
    #[serde(rename = "crate")]
    krate: Option<&'a str>,
    module: Option<&'a str>,
    added: u32,
}

#[derive(Serialize)]
struct WireEdge {
    from: usize,
    to: usize,
}

pub(crate) fn to_json(graph: &PluginGraph) -> String {
    let wire = WireGraph {
        root: crate::ROOT.0,
        nodes: graph
            .nodes()
            .iter()
            .map(|node| WireNode {
                id: node.id.0,
                path: &node.path,
                name: &node.name,
                krate: node.krate.as_deref(),
                module: node.module.as_deref(),
                added: node.added,
            })
            .collect(),
        edges: graph
            .edges()
            .map(|(from, to)| WireEdge {
                from: from.0,
                to: to.0,
            })
            .collect(),
    };

    serde_json::to_string_pretty(&wire).expect("the plugin graph is always serializable")
}

pub(crate) fn to_mermaid(graph: &PluginGraph) -> String {
    let mut out = String::from("flowchart TD\n");

    let modules = rank_modules(graph);
    // With a single module there is nothing to contrast, so the overlay (colour and
    // per-node module note alike) stays out of the way.
    let annotate = modules.len() >= 2;

    for node in graph.nodes() {
        match node.module.as_deref().filter(|_| annotate) {
            // The module rides inside the node instead of a legend: identity is
            // never carried by colour alone, and a legend costs layout space.
            // `<br/>` and `<small>` survive Mermaid's strict-mode sanitizer.
            Some(module) => {
                let _ = writeln!(
                    out,
                    "    n{}[\"{}<br/><small>{}</small>\"]",
                    node.id.0,
                    escape(&node.name),
                    escape(module)
                );
            }
            None => {
                let _ = writeln!(out, "    n{}[\"{}\"]", node.id.0, escape(&node.name));
            }
        }
    }

    if graph.edges().next().is_some() {
        out.push('\n');
        for (from, to) in graph.edges() {
            let _ = writeln!(out, "    n{} --> n{}", from.0, to.0);
        }
    }

    if annotate {
        write_module_colours(&mut out, graph, &modules);
    }

    out
}

/// Modules in the order they take colour slots: biggest group first, ties broken by
/// name so that the same app always renders the same colours.
fn rank_modules(graph: &PluginGraph) -> Vec<&str> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for node in graph.nodes() {
        if let Some(module) = node.module.as_deref() {
            *counts.entry(module).or_default() += 1;
        }
    }

    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
    ranked.sort_by(|(a_name, a_count), (b_name, b_count)| {
        b_count.cmp(a_count).then(a_name.cmp(b_name))
    });
    ranked.into_iter().map(|(module, _)| module).collect()
}

/// Colour node *strokes* by module. Identity itself is carried by the module note
/// inside each node, so no legend is emitted.
///
/// Strokes rather than fills: a `classDef` is static, so it cannot carry a
/// light/dark swap, and leaving fill and text to Mermaid keeps the diagram legible
/// in whichever theme the reader is using.
///
/// Deliberately not subgraphs. Grouping would lay nodes out by module and so distort
/// the shape of the wiring tree — which is the very thing the colour is meant to be
/// compared against.
fn write_module_colours(out: &mut String, graph: &PluginGraph, modules: &[&str]) {
    out.push('\n');
    for (slot, colour) in PALETTE.iter().enumerate().take(modules.len()) {
        let _ = writeln!(out, "    classDef m{slot} stroke:{colour},stroke-width:2px");
    }
    if modules.len() > PALETTE.len() {
        let _ = writeln!(out, "    classDef mOther stroke:{OTHER},stroke-width:2px");
    }

    // One `class` line per module keeps the output diffable.
    for (slot, module) in modules.iter().enumerate() {
        let class = if slot < PALETTE.len() {
            format!("m{slot}")
        } else {
            "mOther".to_string()
        };
        let members: Vec<String> = graph
            .nodes()
            .iter()
            .filter(|node| node.module.as_deref() == Some(*module))
            .map(|node| format!("n{}", node.id.0))
            .collect();

        let _ = writeln!(out, "    class {} {class}", members.join(","));
    }
}

/// Mermaid reads `<` and `>` inside quoted labels as HTML, which mangles generic
/// plugins. Its `#entity;` escapes survive.
fn escape(label: &str) -> String {
    label
        .replace('&', "#amp;")
        .replace('<', "#lt;")
        .replace('>', "#gt;")
        .replace('"', "#quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_generics() {
        assert_eq!(escape("Plugin<Foo>"), "Plugin#lt;Foo#gt;");
    }

    #[test]
    fn root_only_mermaid_has_no_edge_block() {
        let rendered = to_mermaid(&PluginGraph::new());
        assert_eq!(rendered, "flowchart TD\n    n0[\"App\"]\n");
    }

    #[test]
    fn palette_slots_are_distinct() {
        let mut seen = PALETTE.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), PALETTE.len());
        assert!(!PALETTE.contains(&OTHER));
    }
}
