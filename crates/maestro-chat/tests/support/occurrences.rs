//! Retained occurrence association through records and physical children.
use super::tree::Node;
/// Associates retained occurrences by source records and ordered physical children.
pub fn occurrences(
    list: &maestro_chat::TreeList,
    nodes: &[Node],
) -> Vec<Option<maestro_chat::SessionTreeNode>> {
    let restore = super::registry::Restore(maestro_tui::get_keybindings());
    maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
        maestro_app::presentation_data::bindings::KEYBINDINGS.clone(),
        vec![],
    ));
    let mut observed = Vec::new();
    if let Some(start) = list.get_selected_node() {
        loop {
            observed.push(list.get_selected_node().unwrap());
            list.handle_input("\u{1b}[B").unwrap();
            if list.get_selected_node().unwrap() == start {
                break;
            }
            assert!(observed.len() <= nodes.len());
        }
    }
    let mut result = vec![None; nodes.len()];
    let mut pending = Vec::new();
    for handle in observed {
        let candidates: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.entry == **handle.entry())
            .collect();
        if let [(index, _)] = candidates.as_slice() {
            pending.push((*index, handle));
        }
    }
    while let Some((index, handle)) = pending.pop() {
        assert_eq!(**handle.entry(), nodes[index].entry);
        assert_eq!(handle.children().len(), nodes[index].children.len());
        pending.extend(nodes[index].children.iter().copied().zip(handle.children()));
        result[index] = Some(handle);
    }
    drop(restore);
    result
}
