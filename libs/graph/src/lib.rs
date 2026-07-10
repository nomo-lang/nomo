//! Deterministic directed graph utilities shared by Nomo workspace layers.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle<N> {
    path: Vec<N>,
}

impl<N> Cycle<N> {
    pub fn new(path: Vec<N>) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &[N] {
        &self.path
    }

    pub fn into_path(self) -> Vec<N> {
        self.path
    }
}

impl<N: fmt::Display> fmt::Display for Cycle<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, node) in self.path.iter().enumerate() {
            if index > 0 {
                formatter.write_str(" -> ")?;
            }
            node.fmt(formatter)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectedGraph<N> {
    edges: BTreeMap<N, BTreeSet<N>>,
}

impl<N> Default for DirectedGraph<N> {
    fn default() -> Self {
        Self {
            edges: BTreeMap::new(),
        }
    }
}

impl<N: Ord + Clone> DirectedGraph<N> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: N) -> bool {
        if self.edges.contains_key(&node) {
            return false;
        }
        self.edges.insert(node, BTreeSet::new());
        true
    }

    pub fn add_edge(&mut self, from: N, to: N) -> bool {
        self.edges.entry(to.clone()).or_default();
        self.edges.entry(from).or_default().insert(to)
    }

    pub fn contains_node(&self, node: &N) -> bool {
        self.edges.contains_key(node)
    }

    pub fn contains_edge(&self, from: &N, to: &N) -> bool {
        self.edges
            .get(from)
            .is_some_and(|successors| successors.contains(to))
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &N> {
        self.edges.keys()
    }

    pub fn successors<'a>(&'a self, node: &N) -> impl Iterator<Item = &'a N> {
        self.edges.get(node).into_iter().flatten()
    }

    pub fn node_count(&self) -> usize {
        self.edges.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.values().map(BTreeSet::len).sum()
    }

    /// Returns dependencies before dependants for edges `dependant -> dependency`.
    pub fn topological_sort(&self) -> Result<Vec<N>, Cycle<N>> {
        let mut states = BTreeMap::new();
        let mut stack = Vec::new();
        let mut ordered = Vec::with_capacity(self.node_count());
        for node in self.edges.keys() {
            if !states.contains_key(node) {
                self.visit(node, &mut states, &mut stack, &mut ordered)?;
            }
        }
        Ok(ordered)
    }

    pub fn find_cycle(&self) -> Option<Cycle<N>> {
        self.topological_sort().err()
    }

    fn visit(
        &self,
        node: &N,
        states: &mut BTreeMap<N, VisitState>,
        stack: &mut Vec<N>,
        ordered: &mut Vec<N>,
    ) -> Result<(), Cycle<N>> {
        states.insert(node.clone(), VisitState::Visiting);
        stack.push(node.clone());

        for successor in self.successors(node) {
            match states.get(successor) {
                Some(VisitState::Visited) => {}
                Some(VisitState::Visiting) => {
                    let start = stack
                        .iter()
                        .position(|candidate| candidate == successor)
                        .expect("visiting graph node must be present in the DFS stack");
                    let mut path = stack[start..].to_vec();
                    path.push(successor.clone());
                    return Err(Cycle::new(path));
                }
                None => self.visit(successor, states, stack, ordered)?,
            }
        }

        stack.pop();
        states.insert(node.clone(), VisitState::Visited);
        ordered.push(node.clone());
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_tracks_nodes_and_edges_without_duplicates() {
        let mut graph = DirectedGraph::new();

        assert!(graph.add_node("app"));
        assert!(!graph.add_node("app"));
        assert!(graph.add_edge("app", "core"));
        assert!(!graph.add_edge("app", "core"));
        assert!(graph.contains_node(&"core"));
        assert!(graph.contains_edge(&"app", &"core"));
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn topological_sort_is_stable_and_dependency_first() {
        let mut graph = DirectedGraph::new();
        graph.add_edge("app", "core");
        graph.add_edge("core", "util");
        graph.add_node("standalone");

        assert_eq!(
            graph.topological_sort().unwrap(),
            vec!["util", "core", "app", "standalone"]
        );
    }

    #[test]
    fn cycle_includes_the_closing_node() {
        let mut graph = DirectedGraph::new();
        graph.add_edge("app.a", "app.b");
        graph.add_edge("app.b", "app.c");
        graph.add_edge("app.c", "app.a");

        let cycle = graph.find_cycle().unwrap();
        assert_eq!(cycle.path(), &["app.a", "app.b", "app.c", "app.a"]);
        assert_eq!(cycle.to_string(), "app.a -> app.b -> app.c -> app.a");
    }

    #[test]
    fn self_cycle_is_reported() {
        let mut graph = DirectedGraph::new();
        graph.add_edge("app.main", "app.main");

        assert_eq!(
            graph.find_cycle().unwrap().into_path(),
            vec!["app.main", "app.main"]
        );
    }
}
