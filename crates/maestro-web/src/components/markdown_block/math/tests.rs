//! Registered math builder and reached document witnesses.
use super::{context, render_to_string};
use katex::{KatexContext, OutputFormat, Settings};

/// Settings used by the document's HTML math path.
fn settings(display: bool) -> Settings {
    Settings::builder()
        .output(OutputFormat::Html)
        .throw_on_error(false)
        .display_mode(display)
        .build()
}

/// Invoke the registered parse/build/serialize path.
fn render(ctx: &KatexContext, expression: &str, display: bool) -> String {
    render_to_string(ctx, expression, &settings(display)).unwrap()
}

#[test]
fn bare_radical_builder_retains_symbol_padding() {
    let ctx = context();
    for expression in [r"\sqrt x", r"\sqrt{x}", r"\sqrt[3]x"] {
        let html = render(&ctx, expression, false);
        assert!(
            html.contains("padding-left:0.833em"),
            "registered radical padding"
        );
        let document = super::super::render::prepare(&format!("${expression}$"), false);
        assert!(
            document.html.contains("padding-left:0.833em"),
            "document radical padding"
        );
    }
}

#[test]
fn explicit_size_builder_keeps_display_clearance() {
    let ctx = context();
    let expression = r"\Huge\sqrt{x}";
    let display = render(&ctx, expression, true);
    assert!(
        display.contains("height:0.8492em"),
        "registered display clearance"
    );
    let document = super::super::render::prepare(&format!("$${expression}$$"), false);
    assert!(
        document.html.contains("height:0.8492em"),
        "document display clearance"
    );
    for (formula, mode, clearance) in [
        (expression, false, "height:0.8003em"),
        (r"\sqrt{x}", true, "height:0.8492em"),
    ] {
        assert!(render(&ctx, formula, mode).contains(clearance));
    }
}

#[test]
fn builtin_macro_failure_reaches_inline_error_owner() {
    let ctx = context();
    assert!(
        render(&ctx, r"\char", false).contains("class=\"katex-error\""),
        "builtin error owner"
    );
    let document = super::super::render::prepare(r"$\char$", false);
    assert!(
        document.html.contains("class=\"katex-error\""),
        "document error owner"
    );
    let unknown = render(&ctx, r"\unknowncommand", false);
    assert!(!unknown.contains("katex-error"));
    assert!(unknown.contains("unknowncommand"));
}

#[test]
fn malformed_environment_reaches_caller_fallback() {
    let ctx = context();
    let expression = r"\begin{\foo}x\end{\foo}";
    assert!(
        render_to_string(&ctx, expression, &settings(false)).is_err(),
        "typed malformed-name failure"
    );
    let document = super::super::render::prepare(&format!("${expression}$"), false);
    assert_eq!(
        document.html,
        format!("<p><span class=\"maestro-math-fallback\">${expression}$</span></p>\n")
    );
    assert!(render(&ctx, r"\frac{", false).contains("katex-error"));
    assert!(!render(&ctx, r"\begin{matrix}a&b\\c&d\end{matrix}", false).contains("katex-error"));
}

#[test]
fn math_macro_errors_are_local_to_each_formula() {
    for _ in 0..2 {
        let ctx = context();
        for (expression, error) in [
            (r"\char", true),
            (r"\char65", false),
            (r"\newcommand{\fresh}{x}\fresh", false),
        ] {
            assert_eq!(
                render(&ctx, expression, false).contains("katex-error"),
                error
            );
        }
    }
    let document = super::super::render::prepare(r"$\char$ $\char65$ $\def\char{x}\char$", false);
    assert_eq!(document.html.matches("class=\"katex-error\"").count(), 1);
    assert_eq!(document.html.matches("class=\"katex\"").count(), 2);
    let next = super::super::render::prepare(r"$\char65$", false);
    assert!(!next.html.contains("katex-error"));
}

thread_local! {
    /// Actual iterations observed in the registered sizing builder.
    static SIZING_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Observe one actual rescaling iteration, only in test builds.
pub(super) fn record_size_visit() {
    SIZING_VISITS.set(SIZING_VISITS.get() + 1);
}

#[test]
fn explicit_size_work_stays_below_recorded_upper_bounds() {
    let ctx = context();
    for (n, flat_limit, nested_limit) in [(8, 30, 97), (16, 62, 321), (32, 126, 1153)] {
        let flat = format!("\\Huge {}", vec!["x"; n].join("+"));
        let nested = (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    r"\Huge x+"
                } else {
                    r"\small x+"
                }
            })
            .collect::<String>()
            + "x";
        for (expression, limit) in [(&flat, flat_limit), (&nested, nested_limit)] {
            SIZING_VISITS.set(0);
            assert!(render(&ctx, expression, true).contains("katex"));
            let visits = SIZING_VISITS.get();
            eprintln!(
                "SIZING N={n} flat={} visits={visits} upper={limit}",
                expression == &flat
            );
            assert!(visits <= limit, "N={n}: {visits} exceeds {limit}");
        }
        SIZING_VISITS.set(0);
        for _ in 0..2 {
            assert!(render(&ctx, &nested, true).contains("katex"));
        }
        assert!(
            SIZING_VISITS.get() > nested_limit,
            "duplicated-render control must be rejected"
        );
    }
}

#[test]
fn formula_tags_reach_escaped_math_errors_on_all_delimiters() {
    for (open, close) in [("$", "$"), ("\\(", "\\)"), ("$$", "$$"), ("\\[", "\\]")] {
        let source = format!("{open}\\char <img src=x onerror=alert(1)>{close}");
        let document = super::super::render::prepare(&source, false);
        assert!(
            !document.html.contains("<img"),
            "formula cannot publish an authored image: {open}"
        );
        assert!(document.html.contains("class=\"katex-error\""));
        assert!(document.html.contains("&lt;img"));
    }
    let document = super::super::render::prepare("before <b>kept</b>", false);
    assert!(document.html.contains("<b>kept</b>"));
}
