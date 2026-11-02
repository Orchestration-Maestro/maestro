//! Recorded-usage report behaviour.
#![cfg(all(test, not(target_arch = "wasm32")))]

use crate::analysis_fixtures::{ZONE, expected, home, resolved, run_usage, sessions};
use crate::isolated_home::with_environment;

const HELP: &str = "Usage: scripts/stats.ts [options]

Options:
  -n, --days <days>         Number of local calendar days to include (default: 7)
  -d, --dir, --cwd <path>   Project cwd to inspect (default: current cwd)
  --sessions-base <path>    Sessions base directory (default: ~/.maestro/agent/sessions)
  -h, --help                Show this help
";

/// The rows of one recorded usage report.
#[derive(Clone, Copy)]
struct Totals {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    total_tokens: f64,
    cost_total: f64,
}

/// An assistant entry with supplied usage members.
fn usage_entry(timestamp: &str, provider: &str, totals: Totals) -> String {
    format!(
        "{{\"type\":\"message\",\"timestamp\":\"{timestamp}\",\"message\":{{\"role\":\"assistant\",\"provider\":\"{provider}\",\"usage\":{{\"input\":{},\"output\":{},\"cacheRead\":{},\"cacheWrite\":{},\"totalTokens\":{},\"cost\":{{\"total\":{}}}}}}}}}\n",
        totals.input,
        totals.output,
        totals.cache_read,
        totals.cache_write,
        totals.total_tokens,
        totals.cost_total
    )
}

/// Run the report with the supplied session base.
///
/// # Errors
/// Returns the report's own rejection.
fn report(base: &str, extra: &[&str]) -> std::io::Result<crate::analysis_fixtures::Run> {
    let sessions_base = format!("{base}/home/.maestro/agent/sessions");
    let mut args = vec!["--cwd", "/project", "--sessions-base", &sessions_base];
    args.extend_from_slice(extra);
    run_usage(&args)
}

/// An assistant entry reporting 7/3/2/1 at cost 1 with `member` as the raw total member.
fn raw_usage_entry(member: &str) -> String {
    format!(
        "{{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{{\"role\":\"assistant\",\"provider\":\"one\",\"usage\":{{\"input\":7,\"output\":3,\"cacheRead\":2,\"cacheWrite\":1{member},\"cost\":{{\"total\":1}}}}}}}}\n"
    )
}

/// One report totals row for a single assistant reporting 7/3/2/1 at cost 1.
fn totals_row(label: &str, total: &str) -> String {
    format!(
        "{label:<16} messages: {:>5}  sessions: {:>3}  input: {:>12}  output: {:>10}  \
         cache read: {:>13}  cache write: {:>10}  total: {:>13}  cost: {:>10}",
        "1", "1", "7", "3", "2", "1", total, "$1.0000"
    )
}

/// Run the report for `cwd`, whose sessions sit beneath `base`'s home.
///
/// # Errors
/// Returns the report's own rejection.
fn report_for(base: &str, cwd: &str) -> std::io::Result<crate::analysis_fixtures::Run> {
    let sessions_base = format!("{base}/home/.maestro/agent/sessions");
    run_usage(&["--cwd", cwd, "--sessions-base", &sessions_base])
}

#[test]
fn usage_report_leaves_missing_total_at_zero() {
    with_environment(
        "usage_report_leaves_missing_total_at_zero",
        &[("TZ", ZONE)],
        |base| {
            for (index, (member, shown)) in [
                ("", "0"),
                (",\"totalTokens\":null", "0"),
                (",\"totalTokens\":0", "0"),
                (",\"totalTokens\":-0.0", "0"),
                (",\"totalTokens\":97", "97"),
                (",\"totalTokens\":1.5", "2"),
                (",\"totalTokens\":-2", "-2"),
            ]
            .into_iter()
            .enumerate()
            {
                let cwd = format!("/project{index}");
                let journal = raw_usage_entry(member);
                sessions(
                    base,
                    &resolved(&cwd),
                    &[("2026-03-09T12-00-00-000Z_a.jsonl", &journal)],
                );
                let run = report_for(base, &cwd).expect("the report runs");
                assert!(run.succeeded(), "{}", run.stderr);
                for label in ["2026-03-09", "  one", "TOTAL"] {
                    let row = totals_row(label, shown);
                    assert!(
                        run.stdout.contains(&row),
                        "{member:?} reports no {row:?} in:\n{}",
                        run.stdout
                    );
                }
            }
        },
    );
}

