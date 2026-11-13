//! The callback pages a login shows in the browser.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Controlled oracle data.
struct SuccessCase {
    /// Supplied oracle message.
    message: String,
    /// Supplied oracle escaped message.
    escaped_message: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Controlled oracle data.
struct ErrorCase {
    /// Supplied oracle message.
    message: String,
    /// Supplied oracle details.
    details: Option<String>,
    /// Supplied oracle escaped message.
    escaped_message: String,
    /// Supplied oracle escaped details.
    escaped_details: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
/// Controlled oracle data.
struct Pages {
    /// Supplied oracle success.
    success: Vec<SuccessCase>,
    /// Supplied oracle error.
    error: Vec<ErrorCase>,
}

#[test]
fn oauth_pages_escape_caller_text() {
    use maestro_providers::oauth_success_html;
    let pages: Pages = serde_json::from_str(include_str!("fixtures/oauth_pages.json")).unwrap();
    assert_eq!((pages.success.len(), pages.error.len()), (9, 63));
    for SuccessCase {
        message,
        escaped_message,
    } in pages.success
    {
        let html = oauth_success_html(&message).unwrap();
        assert!(html.contains(&format!("<p>{escaped_message}</p>")));
        assert!(html.contains("<title>Authentication successful</title>"));
        assert!(html.contains("<h1>Authentication successful</h1>"));
        assert!(!html.contains("<div class=\"details\">"));
        assert!(!html.contains("<script>"));
    }
    for case in pages.error {
        check_error_page(case).unwrap();
    }
}

#[test]
fn oauth_pages_keep_callback_layout() {
    for html in [
        maestro_providers::oauth_success_html("Layout witness").unwrap(),
        maestro_providers::oauth_error_html("Layout witness", Some("Details witness")).unwrap(),
    ] {
        let markers = [
            "<!doctype html>",
            "<html lang=\"en\">",
            "<meta charset=\"utf-8\" />",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />",
            "<style>",
            "<body>",
            "<main>",
            "<div class=\"logo\">",
            "<h1>",
            "<p>Layout witness</p>",
            "</main>",
            "</body>",
            "</html>",
        ];
        let offsets = markers.map(|marker| html.find(marker).unwrap());
        assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(html.matches("<p>").count(), 1);
        for declaration in [
            "box-sizing: border-box",
            "color-scheme: dark",
            "margin: 0;",
            "min-height: 100vh",
            "display: flex",
            "align-items: center",
            "justify-content: center",
            "padding: 24px",
            "text-align: center",
            "width: 100%",
            "max-width: 560px",
            "flex-direction: column",
            "width: 72px",
            "height: 72px",
            "display: block",
            "margin-bottom: 24px",
            "margin: 0 0 10px",
            "font-size: 28px",
            "line-height: 1.15",
            "font-weight: 650",
            "line-height: 1.7",
            "font-size: 15px",
            "margin-top: 16px",
            "font-size: 13px",
            "white-space: pre-wrap",
            "word-break: break-word",
            ".logo svg { width: 100%; height: 100%; }",
        ] {
            assert!(html.contains(declaration), "{declaration}");
        }
        assert!(!html.contains("<script"));
        assert!(!html.contains("@import"));
        assert!(!html.contains("<link"));
    }
}

/// Check each emitted text position for one failed callback query.
fn check_error_page(case: ErrorCase) -> Result<(), maestro_models::DiagnosticErrorInfo> {
    use maestro_providers::oauth_error_html;
    let ErrorCase {
        message,
        details,
        escaped_message,
        escaped_details,
    } = case;
    let html = oauth_error_html(&message, details.as_deref())?;
    assert!(html.contains(&format!("<p>{escaped_message}</p>")));
    assert!(html.contains("<title>Authentication failed</title>"));
    assert!(html.contains("<h1>Authentication failed</h1>"));
    if let Some(text) = escaped_details {
        assert!(html.contains(&format!("<div class=\"details\">{text}</div>")));
    } else {
        assert!(!html.contains("<div class=\"details\">"));
    }
    assert!(!html.contains("<script>"));
    Ok(())
}
