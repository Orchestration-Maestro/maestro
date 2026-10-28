//! Reading a layout tree: its first leaf, what a leaf launches and the limits a tree keeps.

use crate::app::api::{env::normalize_launch_env, responses::api_error};
use maestro_protocol::schema::{ErrorBody, LayoutNode, LayoutPane};

/// Most panes one layout may hold.
const MAX_LAYOUT_PANES: usize = 24;
/// Deepest nesting one layout may have, the root counting as the first level.
const MAX_LAYOUT_DEPTH: usize = 16;

/// What a leaf starts: its extra environment and its literal command, if any.
pub(super) struct LeafLaunch {
    /// Environment pairs added to the pane's process.
    pub(super) extra_env: Vec<(String, String)>,
    /// Program and arguments run without a shell; the configured shell when absent.
    pub(super) argv: Option<Vec<String>>,
}

/// The leftmost leaf of a tree: the one the tab's root pane launches.
pub(super) fn first_leaf(node: &LayoutNode) -> &LayoutPane {
    match node {
        LayoutNode::Pane { pane } => pane,
        LayoutNode::Split { first, .. } => first_leaf(first),
    }
}

/// Reads what a leaf launches, refusing an empty command and a bad environment.
pub(super) fn leaf_launch(pane: &LayoutPane) -> Result<LeafLaunch, ErrorBody> {
    if pane.command.as_ref().is_some_and(Vec::is_empty) {
        return Err(api_error(
            "invalid_layout",
            "pane command must not be empty",
        ));
    }
    Ok(LeafLaunch {
        extra_env: normalize_launch_env(pane.env.clone())
            .map_err(|error| api_error("invalid_layout", error.message))?,
        argv: pane.command.clone(),
    })
}

/// Checks every leaf and split of a tree against the limits, depth first, before anything is
/// created.
///
/// # Errors
/// Returns `invalid_layout` with the first violation found.
pub(super) fn validate_layout_tree(root: &LayoutNode) -> Result<(), ErrorBody> {
    validate_node(root, 1, &mut 0)
}

/// Checks one node at `depth`, counting the leaves seen so far in `panes`.
fn validate_node(node: &LayoutNode, depth: usize, panes: &mut usize) -> Result<(), ErrorBody> {
    if depth > MAX_LAYOUT_DEPTH {
        return Err(api_error(
            "invalid_layout",
            format!("layout depth is {depth}; maximum is {MAX_LAYOUT_DEPTH}"),
        ));
    }
    match node {
        LayoutNode::Pane { pane } => {
            *panes += 1;
            if *panes > MAX_LAYOUT_PANES {
                return Err(api_error(
                    "invalid_layout",
                    format!("layout has more than {MAX_LAYOUT_PANES} panes"),
                ));
            }
            leaf_launch(pane)?;
            Ok(())
        }
        LayoutNode::Split {
            ratio,
            first,
            second,
            ..
        } => {
            if !ratio.is_finite() {
                return Err(api_error("invalid_layout", "split ratio must be finite"));
            }
            validate_node(first, depth + 1, panes)?;
            validate_node(second, depth + 1, panes)
        }
    }
}
