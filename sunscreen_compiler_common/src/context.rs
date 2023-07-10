use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

use petgraph::algo::is_isomorphic_matching;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoNodeReferences};
use petgraph::Graph;
use serde::{Deserialize, Serialize};

use crate::{Operation, Render};
use radix_trie::Trie;

/**
 * Stores information about the nodes associated with a certain operation.
 */
#[cfg(feature = "debugger")]
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Group {
    /**
     * The name of the `Group`, representing the name of the operation/gadget.
     */
    pub label: String,

    /**
     * A list of ID's for the nodes contained in the operation.
     */
    pub node_ids: Vec<u64>,
}
#[cfg(feature = "debugger")]
impl Group {
    /**
     * Creates a new `Group` instance.
     */
    pub fn new(name: String) -> Self {
        Group {
            label: name.to_string(),
            node_ids: Vec::new(),
        }
    }

    /**
     * Adds a node ID to the group.
     */
    pub fn add_node<O: Operation>(mut self, node: NodeInfo<O>) {
        self.node_ids.push(node.group_id);
    }
}
/**
 * Stores debug information about groups and stack traces.
 */
#[cfg(feature = "debugger")]
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct DebugData {
    //pub stack_trace: Trie<Vec<u64>, u64>,
}

#[cfg(feature = "debugger")]
impl DebugData {
    /**
     * Creates a new `DebugData` instance.
     */
    pub fn new() -> Self {
        DebugData {
            //group_stack: vec![Group::new()]
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
/**
 * Information about a node in the compilation graph.
 */
pub struct NodeInfo<O>
where
    O: Operation,
{
    /**
     * The operation this node performs.
     */
    pub operation: O,

    #[cfg(feature = "debugger")]
    /**
     * The group ID associated with the ProgramNode.
     */
    pub group_id: u64,
}

impl<O> NodeInfo<O>
where
    O: Operation,
{
    /**
     * Creates a new [`NodeInfo`].
     */
    pub fn new(operation: O, #[cfg(feature = "debugger")] id: u64) -> Self {
        Self {
            operation: operation,
            #[cfg(feature = "debugger")]
            group_id: id,
        }
    }
}

impl<O> Render for NodeInfo<O>
where
    O: Operation,
{
    fn render(&self) -> String {
        format!("{self:?}")
    }
}

impl<O> ToString for NodeInfo<O>
where
    O: Operation,
{
    fn to_string(&self) -> String {
        self.render()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
/**
 * Information about how one compiler graph node relates to another.
 */
pub enum EdgeInfo {
    /**
     * The source node is the left operand of the target.
     */
    Left,

    /**
     * The source node is the right operand of the target.
     */
    Right,

    /**
     * The source node is the only unary operand of the target.
     */
    Unary,

    /**
     * The source node is one of N unordered operands.
     */
    Unordered,

    /**
     * The source is node is i of N ordered operands.
     */
    Ordered(usize),
}

impl EdgeInfo {
    /**
     * Whether or not this edge is a left operand.
     */
    pub fn is_left(&self) -> bool {
        matches!(self, Self::Left)
    }

    /**
     * Whether or not this edge is a right operand.
     */
    pub fn is_right(&self) -> bool {
        matches!(self, Self::Right)
    }

    /**
     * Whether or not this edge is a unary operand.
     */
    pub fn is_unary(&self) -> bool {
        matches!(self, Self::Unary)
    }
}

impl Render for EdgeInfo {
    fn render(&self) -> String {
        format!("{self:?}")
    }
}

#[derive(Clone, Deserialize, Serialize)]
/**
 * The result of a frontend compiler.
 *
 * Need to modify this to also include DebugMetadata which contains info like the tries associated with groups/stack
 * The thing containing the CompilationResult (which is the Context).
 *
 * Also need to have a group stack
 *  I don't need to figure out how to group things on my own
 *  Just determine groups based off of what is currently on the group stack
 *      Make a new type called `Group` which contains group name
 *      Nodes in the `CompilationResult` are actually called `NodeIndex`
 *
 *  Given a function call, we want like a `get_group_id` and `get_groups` that'll return you the group ID and also the group associated
 *      Occassionalyl may need to mutate the trie with getting if the id isn't in there and then return the ID associated with it
 *  with that function
 *   
 * `nodes_for_group` gives you a Vec of NodeIndex that'll, given a group id, return you a vector of nodeindex
 *
 * And need to have a stack trace trie
 *
 * Group stack goes on the level of the Context struct
 */
pub struct CompilationResult<O>
where
    O: Operation,
{
    /**
     * The compilation graph.
     */
    pub graph: StableGraph<NodeInfo<O>, EdgeInfo>,

    /**
     * Stores group data and stack traces.
     */
    #[cfg(feature = "debugger")]
    pub metadata: DebugData,
}

impl<O> Deref for CompilationResult<O>
where
    O: Operation,
{
    type Target = StableGraph<NodeInfo<O>, EdgeInfo>;

    //TODO: support for debugger feature
    fn deref(&self) -> &Self::Target {
        &self.graph
    }
}

impl<O> PartialEq for CompilationResult<O>
where
    O: Operation,
{
    /// FOR TESTING ONLY!!!
    /// Graph isomorphism is an NP-Complete problem!
    fn eq(&self, b: &Self) -> bool {
        is_isomorphic_matching(
            &Graph::from(self.graph.clone()),
            &Graph::from(b.graph.clone()),
            |n1, n2| n1 == n2,
            |e1, e2| e1 == e2,
        )
    }
}

impl<O> Debug for CompilationResult<O>
where
    O: Operation,
{
    //TODO: support for debugger feature
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Nodes = [")?;

        for (i, n) in self.node_references() {
            writeln!(f, "  {i:?}: {n:?}")?;
        }

        writeln!(f, "]")?;

        writeln!(f, "Edges = [")?;

        for i in self.edge_references() {
            writeln!(f, "  {:?}->{:?}: {:?}", i.source(), i.target(), i.weight())?;
        }

        writeln!(f, "]")
    }
}

impl<O> DerefMut for CompilationResult<O>
where
    O: Operation,
{
    //TODO: support for debugger feature
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.graph
    }
}

impl<O> CompilationResult<O>
where
    O: Operation,
{
    /**
     * Create a new [`CompilationResult`]
     */
    pub fn new() -> Self {

        Self {
            graph: StableGraph::new(),
            #[cfg(feature = "debugger")]
            metadata: DebugData::new()
        }
    }
}

impl<O> Default for CompilationResult<O>
where
    O: Operation,
{
    // TODO: support for debugger feature
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/**
 * A compilation context. This stores the current parse graph.
 */
pub struct Context<O, D>
where
    O: Operation,
{
    /**
     * The parse graph.
     */
    pub graph: CompilationResult<O>,

    #[allow(unused)]
    /**
     * Data given by the consumer.
     */
    pub data: D,

    #[cfg(feature = "debugger")]
    /**
     * Used to assign group-set ID's for debugging.
     * Updated whenever a group-set ID is assigned so that ProgramNodes are sequentially identified.
     */
    pub group_counter: u64,

    #[cfg(feature = "debugger")]
    /**
     * Tracks
     */
    pub group_stack: Vec<Group>,
}

// TODO: add modified support for `group_stack` with feature flag
impl<O, D> Context<O, D>
where
    O: Operation,
{
    /**
     * Create a new [`Context`].
     */
    pub fn new(data: D) -> Self {
        Self {
            graph: CompilationResult::<O>::new(),
            data,
            #[cfg(feature = "debugger")] 
            group_stack: Vec::new(),
            #[cfg(feature = "debugger")] 
            group_counter: 0,
        }
    }

    /**
     * Add a node to the parse graph.
     */
    pub fn add_node(&mut self, operation: O) -> NodeIndex {
        #[cfg(feature = "debugger")]
        let group_id = self.group_counter;

        self.graph.add_node(NodeInfo {
            operation,
            #[cfg(feature = "debugger")]
            group_id
        })

    }

    /**
     * Add a binary operation node to the parse graph and edges for
     * the left and right operands.
     */
    pub fn add_binary_operation(
        &mut self,
        operation: O,
        left: NodeIndex,
        right: NodeIndex,
    ) -> NodeIndex {
        let node = self.add_node(operation);

        self.graph.add_edge(left, node, EdgeInfo::Left);
        self.graph.add_edge(right, node, EdgeInfo::Right);

        node
    }

    /**
     * Add a unary operation node to the parse graph and an edge for
     * the unary operand.
     */
    pub fn add_unary_operation(&mut self, operation: O, parent: NodeIndex) -> NodeIndex {
        let node = self.add_node(operation);

        self.graph.add_edge(parent, node, EdgeInfo::Unary);

        node
    }

    /**
     * Add an edge between `from` and `to`.
     */
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, edge: EdgeInfo) {
        self.graph.add_edge(from, to, edge);
    }
}
