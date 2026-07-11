use nomo_graph::{Cycle, DirectedGraph};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId {
    segments: Vec<String>,
}

impl ModuleId {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn dotted(&self) -> String {
        self.segments.join(".")
    }
}

impl From<Vec<String>> for ModuleId {
    fn from(segments: Vec<String>) -> Self {
        Self::new(segments)
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.dotted())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleNode {
    pub id: ModuleId,
    pub source_path: PathBuf,
    pub imports: Vec<ModuleId>,
}

impl ModuleNode {
    pub fn new(id: ModuleId, source_path: PathBuf, imports: Vec<ModuleId>) -> Self {
        Self {
            id,
            source_path,
            imports,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleGraph {
    root: ModuleId,
    modules: BTreeMap<ModuleId, ModuleNode>,
    dependencies: DirectedGraph<ModuleId>,
}

impl ModuleGraph {
    pub(crate) fn new(root: ModuleNode) -> Self {
        let root_id = root.id.clone();
        let mut modules = BTreeMap::new();
        modules.insert(root_id.clone(), root);
        let mut dependencies = DirectedGraph::new();
        dependencies.add_node(root_id.clone());
        Self {
            root: root_id,
            modules,
            dependencies,
        }
    }

    pub fn root(&self) -> &ModuleId {
        &self.root
    }

    pub fn module(&self, id: &ModuleId) -> Option<&ModuleNode> {
        self.modules.get(id)
    }

    pub fn module_by_segments(&self, segments: &[String]) -> Option<&ModuleNode> {
        self.modules
            .values()
            .find(|module| module.id.segments() == segments)
    }

    pub fn modules(&self) -> impl ExactSizeIterator<Item = &ModuleNode> {
        self.modules.values()
    }

    pub fn dependencies<'a>(&'a self, id: &ModuleId) -> impl Iterator<Item = &'a ModuleId> {
        self.dependencies.successors(id)
    }

    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    pub fn contains(&self, id: &ModuleId) -> bool {
        self.modules.contains_key(id)
    }

    pub fn topological_order(&self) -> Vec<ModuleId> {
        self.dependencies
            .topological_sort()
            .expect("validated module graph must remain acyclic")
    }

    pub(crate) fn add_module(&mut self, module: ModuleNode) -> bool {
        if self.modules.contains_key(&module.id) {
            return false;
        }
        self.dependencies.add_node(module.id.clone());
        self.modules.insert(module.id.clone(), module);
        true
    }

    pub(crate) fn add_dependency(
        &mut self,
        importer: ModuleId,
        imported: ModuleId,
    ) -> Option<Cycle<ModuleId>> {
        self.dependencies.add_edge(importer, imported);
        self.dependencies.find_cycle()
    }

    pub fn source_path(&self, id: &ModuleId) -> Option<&Path> {
        self.module(id).map(|module| module.source_path.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(path: &str) -> ModuleId {
        ModuleId::new(path.split('.').map(str::to_string).collect())
    }

    #[test]
    fn module_graph_exposes_stable_dependency_order() {
        let root = ModuleNode::new(id("app.main"), "src/main.nomo".into(), vec![id("app.a")]);
        let mut graph = ModuleGraph::new(root);
        graph.add_module(ModuleNode::new(
            id("app.a"),
            "src/a.nomo".into(),
            vec![id("app.b")],
        ));
        graph.add_module(ModuleNode::new(
            id("app.b"),
            "src/b.nomo".into(),
            Vec::new(),
        ));
        assert!(graph.add_dependency(id("app.main"), id("app.a")).is_none());
        assert!(graph.add_dependency(id("app.a"), id("app.b")).is_none());

        assert_eq!(graph.module_count(), 3);
        assert_eq!(
            graph
                .topological_order()
                .iter()
                .map(ModuleId::dotted)
                .collect::<Vec<_>>(),
            vec!["app.b", "app.a", "app.main"]
        );
        assert_eq!(
            graph
                .dependencies(&id("app.main"))
                .map(ModuleId::dotted)
                .collect::<Vec<_>>(),
            vec!["app.a"]
        );
    }
}
