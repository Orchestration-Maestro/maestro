//! Retained inline image rendering through Component.
use maestro_tui::images::terminal_image::*;
use maestro_tui::{Component, Image, ImageOptions, ImageTheme};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
mod image_support;

/// Replays one retained component's sequential public operations.
fn component_case(case: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let args = &case["args"];
    let count = Rc::new(Cell::new(0));
    let ids = count.clone();
    let terminal = TerminalImage::new(
        |_| None,
        move || {
            ids.set(ids.get() + 1);
            2_147_483_647 + ids.get()
        },
    );
    terminal.set_capabilities(TerminalCapabilities {
        images: image_support::protocol(&args["protocol"]),
        true_color: true,
        hyperlinks: true,
    });
    let calls = Rc::new(RefCell::new(Vec::<String>::new()));
    let styled = calls.clone();
    let options = &args["options"];
    let mut image = Image::new(
        args["payload"]
            .as_str()
            .ok_or("missing text input")?
            .to_owned(),
        "image/png".to_owned(),
        ImageTheme {
            fallback_color: Box::new(move |text| {
                styled.borrow_mut().push(text.to_owned());
                format!("<fallback>{text}</fallback>")
            }),
        },
        ImageOptions {
            max_width_cells: image_support::number(&options["maxWidthCells"]),
            filename: options["filename"].as_str().map(str::to_owned),
            image_id: image_support::number(&options["imageId"]),
            dimensions: args["dimensions"]
                .get("widthPx")
                .map(|_| image_support::dimensions(&args["dimensions"])),
        },
        terminal.clone(),
    );
    let id_before = image_support::image_id(image.get_image_id());
    let mut output = Vec::new();
    for (index, width) in args["widths"]
        .as_array()
        .ok_or("missing width inputs")?
        .iter()
        .enumerate()
    {
        apply_image_action(&mut image, &terminal, args["actions"][index].as_str());
        let width = usize::try_from(width.as_u64().ok_or("missing width")?)?;
        output.push(serde_json::json!({"width": width, "lines": image.render(width), "imageId": image_support::image_id(image.get_image_id())}));
    }
    assert_eq!(
        serde_json::json!({"output": output, "fallbackCalls": *calls.borrow(), "idBefore": id_before, "idAllocations": count.get()}),
        case["expected"],
        "{}",
        case["case"]
    );
    Ok(())
}

#[test]
fn image_dimensions_override_header_and_unknown_default() {
    for case in image_support::cases("image_dimensions_override_header_and_unknown_default") {
        component_case(&case).unwrap();
    }
}

#[test]
fn image_narrow_widths_and_invalid_geometry_fall_back() {
    for case in image_support::cases("image_narrow_widths_and_invalid_geometry_fall_back") {
        if case["symbol"] == "Image" {
            component_case(&case).unwrap();
        } else {
            let args = &case["args"];
            assert_eq!(
                calculate_image_rows(
                    image_support::dimensions(&args[0]),
                    image_support::number(&args[1]).unwrap(),
                    Some(image_support::cells(&args[2]))
                ),
                None
            );
        }
    }
    zero_geometry_falls_back_without_allocating();
}

#[test]
fn image_width_changes_and_invalidation_reuse_id() {
    for case in image_support::cases("image_width_changes_and_invalidation_reuse_id") {
        component_case(&case).unwrap();
    }
}

#[test]
fn image_capability_changes_require_invalidation() {
    for case in image_support::cases("image_capability_changes_require_invalidation") {
        component_case(&case).unwrap();
    }
}

#[test]
fn image_iterm_rows_omit_cursor_restore() {
    for case in image_support::cases("image_iterm_rows_omit_cursor_restore") {
        component_case(&case).unwrap();
    }
}

#[test]
fn image_single_row_has_no_cursor_motion() {
    for case in image_support::cases("image_single_row_has_no_cursor_motion") {
        component_case(&case).unwrap();
    }
}

