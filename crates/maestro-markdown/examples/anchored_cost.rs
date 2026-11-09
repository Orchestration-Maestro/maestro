//! Release-mode measurement of parse time against paragraph length.
//!
//! Run `cargo run --release -p maestro-markdown --example anchored_cost`. A paragraph of
//! plain sentences starts none of the start-anchored recognizers, so every reached cursor
//! makes each of them miss. Doubling the sentences should about double the time; a ratio near
//! four means a miss costs the length of the text after the cursor.
use maestro_markdown::{MarkedOptions, parse_with_options};
use std::hint::black_box;
use std::time::Instant;

/// Timed runs per paragraph; the fastest one discards scheduler noise.
const TRIALS: usize = 15;

/// Sentence counts, each double the one before.
const SENTENCES: [usize; 4] = [800, 1600, 3200, 6400];

/// One paragraph of `sentences` plain sentences.
fn paragraph(sentences: usize) -> String {
    (0..sentences)
        .flat_map(|index| {
            [
                "step ".to_owned(),
                index.to_string(),
                " takes about ~5 min, ".to_owned(),
            ]
        })
        .collect()
}

/// Microseconds the fastest of several parses of `source` takes.
fn fastest_micros(source: &str, options: MarkedOptions) -> u128 {
    (0..TRIALS)
        .map(|_| {
            let text = black_box(source.to_owned());
            let start = Instant::now();
            black_box(parse_with_options(text, options));
            start.elapsed().as_micros()
        })
        .min()
        .unwrap_or_default()
}

fn main() {
    let grammars = [
        ("browser", MarkedOptions::Browser { escape_html: false }),
        ("terminal", MarkedOptions::Terminal),
    ];
    for (name, options) in grammars {
        let mut previous: Option<u128> = None;
        for sentences in SENTENCES {
            let source = paragraph(sentences);
            let micros = fastest_micros(&source, options);
            let growth = previous.map_or_else(String::new, |before| {
                let hundredths = micros * 100 / before.max(1);
                format!(" growth={}.{:02}x", hundredths / 100, hundredths % 100)
            });
            println!(
                "{name} sentences={sentences} bytes={} best_us={micros}{growth}",
                source.len()
            );
            previous = Some(micros);
        }
    }
}
