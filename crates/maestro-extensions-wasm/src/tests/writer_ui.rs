//! The frame writer and custom overlays through the controlled adapter.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    ControlledDriver,
    basic_ui::{install, operate},
    scenario::Driver,
};
use serde_json::json;

/// The transcript lines of one prefix, from `start` on.
fn lines(driver: &ControlledDriver, start: usize, prefix: &str) -> Vec<String> {
    driver.transcript()[start..]
        .iter()
        .filter(|line| line.starts_with(prefix))
        .cloned()
        .collect()
}

#[test]
fn controlled_frame_writer_forwards_each_request_in_order() -> Result<(), String> {
    let mut driver = ControlledDriver::new();
    super::block_on(async {
        driver.start().await?;
        install(&mut driver).await?;
        assert_eq!(
            operate(&mut driver, json!({"op":"writerFactory"})).await?,
            json!({"ok": true})
        );
        let start = driver.transcript().len();
        for command in [
            "children", "clear", "settings", "overlay", "focus", "hide", "terminal", "debug",
            "start", "render", "stop",
        ] {
            driver.widget_input("writer", command)?;
        }
        assert_eq!(
            driver.widget("writer", 40).await?,
            [
                r#"rows ["child"]"#,
                "rows []",
                "cursor true shrink true",
                "overlay true focused true",
                "focus true true",
                "overlay true",
                "terminal 80x24 kitty false",
                "debug set",
                "started",
                "render requested",
                "stopped",
            ]
        );
        let writer = lines(&driver, start, "writer ");
        let lent = writer[4]
            .strip_prefix("writer showOverlay component ")
            .and_then(|rest| rest.strip_suffix(" false"))
            .ok_or_else(|| format!("unexpected request {}", writer[4]))?
            .to_owned();
        assert_eq!(writer, expected(&lent));
        let start = driver.transcript().len();
        assert_eq!(
            operate(&mut driver, json!({"op":"writerDrain"})).await?,
            json!({"ok":"drained"})
        );
        assert_eq!(
            lines(&driver, start, "writer "),
            ["writer drainInput Some(250.0) None"]
        );
        driver.release_all().await;
        Ok(())
    })
}

/// The frame writer requests the commands make, around the identity the overlay was lent under.
fn expected(lent: &str) -> Vec<String> {
    [
        "writer setShowHardwareCursor true".to_owned(),
        "writer setClearOnShrink true".to_owned(),
        "writer getShowHardwareCursor".to_owned(),
        "writer getClearOnShrink".to_owned(),
        format!("writer showOverlay component {lent} false"),
        "writer hasOverlay".to_owned(),
        "writer setFocus None".to_owned(),
        format!("writer lentLive {lent}"),
        format!("writer setFocus Some(\"lent {lent}\")"),
        "writer hideOverlay".to_owned(),
        "writer hasOverlay".to_owned(),
        "writer terminal write \"literal\"".to_owned(),
        "writer terminal moveBy -2".to_owned(),
        "writer terminal hideCursor".to_owned(),
        "writer terminal showCursor".to_owned(),
        "writer terminal clearLine".to_owned(),
        "writer terminal clearFromCursor".to_owned(),
        "writer terminal clearScreen".to_owned(),
        "writer terminal setTitle \"title\"".to_owned(),
        "writer terminal setProgress true".to_owned(),
        "writer terminal columns".to_owned(),
        "writer terminal rows".to_owned(),
        "writer terminal kitty".to_owned(),
        "writer setOnDebug true".to_owned(),
        "writer start".to_owned(),
        "writer requestRender true".to_owned(),
        "writer stop".to_owned(),
    ]
    .into()
}

#[test]
fn controlled_custom_overlay_resolves_placement_and_hands_over_its_handle() -> Result<(), String> {
    let mut driver = ControlledDriver::new();
    super::block_on(async {
        driver.start().await?;
        install(&mut driver).await?;
        let placement = json!({
            "width": {"cells": 30}, "min_width": null, "max_height": null, "anchor": "top-left",
            "offset_x": null, "offset_y": null, "row": null, "col": null, "margin": null,
            "non_capturing": null,
        });
        for resolved in [false, true] {
            let start = driver.transcript().len();
            assert_eq!(
                operate(
                    &mut driver,
                    json!({"op":"customOverlay", "resolved":resolved})
                )
                .await?,
                json!({"ok":"overlay done"})
            );
            let observed: Vec<_> = driver.transcript()[start..]
                .iter()
                .filter(|line| {
                    ["overlay ", "custom overlay", "visible"]
                        .iter()
                        .any(|prefix| line.starts_with(prefix))
                })
                .cloned()
                .collect();
            assert_eq!(
                observed,
                [
                    "overlay visible true".to_owned(),
                    format!("custom overlay {placement}"),
                    "visible true".to_owned(),
                    "overlay handle isFocused".to_owned(),
                    "overlay handle isHidden".to_owned(),
                    "custom overlay rows overlay focused=true hidden=false".to_owned(),
                    "visible failed: visibility failed at 80x24".to_owned(),
                ]
            );
        }
        driver.release_all().await;
        Ok(())
    })
}
