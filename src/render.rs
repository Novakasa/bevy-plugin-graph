//! Renderers. JSON is the interface; Mermaid is one consumer of it.

use crate::graph::PluginGraph;
use serde::Serialize;
use std::path::Path;

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

    for node in graph.nodes() {
        out.push_str(&format!("    n{}[\"{}\"]\n", node.id.0, escape(&node.name)));
    }

    if graph.edges().next().is_some() {
        out.push('\n');
        for (from, to) in graph.edges() {
            out.push_str(&format!("    n{} --> n{}\n", from.0, to.0));
        }
    }

    out
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
}
