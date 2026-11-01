//! Recorded-cost report behaviour.
#![cfg(all(test, not(target_arch = "wasm32")))]

use crate::analysis_fixtures::{ZONE, run_cost, sessions};
use crate::isolated_home::with_environment;

const HELP: &str = "Usage: cost.ts -d <path> -n <days>
  -d, --dir <path>   Directory path (required)
  -n, --days <num>   Number of days to track (required)
  -h, --help         Show this help
";
const REQUIRED: &str = "Error: both --dir and --days are required\nRun with --help for usage\n";

/// An assistant entry with a supplied provider and cost amount.
fn entry(timestamp: &str, provider: &str, total: f64) -> String {
    format!(
        "{{\"type\":\"message\",\"timestamp\":\"{timestamp}\",\"message\":{{\"role\":\"assistant\",\"provider\":\"{provider}\",\"usage\":{{\"cost\":{{\"total\":{total}}}}}}}}}\n"
    )
}

#[test]
fn cost_arguments_keep_required_and_help_branches() {
    with_environment(
        "cost_arguments_keep_required_and_help_branches",
        &[("TZ", ZONE)],
        |base| {
            sessions(base, "/project", &[]);
            for args in [
                vec![],
                vec!["-d", "/project"],
                vec!["-d", "/project", "-n", "0"],
                vec!["-d", "/project", "-n", "bad"],
                vec!["-d", "/project", "-n"],
                vec!["-d", ""],
                vec!["-d", "-n", "7"],
                vec!["-n", "7"],
                vec!["-d", "/project", "-n", "0x10"],
            ] {
                let run = run_cost(&args).expect("the report runs");
                assert_eq!(run.stderr, REQUIRED, "{args:?}");
                assert!(run.failed(), "{args:?}");
            }
            for args in [vec!["--help"], vec!["-h"], vec!["-d", "/project", "--help"]] {
                let run = run_cost(&args).expect("the report runs");
                assert_eq!(run.stdout, HELP, "{args:?}");
                assert!(run.succeeded(), "{args:?}");
            }
            for args in [
                vec!["-d", "/project", "-n", "7tail"],
                vec!["--dir", "/project", "--days", "7"],
                vec!["--unknown", "ignored", "-d", "/project", "-n", "7"],
                vec!["-d", "/project", "-n", "7", "-n", "7"],
            ] {
                let run = run_cost(&args).expect("the report runs");
                assert_eq!(
                    run.stdout, "No sessions found in the last 7 days for: /project\n",
                    "{args:?}"
                );
                assert!(run.succeeded(), "{args:?}");
            }
            let run = run_cost(&["-d", "--help", "-n", "7"]).expect("the report runs");
            assert_eq!(
                run.stderr,
                format!(
                    "Sessions directory not found: {}\n",
                    maestro_path::join(&[base, "home/.maestro/agent/sessions/----help--"])
                )
            );
            assert!(run.failed());
        },
    );
}