#[test]
fn image_fallback_styling_runs_after_state_borrow_ends() {
    let terminal = TerminalImage::new(|_| None, || 1);
    terminal.set_capabilities(TerminalCapabilities {
        images: None,
        true_color: true,
        hyperlinks: true,
    });
    let shared = terminal.clone();
    let observations = Rc::new(RefCell::new(Vec::new()));
    let observed = observations.clone();
    let mut image = Image::new(
        "AAAA".to_owned(),
        "image/png".to_owned(),
        ImageTheme {
            fallback_color: Box::new(move |text| {
                observed.borrow_mut().push(shared.get_capabilities());
                shared.reset_capabilities_cache();
                shared.set_cell_dimensions(CellDimensions {
                    width_px: 11,
                    height_px: 22,
                });
                format!("styled:{text}")
            }),
        },
        ImageOptions {
            dimensions: Some(ImageDimensions {
                width_px: 20,
                height_px: 20,
            }),
            ..ImageOptions::default()
        },
        terminal.clone(),
    );
    let lines = image.render(8);
    let caps = observations.borrow()[0];
    let cells = terminal.get_cell_dimensions();
    let output = serde_json::json!({"lines": lines, "cells": {"widthPx": cells.width_px, "heightPx": cells.height_px}, "observations": [image_support::capability_output(caps)]});
    assert_eq!(
        output,
        image_support::cases("image_fallback_styling_runs_after_state_borrow_ends")[0]["expected"]
    );
}

/// Renders real image output before asking the recognizer about optional tool text.
fn recognized_component(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    for case in image_support::cases(name) {
        if case["symbol"] == "isImageLine" {
            assert_eq!(
                is_image_line(case["args"][0].as_str().ok_or("missing text input")?),
                case["expected"]
                    .as_bool()
                    .ok_or("missing recognition expectation")?
            );
            continue;
        }
        let args = &case["args"];
        let terminal = TerminalImage::new(|_| None, || 1);
        terminal.set_capabilities(TerminalCapabilities {
            images: image_support::protocol(&args["protocol"]),
            true_color: true,
            hyperlinks: true,
        });
        let mut image = Image::new(
            "AAAA".to_owned(),
            "image/png".to_owned(),
            ImageTheme {
                fallback_color: Box::new(str::to_owned),
            },
            ImageOptions {
                image_id: image_support::number(&args["imageId"]),
                max_width_cells: image_support::number(&args["maxWidthCells"]),
                dimensions: Some(image_support::dimensions(&args["dimensions"])),
                ..ImageOptions::default()
            },
            terminal,
        );
        let lines = image.render(usize::try_from(
            args["width"].as_u64().ok_or("missing width")?,
        )?);
        let prefix = args["prefix"].as_str().ok_or("missing prefix")?;
        let recognized = lines
            .iter()
            .any(|line| is_image_line(&format!("{prefix}{line}")));
        assert_eq!(
            serde_json::json!({"lines": lines, "recognized": recognized}),
            case["expected"]
        );
    }
    Ok(())
}

#[test]
fn maestro_images_recognizes_rendered_image_components() {
    recognized_component("maestro_images_recognizes_rendered_image_components").unwrap();
}
#[test]
fn maestro_images_recognizes_images_in_tool_output() {
    recognized_component("maestro_images_recognizes_images_in_tool_output").unwrap();
}

#[test]
fn maestro_images_restores_the_cursor_to_the_reserved_image_row_after_kitty_rendering() {
    let terminal = TerminalImage::new(|_| None, || 2_147_483_648);
    terminal.set_capabilities(TerminalCapabilities {
        images: Some(ImageProtocol::Kitty),
        true_color: true,
        hyperlinks: true,
    });
    terminal.set_cell_dimensions(CellDimensions {
        width_px: 10,
        height_px: 10,
    });
    let mut image = Image::new(
        "AAAA".to_owned(),
        "image/png".to_owned(),
        ImageTheme {
            fallback_color: Box::new(str::to_owned),
        },
        ImageOptions {
            max_width_cells: Some(2),
            dimensions: Some(ImageDimensions {
                width_px: 20,
                height_px: 20,
            }),
            ..ImageOptions::default()
        },
        terminal.clone(),
    );
    assert_eq!(
        image.render(4),
        [
            "",
            "\x1b[1A\x1b_Ga=T,f=100,q=2,C=1,c=2,r=2,i=2147483648;AAAA\x1b\\\x1b[1B"
        ]
    );
    replay_cursor_operations(&terminal).unwrap();
}

