//! Flattened Rust imports shared by source boundary checks.

/// One item a use tree imports.
pub(crate) struct Import<'a> {
    /// Path segments as written, ending with the imported item.
    pub(crate) path: Vec<&'a syn::Ident>,
    /// Local name the import binds, absent for a glob.
    pub(crate) binding: Option<&'a syn::Ident>,
}

/// Every item one use tree imports, whatever nesting or aliasing it writes.
pub(crate) fn flatten(tree: &syn::UseTree) -> Vec<Import<'_>> {
    let mut imports = Vec::new();
    collect(tree, &mut Vec::new(), &mut imports);
    imports
}

/// Appends the items of one use tree below the segments already written.
fn collect<'a>(
    tree: &'a syn::UseTree,
    above: &mut Vec<&'a syn::Ident>,
    imports: &mut Vec<Import<'a>>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            above.push(&path.ident);
            collect(&path.tree, above, imports);
            above.pop();
        }
        syn::UseTree::Name(name) => {
            let bound = if name.ident == "self" {
                above.last().copied()
            } else {
                Some(&name.ident)
            };
            above.push(&name.ident);
            imports.push(Import {
                path: above.clone(),
                binding: bound,
            });
            above.pop();
        }
        syn::UseTree::Rename(rename) => {
            above.push(&rename.ident);
            imports.push(Import {
                path: above.clone(),
                binding: Some(&rename.rename),
            });
            above.pop();
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect(tree, above, imports);
            }
        }
        syn::UseTree::Glob(_) => imports.push(Import {
            path: above.clone(),
            binding: None,
        }),
    }
}
