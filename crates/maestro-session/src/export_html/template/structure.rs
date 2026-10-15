//! One indentation algorithm for physical and visible tree projections.
use super::{SessionTreeNode, ancestors};
use std::{collections::HashMap, hash::BuildHasher};
/// A continuing vertical line at its original display level.
#[derive(Clone)]
pub struct GutterInfo {
    /// Display level of the original connector.
    pub position: usize,
    /// Whether the sibling line continues.
    pub show: bool,
}
/// Geometry attached to one original node occurrence.
#[derive(Clone, Default)]
pub struct FlatNode {
    /// Logical three-cell indentation level.
    pub indent: usize,
    /// Whether this row starts a branching segment.
    pub show_connector: bool,
    /// Whether this occurrence is the last sibling.
    pub is_last: bool,
    /// Continuing connector positions inherited from ancestors.
    pub gutters: Vec<GutterInfo>,
    /// Whether the connector belongs to the virtual root.
    pub virtual_child: bool,
}
/// Projected parent and children, keyed by the last visible occurrence.
#[derive(Clone, Default)]
pub struct Structure {
    /// Geometry in visible occurrence order.
    pub rows: Vec<FlatNode>,
    /// Nearest visible parent by retained identity.
    pub parents: HashMap<String, Option<String>>,
    /// Ordered visible identities under each parent.
    pub children: HashMap<Option<String>, Vec<String>>,
    /// Whether display indentation suppresses a virtual root.
    pub multiple_roots: bool,
}
/// Pending geometry and the extra first-generation indentation rule.
struct Pending {
    /// Occurrence receiving the geometry.
    index: usize,
    /// Geometry to assign at this occurrence.
    row: FlatNode,
    /// Whether its parent had multiple children.
    just_branched: bool,
}
/// Assign geometry iteratively to either projection, retaining unvisited rows.
fn assign(rows: &mut [FlatNode], roots: &[usize], children: &[Vec<usize>]) {
    let multiple = roots.len() > 1;
    let mut pending: Vec<_> = roots
        .iter()
        .enumerate()
        .rev()
        .map(|(i, &index)| Pending {
            index,
            row: FlatNode {
                indent: usize::from(multiple),
                show_connector: multiple,
                is_last: i + 1 == roots.len(),
                virtual_child: multiple,
                gutters: vec![],
            },
            just_branched: multiple,
        })
        .collect();
    while let Some(Pending {
        index,
        row,
        just_branched,
    }) = pending.pop()
    {
        let branch = children[index].len() > 1;
        let indent = row.indent + usize::from(branch || just_branched && row.indent > 0);
        let mut gutters = row.gutters.clone();
        if row.show_connector && !row.virtual_child {
            gutters.push(GutterInfo {
                position: row.indent.saturating_sub(usize::from(multiple) + 1),
                show: !row.is_last,
            });
        }
        for (i, &child) in children[index].iter().enumerate().rev() {
            pending.push(Pending {
                index: child,
                row: FlatNode {
                    indent,
                    show_connector: branch,
                    is_last: i + 1 == children[index].len(),
                    gutters: gutters.clone(),
                    virtual_child: false,
                },
                just_branched: branch,
            });
        }
        rows[index] = row;
    }
}
/// Initial geometry follows physical child occurrences in active-first order.
#[must_use]
pub fn physical(nodes: &[SessionTreeNode]) -> Vec<FlatNode> {
    let indices: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.index, i))
        .collect();
    let mut has_parent = vec![false; nodes.len()];
    let children: Vec<Vec<_>> = nodes
        .iter()
        .map(|node| {
            let mut children: Vec<_> = node.children().map(|child| indices[&child.index]).collect();
            children.sort_unstable();
            for &child in &children {
                has_parent[child] = true;
            }
            children
        })
        .collect();
    let roots: Vec<_> = has_parent
        .iter()
        .enumerate()
        .filter_map(|(i, &parent)| (!parent).then_some(i))
        .collect();
    let mut rows = vec![FlatNode::default(); nodes.len()];
    assign(&mut rows, &roots, &children);
    rows
}
/// Rebuild visible maps and last-identity geometry from stored parent links.
pub fn recalculate_visual_structure<S: BuildHasher>(
    nodes: &[SessionTreeNode],
    visible: &[SessionTreeNode],
    physical: &[FlatNode],
    parents: &HashMap<String, SessionTreeNode, S>,
) -> Structure {
    let indices: HashMap<_, _> = visible
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.entry()["id"].as_str().map(|id| (id.to_owned(), i)))
        .collect();
    let mut structure = Structure::default();
    let physical: HashMap<_, _> = nodes
        .iter()
        .zip(physical.iter())
        .map(|(n, row)| (n.index, row))
        .collect();
    structure.rows = visible
        .iter()
        .map(|node| physical[&node.index].clone())
        .collect();
    for node in visible {
        let Some(id) = node.entry()["id"].as_str() else {
            continue;
        };
        let parent = ancestors(
            parents,
            parents.get(id).and_then(|n| n.entry()["parentId"].as_str()),
        )
        .find(|parent| indices.contains_key(*parent))
        .map(str::to_owned);
        structure.parents.insert(id.to_owned(), parent.clone());
        structure
            .children
            .entry(parent)
            .or_default()
            .push(id.to_owned());
    }
    let roots: Vec<_> = structure
        .children
        .get(&None)
        .into_iter()
        .flatten()
        .map(|id| indices[id])
        .collect();
    if visible.is_empty() {
        return Structure::default();
    }
    structure.multiple_roots = roots.len() > 1;
    let mut children = vec![vec![]; visible.len()];
    for (id, &index) in &indices {
        children[index] = structure
            .children
            .get(&Some(id.clone()))
            .into_iter()
            .flatten()
            .map(|child| indices[child])
            .collect();
    }
    assign(&mut structure.rows, &roots, &children);
    structure
}
impl Structure {
    /// A visible root or branch-start with descendants can fold.
    #[must_use]
    pub fn is_foldable(&self, id: &str) -> bool {
        self.children
            .get(&Some(id.to_owned()))
            .is_some_and(|children| !children.is_empty())
            && self.parents.get(id).is_none_or(|parent| {
                parent.as_deref().is_none_or(|parent| {
                    self.children
                        .get(&Some(parent.to_owned()))
                        .is_some_and(|siblings| siblings.len() > 1)
                })
            })
    }
}
