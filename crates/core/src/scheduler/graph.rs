use std::collections::HashSet;
use std::fmt::Write;
use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use smallvec::SmallVec;

use crate::component::ComponentId;
use crate::resource::ResourceId;
use crate::system::{System, SystemId};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AccessType {
    None,
    Entity,
    World,
    Component(ComponentId),
    Resource(ResourceId),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AccessDesc {
    pub(crate) ty: AccessType,
    pub(crate) exclusive: bool,
}

impl AccessDesc {
    pub fn conflicts(&self, other: &Self) -> bool {
        if self.ty == AccessType::World {
            return true;
        }

        self.ty == other.ty && (self.exclusive || other.exclusive)
    }
}

#[derive(Debug)]
pub struct ScheduleNode {
    pub id: SystemId,
}

fn hash_system_id(id: SystemId) -> u64 {
    let mut hasher = FxHasher::with_seed(0);
    id.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
pub struct ScheduleGraph {
    pub(crate) systems: FxHashMap<SystemId, Box<dyn System>>,
    pub(crate) nodes: Vec<ScheduleNode>,
    pub(crate) edges: FxHashSet<(usize, usize)>,
}

impl ScheduleGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns HTML of a graph of the schedule.
    pub fn render(&self) -> String {
        let mut nodes = String::new();
        for node in &self.nodes {
            let sys = self.systems.get(&node.id).unwrap();
            let id = hash_system_id(node.id);
            let name = sys.meta().name();

            nodes += &format!("{id}(\"{name}\");");
        }

        let mut edges = String::new();
        for &(edge1, edge2) in &self.edges {
            let from = hash_system_id(self.nodes[edge1].id);
            let to = hash_system_id(self.nodes[edge2].id);

            edges += &format!("{from} --> {to};");
        }

        format!(
            r#"
                <!DOCTYPE html>
                <html>
                    <head>
                        <title>Bedrock ECS scheduler graph</title>
                    </head>
                    <body>
                        <script type="module">
                            import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs";
                            mermaid.initialize({{ startOnLoad: true }});
                        </script>

                        <pre class="mermaid">
                            ---
                            title: Bedrock ECS Scheduler Graph
                            config:
                                theme: neutral
                                look: handDrawn
                            ---
                            flowchart TD
                                {nodes}
                                {edges}
                        </pre>
                    </body>
                </html>
            "#
        )
    }
}