#[test]
fn cost_filename_cutoff_uses_local_midnight() {
    with_environment(
        "cost_filename_cutoff_uses_local_midnight",
        &[("TZ", ZONE)],
        |base| {
            if !maestro_test_process::local_zone_follows_tz() {
                return;
            }
            let older = entry("2026-03-09T12:00:00Z", "older", 1.0);
            let cutoff = entry("2026-03-09T12:00:00Z", "cutoff", 1.0);
            let before = entry("2026-03-09T12:00:00Z", "before", 1.0);
            let undated = entry("2026-03-09T12:00:00Z", "undated", 1.0);
            let future = entry("2026-03-09T12:00:00Z", "future", 1.0);
            sessions(
                base,
                "/project",
                &[
                    ("2026-03-07T23-00-00-000Z_a.jsonl", &older),
                    ("2026-03-08T05-00-00-000Z_b.jsonl", &cutoff),
                    ("2026-03-08T04-59-59-999Z_c.jsonl", &before),
                    ("not-a-date_d.jsonl", &undated),
                    ("2026-04-01T00-00-00-000Z_e.jsonl", &future),
                ],
            );
            let run = run_cost(&["-d", "/project", "-n", "1"]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert_eq!(
                run.stdout,
                "\nCost breakdown for: /project
Period: last 1 days (since 2026-03-08)
================================================================================

2026-03-09
----------------------------------------
  cutoff          $  1.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  future          $  1.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  undated         $  1.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  Day total:      $  3.0000

================================================================================
TOTALS BY PROVIDER
----------------------------------------
  cutoff          $  1.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  future          $  1.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  undated         $  1.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
----------------------------------------
  GRAND TOTAL:    $  3.0000

"
            );
        },
    );
}

#[test]
fn cost_groups_entry_days_and_providers() {
    with_environment(
        "cost_groups_entry_days_and_providers",
        &[("TZ", ZONE)],
        |base| {
            let journal = format!(
                "{}{}{}{}",
                entry("2026-03-09T02:30:00Z", "zeta", 0.2345),
                entry("2026-03-08T23:30:00Z", "alpha", 2.0),
                entry("2026-03-09T02:30:00Z", "__proto__", 5.0),
                entry("2026-03-09T03:00:00Z", "toString", 13.7655),
            );
            sessions(
                base,
                "/project",
                &[("2026-03-09T12-00-00-000Z_a.jsonl", &journal)],
            );
            let run = run_cost(&["-d", "/project", "-n", "7"]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert_eq!(
                run.stdout,
                "\nCost breakdown for: /project
Period: last 7 days (since 2026-03-02)
================================================================================

2026-03-08
----------------------------------------
  alpha           $  2.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  Day total:      $  2.0000

2026-03-09
----------------------------------------
  __proto__       $  5.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  toString        $ 13.7655  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  zeta            $  0.2345  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  Day total:      $ 19.0000

================================================================================
TOTALS BY PROVIDER
----------------------------------------
  __proto__       $  5.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  alpha           $  2.0000  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  toString        $ 13.7655  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
  zeta            $  0.2345  (1 reqs, in: $0.0000, out: $0.0000, cache: $0.0000)
----------------------------------------
  GRAND TOTAL:    $ 21.0000

"
            );
        },
    );
}

#[test]
fn cost_partial_costs_and_malformed_lines() {
    with_environment(
        "cost_partial_costs_and_malformed_lines",
        &[("TZ", ZONE)],
        |base| {
            let journal = "not json
{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"absent\",\"usage\":{}}}
{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"nulled\",\"usage\":{\"cost\":null}}}
{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"user\",\"provider\":\"user\",\"usage\":{\"cost\":{\"total\":1}}}}
{\"type\":\"other\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"other\",\"usage\":{\"cost\":{\"total\":1}}}}
{\"type\":\"message\",\"timestamp\":\"bad date\",\"message\":{\"role\":\"assistant\",\"provider\":\"undated\",\"usage\":{\"cost\":{\"total\":1}}}}
{\"type\":\"message\",\"timestamp\":\"2026-03-09T12:00:00Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"bare\",\"usage\":{\"cost\":{}}}}
";
            sessions(
                base,
                "/project",
                &[("2026-03-09T12-00-00-000Z_a.jsonl", journal)],
            );
            let run = run_cost(&["-d", "/project", "-n", "7"]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout.contains("  bare            $  0.0000  (1 reqs"),
                "{}",
                run.stdout
            );
            assert!(
                run.stdout.contains("GRAND TOTAL:    $  0.0000"),
                "{}",
                run.stdout
            );
            for absent in ["absent", "nulled", "user", "other", "undated"] {
                assert!(!run.stdout.contains(absent), "{absent}: {}", run.stdout);
            }
        },
    );
}

#[test]
fn cost_rows_keep_decimal_rounding_and_utf16_padding() {
    with_environment(
        "cost_rows_keep_decimal_rounding_and_utf16_padding",
        &[("TZ", ZONE)],
        |base| {
            let journal = format!(
                "{}{}{}{}",
                entry("2026-03-09T12:00:00Z", "negative", -0.00005),
                entry("2026-03-09T12:00:00Z", "large", 1e21),
                entry("2026-03-09T12:00:00Z", "\u{1f600}", 0.00005),
                entry("2026-03-09T12:00:00Z", "\u{e000}", 1.00005),
            );
            sessions(
                base,
                "/project",
                &[("2026-03-09T12-00-00-000Z_a.jsonl", &journal)],
            );
            let run = run_cost(&["-d", "/project", "-n", "7"]).expect("the report runs");
            assert!(run.succeeded(), "{}", run.stderr);
            assert!(
                run.stdout.contains("  negative        $ -0.0001"),
                "{}",
                run.stdout
            );
            assert!(
                run.stdout.contains("  large           $   1e+21"),
                "{}",
                run.stdout
            );
            assert!(run.stdout.contains("$  0.0001"), "{}", run.stdout);
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
fn cost_empty_and_missing_directories() {
    with_environment(
        "cost_empty_and_missing_directories",
        &[("TZ", ZONE)],
        |base| {
            sessions(base, "/project", &[]);
            let run = run_cost(&["-d", "/absent", "-n", "7"]).expect("the report runs");
            assert!(run.failed());
            assert_eq!(
                run.stderr,
                format!(
                    "Sessions directory not found: {}\n",
                    maestro_path::join(&[base, "home/.maestro/agent/sessions/--absent--"])
                )
            );
            let run = run_cost(&["-d", "/project", "-n", "7"]).expect("the report runs");
            assert!(run.succeeded());
            assert_eq!(
                run.stdout,
                "No sessions found in the last 7 days for: /project\n"
            );
            let directory = sessions(
                base,
                "/project",
                &[(
                    "2026-03-09T12-00-00-000Z_a.jsonl",
                    "{\"type\":\"message\"}\n",
                )],
            );
            std::fs::create_dir_all(
                std::path::Path::new(&directory).join("2026-03-09T13-00-00-000Z_b.jsonl"),
            )
            .expect("the fixture directory is creatable");
            let error = run_cost(&["-d", "/project", "-n", "7"])
                .expect_err("a reached read failure rejects the report");
            // The host's own refusal: Unix reports a directory, Windows denies access.
            let refused = std::fs::read(
                std::path::Path::new(&directory).join("2026-03-09T13-00-00-000Z_b.jsonl"),
            )
            .expect_err("a directory is not readable as a file")
            .kind();
            assert_eq!(error.kind(), refused);
        },
    );
}

#[test]
fn cost_directory_lookup_matches_session_writer() {
    with_environment(
        "cost_directory_lookup_matches_session_writer",
        &[("TZ", ZONE)],
        |base| {
            let agent = format!("{base}/home/.maestro/agent");
            for cwd in ["/project/work", "/project/work:two", "/project/work\\two"] {
                let directory =
                    maestro_session::session_manager::get_default_session_dir(cwd, &agent)
                        .expect("the session writer creates its directory");
                std::fs::write(
                    std::path::Path::new(&directory).join("2026-03-09T12-00-00-000Z_a.jsonl"),
                    entry("2026-03-09T12:00:00Z", "writer", 1.0),
                )
                .expect("the fixture journal is writable");
                let run = run_cost(&["-d", cwd, "-n", "7"]).expect("the report runs");
                assert!(run.succeeded(), "{cwd}: {}", run.stderr);
                assert!(run.stdout.contains("  writer "), "{cwd}: {}", run.stdout);
            }
        },
    );
}
