//! The recorded graph: nodes are plugins, edges are "was added by".

use bevy_ecs::prelude::Resource;
use std::any::{TypeId, type_name};
use std::collections::HashMap;

/// The synthetic root node. Every graph has one, and top-level plugins hang off it
/// so that the rendered result reads as one application rather than a loose forest.
pub const ROOT: NodeId = NodeId(0);

/// Index of a node within a [`PluginGraph`].
///
/// Only meaningful within the graph that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

/// One plugin in the graph.
#[derive(Debug, Clone)]
pub struct PluginNode {
    pub id: NodeId,
    /// Full type path, e.g. `my_game::combat::CombatPlugin`. This is the external
    /// identity of the node; [`TypeId`] is used internally but its value is not
    /// stable across runs, so it is never serialized.
    pub path: String,
    /// `path` with every module qualifier stripped, for labels.
    pub name: String,
    /// The crate the plugin comes from, if the path has any qualifier at all.
    pub krate: Option<String>,
    /// The module the plugin is *defined* in, if any.
    ///
    /// Descriptive only. This crate has no opinion on whether plugins and modules
    /// should line up one to one — there are good reasons to define several plugins
    /// in one module. It is recorded so that consumers can see where the wiring tree
    /// and the module tree diverge, and judge for themselves.
    pub module: Option<String>,
    /// Plugins that added this one.
    ///
    /// A list rather than a single parent: v1 only ever records one, but shared
    /// plugins are a planned addition and widening this later would otherwise
    /// touch every consumer.
    pub parents: Vec<NodeId>,
    /// How many times this plugin was added. Above 1 only for plugins that opt out
    /// of `Plugin::is_unique`.
    pub added: u32,
}

/// The graph recorded so far, plus the stack of plugins currently being built.
///
/// Lives in its world as a resource, inserted only by
/// [`init_graph`](crate::PluginGraphExt::init_graph) — recording is opt-in.
#[derive(Debug, Resource)]
pub struct PluginGraph {
    nodes: Vec<PluginNode>,
    by_type: HashMap<TypeId, NodeId>,
    /// Innermost plugin currently building. Never empty — the root is the floor.
    stack: Vec<NodeId>,
}

impl Default for PluginGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginGraph {
    /// A graph containing only the synthetic root.
    pub fn new() -> Self {
        let root = PluginNode {
            id: ROOT,
            path: "App".to_string(),
            name: "App".to_string(),
            krate: None,
            module: None,
            parents: Vec::new(),
            added: 1,
        };
        Self {
            nodes: vec![root],
            by_type: HashMap::new(),
            stack: vec![ROOT],
        }
    }

    /// Every node, in the order they were first added. The root is always index 0.
    pub fn nodes(&self) -> &[PluginNode] {
        &self.nodes
    }

    /// The name of the synthetic root — the app or sub-app this graph belongs to.
    pub fn root_name(&self) -> &str {
        &self.nodes[ROOT.0].name
    }

    /// Name the synthetic root. Each app and sub-app names its own, so the
    /// rendered graphs can be told apart.
    pub fn set_root_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.nodes[ROOT.0].path = name.clone();
        self.nodes[ROOT.0].name = name;
    }

    pub fn node(&self, id: NodeId) -> Option<&PluginNode> {
        self.nodes.get(id.0)
    }

    /// Every edge as `(parent, child)`, in node order.
    pub fn edges(&self) -> impl Iterator<Item = (NodeId, NodeId)> + '_ {
        self.nodes
            .iter()
            .flat_map(|node| node.parents.iter().map(move |&parent| (parent, node.id)))
    }

    /// Look a plugin up by type.
    pub fn find<P: 'static>(&self) -> Option<&PluginNode> {
        self.by_type
            .get(&TypeId::of::<P>())
            .and_then(|&id| self.node(id))
    }

    /// Record `P` as added by whatever is currently building, and make it the
    /// current parent. Paired with [`PluginGraph::end`].
    pub(crate) fn begin_owned<P: 'static>(&mut self) -> NodeId {
        let parent = *self.stack.last().unwrap_or(&ROOT);
        let type_id = TypeId::of::<P>();

        let id = match self.by_type.get(&type_id) {
            Some(&id) => {
                let node = &mut self.nodes[id.0];
                node.added += 1;
                if !node.parents.contains(&parent) {
                    node.parents.push(parent);
                }
                id
            }
            None => {
                let id = NodeId(self.nodes.len());
                let path = type_name::<P>().to_string();
                self.nodes.push(PluginNode {
                    id,
                    name: short_name(&path),
                    krate: crate_name(&path),
                    module: module_path(&path),
                    path,
                    parents: vec![parent],
                    added: 1,
                });
                self.by_type.insert(type_id, id);
                id
            }
        };

        self.stack.push(id);
        id
    }

    /// Pop the plugin that just finished building. The root is never popped, so an
    /// unbalanced call cannot corrupt the graph.
    pub(crate) fn end_owned(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }
}

/// Strip module qualifiers from every path in a type name, leaving generics intact:
/// `a::B<c::D>` becomes `B<D>`.
fn short_name(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut segment = String::new();

    for ch in path.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == ':' {
            segment.push(ch);
        } else {
            out.push_str(last_segment(&segment));
            segment.clear();
            out.push(ch);
        }
    }
    out.push_str(last_segment(&segment));
    out
}

fn last_segment(segment: &str) -> &str {
    segment.rsplit("::").next().unwrap_or(segment)
}

/// The module a type is defined in, or `None` for an unqualified name.
fn module_path(path: &str) -> Option<String> {
    let head = path.split(['<', '>', ' ', '&', '(', ',']).next()?;
    let (module, _) = head.rsplit_once("::")?;
    Some(module.to_string())
}

/// The crate a type path starts with, or `None` if it has no qualifier.
fn crate_name(path: &str) -> Option<String> {
    let head = path.split(['<', '>', ' ', '&', '(', ',']).next()?;
    let (first, rest) = head.split_once("::")?;
    if first.is_empty() || rest.is_empty() {
        return None;
    }
    Some(first.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_nested_generics() {
        assert_eq!(short_name("my_game::combat::CombatPlugin"), "CombatPlugin");
        assert_eq!(short_name("a::B<c::D>"), "B<D>");
        assert_eq!(short_name("Bare"), "Bare");
    }

    #[test]
    fn extracts_crate() {
        assert_eq!(
            crate_name("my_game::CombatPlugin").as_deref(),
            Some("my_game")
        );
        assert_eq!(crate_name("a::B<c::D>").as_deref(), Some("a"));
        assert_eq!(crate_name("App"), None);
    }

    #[test]
    fn extracts_module() {
        assert_eq!(
            module_path("my_game::combat::CombatPlugin").as_deref(),
            Some("my_game::combat")
        );
        assert_eq!(module_path("App"), None);
    }

    #[test]
    fn root_only_graph_has_no_edges() {
        let graph = PluginGraph::new();
        assert_eq!(graph.nodes().len(), 1);
        assert_eq!(graph.edges().count(), 0);
    }
}
