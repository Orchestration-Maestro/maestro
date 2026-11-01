//! Original attachment owners and native thumbnail placements.
#![cfg(test)]

/// Public thumbnail preparation keeps the original mutable application handle.
#[test]
fn attachment_widgets_keep_the_public_embedding_paths() {
    let record = crate::attachment_support::attachment();
    let tile: maestro_web::AttachmentTile = maestro_web::AttachmentTile::from(record.clone());
    assert!(!tile.show_delete);
    assert!(tile.on_delete.is_none());
    assert!(std::sync::Arc::ptr_eq(&tile.attachment, &record));
    drop(record);
    tile.attachment.write().unwrap().file_name = "retained owner.txt".into();
    assert_eq!(
        tile.attachment.read().unwrap().file_name,
        "retained owner.txt"
    );
}

#[cfg(target_arch = "wasm32")]
mod native;
#[cfg(target_arch = "wasm32")]
pub(crate) use native::{
    attachment_modal_hides_and_restores_existing_surfaces,
    attachment_styles_follow_caller_brand_pack,
    attachment_surfaces_keep_fractional_rects_and_owned_content,
    attachment_tiles_replace_transactionally_and_release_listeners,
};
