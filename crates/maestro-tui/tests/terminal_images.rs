//! Terminal image behavior through public helpers.
use maestro_tui::images::terminal_image::TerminalImage;
use std::{cell::RefCell, rc::Rc};

#[test]
fn capability_environment_precedence_and_color_values() {
    let corpus: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/tests/fixtures/image_cases.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("recorded fixture"),
    )
    .unwrap();
    for case in corpus
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["test"] == "capability_environment_precedence_and_color_values")
    {
        let env = case["environment"].clone();
        let terminal = TerminalImage::new(move |key| env[key].as_str().map(str::to_owned), || 1);
        let caps = terminal.detect_capabilities();
        let protocol = caps.images.map(|protocol| match protocol {
            maestro_tui::images::terminal_image::ImageProtocol::Kitty => "kitty",
            maestro_tui::images::terminal_image::ImageProtocol::Iterm2 => "iterm2",
        });
        assert_eq!(
            serde_json::json!({"images": protocol, "trueColor": caps.true_color, "hyperlinks": caps.hyperlinks}),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn capability_cache_reset_and_override_preserve_cells() {
    use maestro_tui::images::terminal_image::{CellDimensions, TerminalCapabilities};
    let env = std::rc::Rc::new(std::cell::RefCell::new("kitty".to_owned()));
    let input = env.clone();
    let terminal = TerminalImage::new(
        move |key| (key == "TERM_PROGRAM").then(|| input.borrow().clone()),
        || 1,
    );
    let shared = terminal.clone();
    let mut observations = vec![image_support::capability_output(shared.get_capabilities())];
    *env.borrow_mut() = "iterm.app".to_owned();
    observations.push(image_support::capability_output(
        terminal.get_capabilities(),
    ));
    terminal.set_cell_dimensions(CellDimensions {
        width_px: 10,
        height_px: 20,
    });
    shared.reset_capabilities_cache();
    observations.push(image_support::capability_output(
        terminal.get_capabilities(),
    ));
    shared.set_capabilities(TerminalCapabilities::default());
    observations.push(image_support::capability_output(
        terminal.get_capabilities(),
    ));
    let cells = terminal.get_cell_dimensions();
    observations.push(serde_json::json!({"widthPx": cells.width_px, "heightPx": cells.height_px}));
    assert_eq!(
        serde_json::json!(observations),
        image_support::cases("capability_cache_reset_and_override_preserve_cells")[0]["expected"]
    );
    assert_eq!(
        TerminalImage::default().get_cell_dimensions(),
        CellDimensions::default()
    );
    environment_callback_reenters_shared_state();
}

use crate::image_support;
use crate::terminal_support;

#[test]
fn kitty_payload_chunk_boundaries_preserve_parameters() {
    use maestro_tui::images::terminal_image::{KittyOptions, encode_kitty};
    for case in image_support::cases("kitty_payload_chunk_boundaries_preserve_parameters") {
        let options = &case["args"][1];
        assert_eq!(
            encode_kitty(
                case["args"][0].as_str().unwrap(),
                KittyOptions {
                    columns: image_support::number(&options["columns"]),
                    rows: options["rows"]
                        .as_u64()
                        .map(|number| usize::try_from(number).unwrap()),
                    image_id: image_support::number(&options["imageId"]),
                    move_cursor: terminal_support::enabled(&options["moveCursor"]),
                }
            ),
            case["expected"].as_str().unwrap(),
            "{}",
            case["case"]
        );
    }
}

#[test]
fn iterm_payload_options_encode_utf8_names() {
    use maestro_tui::images::terminal_image::{ITerm2Options, ImageSize, encode_i_term2};
    for case in image_support::cases("iterm_payload_options_encode_utf8_names") {
        let options = &case["args"][1];
        let size = |value: &serde_json::Value| {
            image_support::number(value)
                .map(ImageSize::Cells)
                .or_else(|| {
                    value
                        .as_str()
                        .map(|value| ImageSize::Text(value.to_owned()))
                })
        };
        assert_eq!(
            encode_i_term2(
                case["args"][0].as_str().unwrap(),
                if options.as_object().unwrap().is_empty() {
                    ITerm2Options::default()
                } else {
                    ITerm2Options {
                        width: size(&options["width"]),
                        height: size(&options["height"]),
                        name: options["name"].as_str().map(str::to_owned),
                        preserve_aspect_ratio: terminal_support::enabled(
                            &options["preserveAspectRatio"],
                        ),
                        inline: terminal_support::enabled(&options["inline"]),
                    }
                }
            ),
            case["expected"].as_str().unwrap(),
            "{}",
            case["case"]
        );
    }
}

#[test]
fn rows_follow_aspect_ratio_and_custom_cells() {
    use maestro_tui::images::terminal_image::calculate_image_rows;
    for case in image_support::cases("rows_follow_aspect_ratio_and_custom_cells") {
        let args = &case["args"];
        let rows = calculate_image_rows(
            image_support::dimensions(&args[0]),
            image_support::number(&args[1]).unwrap(),
            args.get(2).map(image_support::cells),
        );
        assert_eq!(
            serde_json::json!(rows),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn png_header_boundaries_and_zero_dimensions() {
    for case in image_support::cases("png_header_boundaries_and_zero_dimensions") {
        let output = maestro_tui::images::terminal_image::get_png_dimensions(
            case["args"][0].as_str().unwrap(),
        );
        assert_eq!(
            terminal_support::dimension_output(output),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn jpeg_markers_segments_and_truncation() {
    for case in image_support::cases("jpeg_markers_segments_and_truncation") {
        let output = maestro_tui::images::terminal_image::get_jpeg_dimensions(
            case["args"][0].as_str().unwrap(),
        );
        assert_eq!(
            terminal_support::dimension_output(output),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn gif_versions_and_header_boundaries() {
    for case in image_support::cases("gif_versions_and_header_boundaries") {
        let output = maestro_tui::images::terminal_image::get_gif_dimensions(
            case["args"][0].as_str().unwrap(),
        );
        assert_eq!(
            terminal_support::dimension_output(output),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn webp_variants_and_header_boundaries() {
    for case in image_support::cases("webp_variants_and_header_boundaries") {
        let output = maestro_tui::images::terminal_image::get_webp_dimensions(
            case["args"][0].as_str().unwrap(),
        );
        assert_eq!(
            terminal_support::dimension_output(output),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn mime_dispatch_is_exact() {
    for case in image_support::cases("mime_dispatch_is_exact") {
        let args = &case["args"];
        let result = maestro_tui::images::terminal_image::get_image_dimensions(
            args[0].as_str().unwrap(),
            args[1].as_str().unwrap(),
        );
        assert_eq!(
            terminal_support::dimension_output(result),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn render_protocol_defaults_and_ignored_options() {
    use maestro_tui::images::terminal_image::{
        ImageProtocol, ImageRenderOptions, TerminalCapabilities,
    };
    for case in image_support::cases("render_protocol_defaults_and_ignored_options") {
        let terminal = TerminalImage::new(|_| None, || 1);
        terminal.set_capabilities(TerminalCapabilities {
            images: match case["capabilities"].as_str() {
                Some("kitty") => Some(ImageProtocol::Kitty),
                Some("iterm2") => Some(ImageProtocol::Iterm2),
                _ => None,
            },
            true_color: true,
            hyperlinks: true,
        });
        if case["cells"].is_object() {
            terminal.set_cell_dimensions(image_support::cells(&case["cells"]));
        }
        let args = &case["args"];
        let options = &args[2];
        let result = terminal.render_image(
            args[0].as_str().unwrap(),
            image_support::dimensions(&args[1]),
            ImageRenderOptions {
                max_width_cells: image_support::number(&options["maxWidthCells"]),
                max_height_cells: image_support::number(&options["maxHeightCells"]),
                preserve_aspect_ratio: terminal_support::enabled(&options["preserveAspectRatio"]),
                image_id: image_support::number(&options["imageId"]),
                move_cursor: terminal_support::enabled(&options["moveCursor"]),
            },
        );
        let output = result.map_or(serde_json::Value::Null, |result| {
            let mut output = serde_json::json!({"sequence": result.sequence, "rows": result.rows});
            if case["capabilities"] == "kitty" {
                output["imageId"] = image_support::image_id(result.image_id);
            }
            output
        });
        assert_eq!(output, case["expected"], "{}", case["case"]);
    }
}

#[test]
fn fallback_text_includes_only_present_metadata() {
    for case in image_support::cases("fallback_text_includes_only_present_metadata") {
        let args = &case["args"];
        let result = maestro_tui::images::terminal_image::image_fallback(
            args[0].as_str().unwrap(),
            args.get(1)
                .filter(|value| value.get("widthPx").is_some())
                .map(image_support::dimensions),
            args.get(2).and_then(serde_json::Value::as_str),
        );
        assert_eq!(
            result,
            case["expected"].as_str().unwrap(),
            "{}",
            case["case"]
        );
    }
}

#[test]
fn maestro_images_suppresses_kitty_replies_for_delete_commands() {
    use maestro_tui::images::terminal_image::{delete_all_kitty_images, delete_kitty_image};
    assert_eq!(delete_kitty_image(42), "\x1b_Ga=d,d=I,i=42,q=2\x1b\\");
    assert_eq!(delete_all_kitty_images(), "\x1b_Ga=d,d=A,q=2\x1b\\");
}

#[test]
fn maestro_images_a_render_can_request_no_side_cursor_move() {
    use maestro_tui::images::terminal_image::*;
    for case in image_support::cases("maestro_images_a_render_can_request_no_side_cursor_move") {
        let args = &case["args"];
        assert_eq!(
            encode_kitty(
                args[0].as_str().unwrap(),
                terminal_support::kitty_options(&args[1])
            ),
            case["expected"].as_str().unwrap()
        );
    }
}

#[test]
fn base64_header_inputs_preserve_valid_encodings() {
    use maestro_tui::images::terminal_image::*;
    for case in image_support::cases("base64_header_inputs_preserve_valid_encodings") {
        let args = &case["args"];
        let output = match case["symbol"].as_str().unwrap() {
            "getPngDimensions" => get_png_dimensions(args[0].as_str().unwrap()),
            "getJpegDimensions" => get_jpeg_dimensions(args[0].as_str().unwrap()),
            "getGifDimensions" => get_gif_dimensions(args[0].as_str().unwrap()),
            "getWebpDimensions" => get_webp_dimensions(args[0].as_str().unwrap()),
            symbol => panic!("unexpected header reader {symbol}"),
        };
        assert_eq!(
            terminal_support::dimension_output(output),
            case["expected"],
            "{}",
            case["case"]
        );
    }
}

#[test]
fn invalid_cell_dimensions_leave_previous_values() {
    use maestro_tui::images::terminal_image::*;
    for case in image_support::cases("invalid_cell_dimensions_leave_previous_values") {
        let args = &case["args"];
        let terminal = TerminalImage::default();
        terminal.set_cell_dimensions(CellDimensions {
            width_px: 10,
            height_px: 20,
        });
        terminal.set_cell_dimensions(image_support::cells(&args[0]));
        let dims = terminal.get_cell_dimensions();
        assert_eq!(
            serde_json::json!({"widthPx": dims.width_px, "heightPx": dims.height_px}),
            case["expected"]
        );
    }
}

#[test]
fn kitty_optional_parameters_preserve_truthiness_and_order() {
    use maestro_tui::images::terminal_image::*;
    for case in image_support::cases("kitty_optional_parameters_preserve_truthiness_and_order") {
        let args = &case["args"];
        assert_eq!(
            encode_kitty(
                args[0].as_str().unwrap(),
                terminal_support::kitty_options(&args[1])
            ),
            case["expected"].as_str().unwrap()
        );
    }
}

#[test]
fn image_id_sources_cover_inclusive_bounds_and_swap() {
    use maestro_tui::images::terminal_image::*;
    use maestro_tui::{Component, Image, ImageOptions, ImageTheme};
    let mut source_ids = [1, 2_147_483_648, 0xffff_fffe].into_iter();
    for case in image_support::cases("image_id_sources_cover_inclusive_bounds_and_swap") {
        if case["symbol"] == "allocateImageId" {
            let value = source_ids.next().unwrap();
            let terminal = TerminalImage::new(|_| None, move || value);
            assert_eq!(
                terminal.allocate_image_id(),
                image_support::number(&case["expected"]).unwrap()
            );
        } else {
            let mut output = Vec::new();
            for id in [1, 0xffff_fffe] {
                let terminal = TerminalImage::new(|_| None, move || id);
                terminal.set_capabilities(TerminalCapabilities {
                    images: Some(ImageProtocol::Kitty),
                    true_color: true,
                    hyperlinks: true,
                });
                let image = Image::new(
                    "AAAA".to_owned(),
                    "image/png".to_owned(),
                    ImageTheme {
                        fallback_color: Box::new(str::to_owned),
                    },
                    ImageOptions {
                        dimensions: Some(ImageDimensions {
                            width_px: 20,
                            height_px: 20,
                        }),
                        ..ImageOptions::default()
                    },
                    terminal,
                );
                output.push(
                    serde_json::json!({"lines": image.render(4), "imageId": image.get_image_id()}),
                );
            }
            assert_eq!(serde_json::json!(output), case["expected"]);
        }
    }
    let native = TerminalImage::default();
    assert!((1..=0xffff_fffe).contains(&native.allocate_image_id()));
    id_callback_reenters_shared_state();
}

/// ID callbacks can update the terminal whose ID is being requested.
fn id_callback_reenters_shared_state() {
    use maestro_tui::images::terminal_image::*;
    let shared = Rc::new(RefCell::new(None::<TerminalImage>));
    let ids = shared.clone();
    let terminal = TerminalImage::new(
        |_| None,
        move || {
            let terminal = ids.borrow().clone();
            if let Some(terminal) = terminal {
                terminal.reset_capabilities_cache();
                terminal.set_cell_dimensions(CellDimensions {
                    width_px: 12,
                    height_px: 24,
                });
            }
            7
        },
    );
    *shared.borrow_mut() = Some(terminal.clone());
    assert_eq!(terminal.allocate_image_id(), 7);
    assert_eq!(
        terminal.get_cell_dimensions(),
        CellDimensions {
            width_px: 12,
            height_px: 24
        }
    );
    shared.borrow_mut().take();
}

/// Detection callbacks can access terminal state while the cache is being filled.
fn environment_callback_reenters_shared_state() {
    let shared = Rc::new(RefCell::new(None::<TerminalImage>));
    let environment = shared.clone();
    let terminal = TerminalImage::new(
        move |key| {
            let terminal = environment.borrow().clone();
            if let Some(terminal) = terminal {
                terminal.set_cell_dimensions(maestro_tui::CellDimensions {
                    width_px: 13,
                    height_px: 26,
                });
            }
            (key == "TERM_PROGRAM").then(|| "kitty".to_owned())
        },
        || 1,
    );
    *shared.borrow_mut() = Some(terminal.clone());
    assert_eq!(
        terminal.get_capabilities().images,
        Some(maestro_tui::ImageProtocol::Kitty)
    );
    assert_eq!(
        terminal.get_cell_dimensions(),
        maestro_tui::CellDimensions {
            width_px: 13,
            height_px: 26
        }
    );
    shared.borrow_mut().take();
}

/// Recorded cases whose line is checked for an image sequence.
const IMAGE_LINE_CASES: &[&str] = &[
    "maestro_images_an_iterm2_image_at_line_start_is_found",
    "maestro_images_text_before_an_iterm2_image_still_marks_an_image_line",
    "maestro_images_an_iterm2_image_mid_line_is_found",
    "maestro_images_an_iterm2_image_at_line_end_is_found",
    "maestro_images_the_shortest_iterm2_image_is_found",
    "maestro_images_a_kitty_image_at_line_start_is_found",
    "maestro_images_text_before_a_kitty_image_still_marks_an_image_line",
    "maestro_images_padded_kitty_parameters_are_read",
    "maestro_images_a_line_of_304k_characters_still_carries_images",
    "maestro_images_detect_image_sequences_when_terminal_doesn_t_support_images",
    "maestro_images_ansi_codes_before_an_image_still_mark_an_image_line",
    "maestro_images_ansi_codes_after_an_image_still_mark_an_image_line",
    "maestro_images_plain_text_carries_no_image",
    "maestro_images_ansi_codes_alone_are_no_image",
    "maestro_images_cursor_codes_alone_are_no_image",
    "maestro_images_an_incomplete_iterm2_sequence_is_no_image",
    "maestro_images_an_incomplete_kitty_sequence_is_no_image",
    "maestro_images_empty_lines_carry_no_image",
    "maestro_images_newlines_alone_are_no_image",
    "maestro_images_kitty_and_iterm2_images_coexist_in_a_line",
    "maestro_images_text_and_images_sharing_a_line_still_mark_an_image_line",
    "maestro_images_a_path_holding_image_words_is_no_image",
];

#[test]
fn image_line_cases_report_whether_a_line_carries_an_image() {
    use maestro_tui::images::terminal_image::is_image_line;
    for name in IMAGE_LINE_CASES {
        for case in image_support::cases(name) {
            let args = &case["args"];
            assert_eq!(
                is_image_line(args[0].as_str().unwrap()),
                case["expected"].as_bool().unwrap(),
                "{name}"
            );
        }
    }
}

/// Recorded cases whose environment selects the terminal capabilities.
const CAPABILITY_CASES: &[&str] = &[
    "maestro_images_unknown_terminals_start_without_hyperlinks",
    "maestro_images_tmux_turns_hyperlinks_off_despite_outer_osc_8",
    "maestro_images_a_tmux_terminal_turns_hyperlinks_off",
    "maestro_images_a_screen_terminal_turns_hyperlinks_off",
    "maestro_images_enables_hyperlinks_for_the_ghostty_terminal_identifier",
    "maestro_images_cmux_keeps_ghostty_images_on",
    "maestro_images_hyperlink_wraps_text_for_kitty",
    "maestro_images_enables_hyperlinks_for_the_wezterm_terminal_identifier",
    "maestro_images_hyperlink_wraps_text_for_iterm2",
    "maestro_images_enables_hyperlinks_for_the_vscode_terminal_identifier",
];

#[test]
fn capability_cases_detect_hyperlink_support_from_the_environment() {
    for name in CAPABILITY_CASES {
        for case in image_support::cases(name) {
            let env = case["environment"].clone();
            let terminal =
                TerminalImage::new(move |key| env[key].as_str().map(str::to_owned), || 1);
            assert_eq!(
                image_support::capability_output(terminal.detect_capabilities()),
                case["expected"],
                "{name}"
            );
        }
    }
}

/// Recorded cases whose text is wrapped in a hyperlink.
const HYPERLINK_CASES: &[&str] = &[
    "maestro_images_text_gets_osc_8_open_and_close",
    "maestro_images_ansi_styling_survives_inside_the_hyperlink",
    "maestro_images_hyperlink_wraps_empty_text",
    "maestro_images_hyperlink_wraps_file_uri_targets",
];

#[test]
fn hyperlink_cases_wrap_text_in_osc_8() {
    use maestro_tui::images::terminal_image::hyperlink;
    for name in HYPERLINK_CASES {
        for case in image_support::cases(name) {
            let args = &case["args"];
            assert_eq!(
                hyperlink(args[0].as_str().unwrap(), args[1].as_str().unwrap()),
                case["expected"].as_str().unwrap(),
                "{name}"
            );
        }
    }
}

/// Recorded cases whose operations drive one terminal's image rendering.
const RENDER_SEQUENCE_CASES: &[&str] = &[
    "maestro_images_preserves_renderimage_s_default_terminal_side_cursor_movement",
    "maestro_images_render_image_can_decline_side_cursor_moves",
];

#[test]
fn render_sequence_cases_replay_their_operations_in_order() {
    for name in RENDER_SEQUENCE_CASES {
        let env = image_support::cases(name)[0]["environment"].clone();
        let terminal = TerminalImage::new(move |key| env[key].as_str().map(str::to_owned), || 1);
        for case in image_support::cases(name) {
            let args = &case["args"];
            match case["symbol"].as_str().unwrap() {
                "setCapabilities" => {
                    terminal.set_capabilities(terminal_support::capabilities(&args[0]));
                }
                "setCellDimensions" => terminal.set_cell_dimensions(image_support::cells(&args[0])),
                "resetCapabilitiesCache" => terminal.reset_capabilities_cache(),
                "renderImage" => {
                    let result = terminal.render_image(
                        args[0].as_str().unwrap(),
                        image_support::dimensions(&args[1]),
                        terminal_support::render_options(&args[2]),
                    );
                    assert_eq!(
                        terminal_support::render_output(result),
                        terminal_support::render_expected(case["expected"].clone()),
                        "{name}"
                    );
                }
                symbol => panic!("{name}: unexpected rendering operation {symbol}"),
            }
        }
    }
}
