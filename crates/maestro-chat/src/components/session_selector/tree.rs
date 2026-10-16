//! Index forest retaining each original session occurrence.
use maestro_app::{Presence, ResourceOperations, SessionInfo, canonicalize_path};
use std::collections::HashMap;
/// Visible original occurrence with its connector text.
pub(super) struct FlatSessionNode {
    /// Index in the immutable source snapshot.
    pub index: usize,
    /// Ancestor gutters and the branch connector.
    pub prefix: String,
}
/// Builds canonical links while retaining all eligible original occurrences.
pub(super) fn build_session_tree(
    sessions: &[std::rc::Rc<SessionInfo>],
    eligible: &[usize],
    paths: &dyn ResourceOperations,
) -> Vec<FlatSessionNode> {
    let canonical: HashMap<_, _> = eligible
        .iter()
        .map(|&i| (canonicalize_path(&sessions[i].path, paths), i))
        .collect();
    let mut parents = vec![None; sessions.len()];
    for &i in eligible {
        if let Presence::Present(value) = &sessions[i].parent_session_path {
            parents[i] = value
                .as_str()
                .filter(|p| !p.is_empty())
                .and_then(|p| canonical.get(&canonicalize_path(p, paths)).copied());
        }
    }
    break_cycles(&mut parents, eligible);
    let mut children = vec![Vec::new(); sessions.len()];
    let mut roots = Vec::new();
    for &i in eligible {
        match parents[i] {
            Some(parent) => children[parent].push(i),
            None => roots.push(i),
        }
    }
    let sort = |nodes: &mut Vec<usize>| {
        nodes.sort_by(|&a, &b| {
            sessions[b]
                .modified
                .partial_cmp(&sessions[a].modified)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    };
    sort(&mut roots);
    for nodes in &mut children {
        sort(nodes);
    }
    flatten_session_tree(&roots, &children)
}
/// Each parent edge is visited once; a gray revisit closes only that cycle edge.
fn break_cycles(parents: &mut [Option<usize>], eligible: &[usize]) {
    let mut visited = vec![0; parents.len()];
    for &start in eligible {
        let mut node = start;
        let mut walk = Vec::new();
        while visited[node] == 0 {
            visited[node] = 1;
            walk.push(node);
            let Some(parent) = parents[node] else { break };
            if visited[parent] == 1 {
                parents[node] = None;
                break;
            }
            node = parent;
        }
        for node in walk {
            visited[node] = 2;
        }
    }
}
/// Iterative preorder maintains each ancestor's continuation gutter.
fn flatten_session_tree(roots: &[usize], children: &[Vec<usize>]) -> Vec<FlatSessionNode> {
    let mut pending: Vec<_> = roots
        .iter()
        .enumerate()
        .rev()
        .map(|(n, &i)| (i, String::new(), n + 1 == roots.len(), true))
        .collect();
    let mut result = Vec::new();
    while let Some((index, gutters, last, root)) = pending.pop() {
        let prefix = if root {
            String::new()
        } else {
            format!("{gutters}{}", if last { "└─ " } else { "├─ " })
        };
        let next = format!("{gutters}{}", if root || last { "   " } else { "│  " });
        result.push(FlatSessionNode { index, prefix });
        pending.extend(
            children[index]
                .iter()
                .enumerate()
                .rev()
                .map(|(n, &child)| (child, next.clone(), n + 1 == children[index].len(), false)),
        );
    }
    result
}