#[test]
fn usage_arguments_keep_defaults_aliases_and_decimal_prefixes() {
    with_environment(
        "usage_arguments_keep_defaults_aliases_and_decimal_prefixes",
        &[("TZ", ZONE)],
        |base| {
            sessions(base, &resolved("/project"), &[]);
            for days in ["0", "-1", "bad", "0x10"] {
                let error =
                    report(base, &["--days", days]).expect_err("an invalid day count is rejected");
                assert!(
                    error
                        .to_string()
                        .contains("--days must be a positive integer"),
                    "{days}: {error}"
                );
            }
            for (args, label) in [
                (vec!["--days", "7"], "7"),
                (vec!["--days", "7suffix"], "7"),
                (vec!["--days", "+2.9"], "2"),
                (vec!["--days"], "7"),
                (vec!["--days", ""], "7"),
                (vec!["-n", "1"], "1"),
            ] {
                let run = report(base, &args).expect("the report runs");
                assert!(run.succeeded(), "{args:?}: {}", run.stderr);
                assert!(
                    run.stdout.contains(&format!("({label} local days)")),
                    "{args:?}: {}",
                    run.stdout
                );
            }
            for alias in ["--cwd", "--dir", "-d"] {
                let run = report(base, &[alias, "/project"]).expect("the report runs");
                assert!(run.succeeded(), "{alias}");
                let heading = format!("Usage for {}\n", resolved("/project"));
                assert!(run.stdout.starts_with(&heading), "{alias}");
            }
            let run = run_usage(&["--help"]).expect("help runs");
            assert_eq!(run.stdout, HELP);
            assert!(run.succeeded());
            let run = run_usage(&["-h"]).expect("help runs");
            assert_eq!(run.stdout, HELP);
            assert!(run.succeeded());
        },
    );
}

#[test]
fn usage_counts_local_calendar_days_across_dst() {
    with_environment(
        "usage_counts_local_calendar_days_across_dst",
        &[("TZ", ZONE)],
        |base| {
            if !maestro_test_process::local_zone_follows_tz() {
                return;
            }
            let noon = Totals {
                input: 1.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: 1.0,
                cost_total: 0.0,
            };
            let journal = format!(
                "{}{}{}",
                usage_entry("2026-03-03T05:00:00Z", "start", noon),
                usage_entry("2026-03-09T03:59:59.999Z", "last", noon),
                usage_entry("2026-03-09T04:00:00Z", "end", noon),
            );
            sessions(
                base,
                &resolved("/project"),
                &[("2026-03-09T12-00-00-000Z_a.jsonl", &journal)],
            );
            let run = report(base, &["--days", "7"]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout
                    .contains("Period: 2026-03-03 through 2026-03-09 (7 local days)"),
                "{}",
                run.stdout
            );
            assert!(
                run.stdout.contains("2026-03-03       messages:     1"),
                "{}",
                run.stdout
            );
            assert!(
                run.stdout.contains("2026-03-08       messages:     1"),
                "{}",
                run.stdout
            );
            assert!(
                run.stdout.contains("2026-03-09       messages:     1"),
                "{}",
                run.stdout
            );
            assert!(
                run.stdout.contains("TOTAL            messages:     3"),
                "{}",
                run.stdout
            );
        },
    );
}

#[test]
fn usage_timestamp_presence_precedes_entry_timestamp() {
    with_environment(
        "usage_timestamp_presence_precedes_entry_timestamp",
        &[("TZ", ZONE)],
        |base| {
            let journal = "{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"nulled\",\"timestamp\":null,\"usage\":{\"input\":1}}}\n{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"inherited\",\"usage\":{\"input\":2}}}\n";
            sessions(
                base,
                &resolved("/project"),
                &[("2026-03-09T12-00-00-000Z_a.jsonl", journal)],
            );
            let run = report(base, &[]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout.contains("TOTAL            messages:     1"),
                "{}",
                run.stdout
            );
            assert!(run.stdout.contains("  inherited"), "{}", run.stdout);
            assert!(!run.stdout.contains("nulled"), "{}", run.stdout);
        },
    );
}

#[test]
fn usage_aggregates_reported_totals_and_distinct_sessions() {
    with_environment(
        "usage_aggregates_reported_totals_and_distinct_sessions",
        &[("TZ", ZONE)],
        |base| {
            let totals = Totals {
                input: 1000.5,
                output: 2.0,
                cache_read: 3.0,
                cache_write: 4.0,
                total_tokens: 123.0,
                cost_total: 9.0,
            };
            let journal = format!(
                "{}{}",
                usage_entry("2026-03-09T12:00:00Z", "one", totals),
                usage_entry("2026-03-09T12:00:00Z", "one", totals),
            );
            let other = usage_entry("2026-03-09T12:00:00Z", "two", totals);
            let directory = sessions(
                base,
                &resolved("/project"),
                &[
                    ("2026-03-09T12-00-00-000Z_a.jsonl", &journal),
                    ("2026-03-09T12-00-00-000Z_b.jsonl", &other),
                ],
            );
            let run = report(base, &[]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout.contains(&expected(
                    "TOTAL            messages:     3  sessions:   2  input:        3,002  output:          6  cache read:             9  cache write:         12  total:           369  cost:   $27.0000",
                    base
                )),
                "{}",
                run.stdout
            );
            assert!(run.stdout.contains(&directory), "{}", run.stdout);
        },
    );
}

