//! Clipboard image reading through controlled application effects.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::utils::image_clipboard::extension_for_image_mime_type;
#[path = "support/clipboard_image_effects.rs"]
mod controlled;

#[test]
fn extensions_match_normalized_base_mime() {
    for row in controlled::corpus().extensions {
        assert_eq!(
            extension_for_image_mime_type(&row.input),
            row.expected.as_deref()
        );
    }
}

#[test]
fn native_images_retain_png_bytes() {
    controlled::run("native_images_retain_png_bytes");
}

#[test]
fn wayland_reads_without_native() {
    controlled::run("wayland_reads_without_native");
}

#[test]
fn wayland_failures_try_x11() {
    controlled::run("wayland_failures_try_x11");
}

#[test]
fn powershell_embeds_quoted_path() {
    controlled::run("powershell_embeds_quoted_path");
}

#[test]
fn unsupported_images_become_png() {
    controlled::run("unsupported_images_become_png");
}

#[test]
fn default_reader_forwards_ambient_observations() {
    controlled::run("default_reader_forwards_ambient_observations");
}

#[test]
fn classification_overrides_leave_child_environment() {
    controlled::run("classification_overrides_leave_child_environment");
}

#[test]
fn clipboard_conversion_keeps_original_orientation() {
    controlled::run("clipboard_conversion_keeps_original_orientation");
}

#[test]
fn termux_skips_all_effects() {
    controlled::run("termux_skips_all_effects");
}

#[test]
fn native_failures_propagate() {
    controlled::run("native_failures_propagate");
}

#[test]
fn mime_preferences_preserve_raw_labels() {
    controlled::run("mime_preferences_preserve_raw_labels");
}

#[test]
fn x11_candidates_keep_order_and_duplicates() {
    controlled::run("x11_candidates_keep_order_and_duplicates");
}

#[test]
fn wsl_detection_uses_flags_then_kernel() {
    controlled::run("wsl_detection_uses_flags_then_kernel");
}

#[test]
fn linux_routing_keeps_native_off_wayland() {
    controlled::run("linux_routing_keeps_native_off_wayland");
}

#[test]
fn powershell_failures_always_clean_up() {
    controlled::run("powershell_failures_always_clean_up");
}

#[test]
fn conversion_failure_does_not_retry_routes() {
    controlled::run("conversion_failure_does_not_retry_routes");
}

#[test]
fn supported_mime_does_not_validate_bytes() {
    controlled::run("supported_mime_does_not_validate_bytes");
}

#[test]
fn native_and_temp_reads_are_not_command_bounded() {
    controlled::run("native_and_temp_reads_are_not_command_bounded");
}

#[test]
fn command_buffer_failure_uses_fallback() {
    controlled::run("command_buffer_failure_uses_fallback");
}

#[test]
fn native_image_future_finishes_before_return() {
    controlled::run("native_image_future_finishes_before_return");
}

#[test]
fn powershell_safe_paths_remain_expression_literals() {
    controlled::safe_powershell_literal();
}