/// Invalid retained geometry is rejected before ID allocation.
fn zero_geometry_falls_back_without_allocating() {
    let allocations = Rc::new(Cell::new(0));
    let ids = allocations.clone();
    let terminal = TerminalImage::new(
        |_| None,
        move || {
            ids.set(ids.get() + 1);
            1
        },
    );
    terminal.set_capabilities(TerminalCapabilities {
        images: Some(ImageProtocol::Kitty),
        true_color: true,
        hyperlinks: true,
    });
    for dims in [
        ImageDimensions {
            width_px: 0,
            height_px: 5,
        },
        ImageDimensions {
            width_px: 5,
            height_px: 0,
        },
    ] {
        let mut image = Image::new(
            "AAAA".to_owned(),
            "image/png".to_owned(),
            ImageTheme {
                fallback_color: Box::new(|text| format!("styled:{text}")),
            },
            ImageOptions {
                dimensions: Some(dims),
                ..ImageOptions::default()
            },
            terminal.clone(),
        );
        assert_eq!(
            image.render(8),
            [format!(
                "styled:[Image: [image/png] {}x{}]",
                dims.width_px, dims.height_px
            )]
        );
        assert_eq!(image.get_image_id(), None);
    }
    assert_eq!(allocations.get(), 0);
}

/// Replays the helper operations reached during component cursor placement.
fn replay_cursor_operations(terminal: &TerminalImage) -> Result<(), Box<dyn std::error::Error>> {
    for case in image_support::cases(
        "maestro_images_restores_the_cursor_to_the_reserved_image_row_after_kitty_rendering",
    ) {
        let args = &case["args"];
        match case["symbol"].as_str().ok_or("missing operation input")? {
            "setCapabilities" => terminal.set_capabilities(TerminalCapabilities {
                images: image_support::protocol(&args[0]["images"]),
                true_color: true,
                hyperlinks: true,
            }),
            "setCellDimensions" => terminal.set_cell_dimensions(image_support::cells(&args[0])),
            "getCapabilities" => assert_eq!(
                image_support::capability_output(terminal.get_capabilities()),
                case["expected"]
            ),
            "allocateImageId" => assert_eq!(
                terminal.allocate_image_id(),
                image_support::number(&case["expected"]).ok_or("missing ID")?
            ),
            "renderImage" => {
                let result = terminal
                    .render_image(
                        args[0].as_str().ok_or("missing operation input")?,
                        image_support::dimensions(&args[1]),
                        ImageRenderOptions {
                            max_width_cells: image_support::number(&args[2]["maxWidthCells"]),
                            image_id: image_support::number(&args[2]["imageId"]),
                            move_cursor: false,
                            ..ImageRenderOptions::default()
                        },
                    )
                    .ok_or("missing render result")?;
                assert_eq!(
                    serde_json::json!({"sequence": result.sequence, "rows": result.rows, "imageId": result.image_id}),
                    case["expected"]
                );
            }
            "resetCapabilitiesCache" => terminal.reset_capabilities_cache(),
            symbol => return Err(format!("unexpected operation {symbol}").into()),
        }
    }
    Ok(())
}

/// Applies a retained-component invalidation or terminal support change.
fn apply_image_action(image: &mut Image, terminal: &TerminalImage, action: Option<&str>) {
    match action {
        Some("invalidate") => image.invalidate(),
        Some("none" | "iterm2") => terminal.set_capabilities(TerminalCapabilities {
            images: if action == Some("iterm2") {
                Some(ImageProtocol::Iterm2)
            } else {
                None
            },
            true_color: true,
            hyperlinks: true,
        }),
        _ => {}
    }
}