#[test]
fn usage_ignores_nonassistant_and_missing_usage() {
    with_environment(
        "usage_ignores_nonassistant_and_missing_usage",
        &[("TZ", ZONE)],
        |base| {
            let journal = "malformed\n\n{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"user\",\"provider\":\"user\",\"usage\":{\"input\":5}}}\n{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"absent\"}}\n{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"empty\",\"usage\":{}}}\n";
            sessions(
                base,
                &resolved("/project"),
                &[("2026-03-09T12-00-00-000Z_a.jsonl", journal)],
            );
            let run = report(base, &[]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout.contains("TOTAL            messages:     1"),
                "{}",
                run.stdout
            );
            assert!(run.stdout.contains("  empty"), "{}", run.stdout);
            assert!(!run.stdout.contains("absent"), "{}", run.stdout);
            assert!(!run.stdout.contains("  user"), "{}", run.stdout);
        },
    );
}

#[test]
fn usage_formats_grouped_signed_counts_and_costs() {
    with_environment(
        "usage_formats_grouped_signed_counts_and_costs",
        &[("TZ", ZONE)],
        |base| {
            let rounded = Totals {
                input: 1_234_567.1,
                output: -0.4,
                cache_read: 0.5,
                cache_write: 1.5,
                total_tokens: 2.5,
                cost_total: -0.25,
            };
            let journal = usage_entry("2026-03-09T12:00:00Z", "\u{e000}", rounded);
            let other = usage_entry("2026-03-09T12:00:00Z", "\u{1f600}", rounded);
            sessions(
                base,
                &resolved("/project"),
                &[
                    ("2026-03-09T12-00-00-000Z_a.jsonl", &journal),
                    ("2026-03-09T12-00-00-000Z_b.jsonl", &other),
                ],
            );
            let run = report(base, &[]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout.contains(&expected(
                    "TOTAL            messages:     2  sessions:   2  input:    2,469,134  output:         -1  cache read:             1  cache write:          3  total:             5  cost:   $-0.5000",
                    base
                )),
                "{}",
                run.stdout
            );
            let emoji = run
                .stdout
                .find('\u{1f600}')
                .expect("the supplementary label is reported");
            let private = run
                .stdout
                .find('\u{e000}')
                .expect("the private-use label is reported");
            assert!(emoji < private, "{}", run.stdout);
        },
    );
}

#[test]
fn usage_empty_and_missing_directories() {
    with_environment(
        "usage_empty_and_missing_directories",
        &[("TZ", ZONE)],
        |base| {
            sessions(base, &resolved("/project"), &[]);
            let run = report(base, &[]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout.contains("TOTAL            messages:     0"),
                "{}",
                run.stdout
            );
            let sessions_base = home(base, "sessions");
            let absent = maestro_session::sessions_dir(&sessions_base, &resolved("/absent"));
            let run = run_usage(&["--cwd", "/absent", "--sessions-base", &sessions_base])
                .expect("the report runs");
            assert!(run.failed());
            assert_eq!(
                run.stderr,
                format!("Sessions directory not found: {}\n", absent.as_str())
            );
        },
    );
}

#[test]
fn usage_directory_lookup_matches_session_writer() {
    with_environment(
        "usage_directory_lookup_matches_session_writer",
        &[("TZ", ZONE)],
        |base| {
            let agent = format!("{base}/home/.maestro/agent");
            for cwd in ["/project/work", "/project/work:two", "/project/work\\two"] {
                let directory = maestro_session::session_manager::get_default_session_dir(
                    &resolved(cwd),
                    &agent,
                )
                .expect("the session writer creates its directory");
                assert!(std::path::Path::new(&directory).is_dir(), "{directory}");
                std::fs::write(
                    std::path::Path::new(&directory).join("2026-03-09T12-00-00-000Z_a.jsonl"),
                    usage_entry(
                        "2026-03-09T12:00:00Z",
                        "writer",
                        Totals {
                            input: 1.0,
                            output: 0.0,
                            cache_read: 0.0,
                            cache_write: 0.0,
                            total_tokens: 0.0,
                            cost_total: 0.0,
                        },
                    ),
                )
                .expect("the fixture journal is writable");
                let sessions_base = format!("{base}/home/.maestro/agent/sessions");
                let run = run_usage(&["--cwd", cwd, "--sessions-base", &sessions_base])
                    .expect("the report runs");
                assert!(run.succeeded(), "{cwd}: {}", run.stderr);
                assert!(run.stdout.contains("  writer"), "{cwd}: {}", run.stdout);
            }
        },
    );
}
