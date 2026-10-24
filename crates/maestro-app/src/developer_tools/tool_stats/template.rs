//! The generated tool report page: emitted data, brand styling and script.

use super::{Report, bucket_labels, histogram};
use maestro_text::markup::{EscapeMode, escape};
use maestro_theme::{BrandMode, BrandPack};
use num_traits::ToPrimitive;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io;

/// The shared identity pack the page takes its colors and fonts from.
const BRAND: &str = include_str!("../../../../../assets/brand/brand.json");
/// The pack's own path, used only to place its mark files.
const BRAND_PATH: &str = "assets/brand/brand.json";
/// The two pinned page libraries, written as script sources and not bundled.
const TAILWIND: &str = "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4";
/// The chart library.
const CHART: &str = "https://cdn.jsdelivr.net/npm/chart.js@4.4.9/dist/chart.umd.min.js";

/// The complete page text for one report run.
///
/// # Errors
/// Returns the brand pack's rejection.
pub(super) fn render(
    report: &Report,
    sessions: &str,
    now: chrono::DateTime<chrono::Local>,
) -> io::Result<String> {
    let css = properties().map_err(io::Error::other)?;
    let data = data(report, sessions, now);
    let stamp = data
        .get("generatedAt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut page = head(&css);
    page.push_str(&body(
        &escape(sessions, EscapeMode::Xml),
        report.files,
        stamp,
    ));
    page.push_str(&script(&compact(&data), &Paint::of(&css)));
    Ok(page)
}

/// The page's colors, fonts and radii as custom properties.
fn properties() -> Result<BTreeMap<String, String>, String> {
    let pack = BrandPack::parse(BRAND_PATH, BRAND).map_err(|error| error.to_string())?;
    Ok(pack
        .presentation(BrandMode::Dark)
        .map_err(|error| error.to_string())?
        .css_properties())
}

/// The document head, including the brand declarations.
fn head(css: &BTreeMap<String, String>) -> String {
    let declarations = css
        .iter()
        .map(|(name, value)| format!("{name}:{value}"))
        .collect::<Vec<_>>()
        .join(";");
    format!(
        "<!doctype html>\n<html>\n<head>\n\t<meta charset=\"utf-8\">\n\t<title>Maestro Tool Stats</title>\n\t<script src=\"{TAILWIND}\"></script>\n\t<script src=\"{CHART}\"></script>\n\t<style>\n\t\t:root{{{declarations}}}\n\t\tbody{{background:var(--maestro-color-background);color:var(--maestro-color-text);font-family:var(--maestro-font-body)}}\n\t\th1,h2{{font-family:var(--maestro-font-display)}}\n\t\tcode{{font-family:var(--maestro-font-mono)}}\n\t\t.card{{background:var(--maestro-color-panel)}}\n\t\t.muted{{color:var(--maestro-color-muted)}}\n\t\t.select{{background:var(--maestro-color-panel);color:var(--maestro-color-text)}}\n\t\t#tools tbody tr[data-row]:hover,#bash tbody tr[data-row]:hover{{background:color-mix(in srgb,var(--maestro-color-border) 50%,transparent)}}\n\t\t#tools th,#bash th{{color:var(--maestro-color-muted)}}\n\t\t#tools tbody tr[data-row],#bash tbody tr[data-row]{{border-color:var(--maestro-color-border)}}\n\t\t.mono{{font-family:var(--maestro-font-mono)}}\n\t</style>\n</head>\n"
    )
}

/// The page's document body with the session summary line.
fn body(sessions: &str, files: usize, stamp: &str) -> String {
    format!(
        "<body class=\"p-6\">\n\t<main class=\"max-w-7xl mx-auto space-y-6\">\n\t\t<h1 class=\"text-3xl font-bold\">Maestro Tool Stats</h1>\n\t\t<p class=\"muted\">{files} session files from <code>{sessions}</code>. Generated {stamp}.</p>\n\t\t<section class=\"grid md:grid-cols-2 gap-6\">\n\t\t\t<div class=\"card rounded p-4\"><h2 class=\"font-semibold mb-3\">Estimated result tokens by tool</h2><canvas id=\"tokens\"></canvas></div>\n\t\t\t<div class=\"card rounded p-4\"><h2 class=\"font-semibold mb-3\">Tool calls</h2><canvas id=\"calls\"></canvas></div>\n\t\t</section>\n\t\t<section class=\"grid md:grid-cols-2 gap-6\">\n\t\t\t<div class=\"card rounded p-4\">\n\t\t\t\t<div class=\"flex items-center justify-between gap-4 mb-3\">\n\t\t\t\t\t<h2 class=\"font-semibold\">Tool result token histogram</h2>\n\t\t\t\t\t<select id=\"toolSelect\" class=\"select rounded px-2 py-1 text-sm\"></select>\n\t\t\t\t</div>\n\t\t\t\t<p id=\"toolSummary\" class=\"muted text-sm mb-3\"></p>\n\t\t\t\t<canvas id=\"toolHistogram\" height=\"120\"></canvas>\n\t\t\t</div>\n\t\t\t<div class=\"card rounded p-4\">\n\t\t\t\t<div class=\"flex items-center justify-between gap-4 mb-3\">\n\t\t\t\t\t<h2 class=\"font-semibold\">Bash result token histogram</h2>\n\t\t\t\t\t<select id=\"bashSelect\" class=\"select rounded px-2 py-1 text-sm\"></select>\n\t\t\t\t</div>\n\t\t\t\t<p id=\"bashSummary\" class=\"muted text-sm mb-3\"></p>\n\t\t\t\t<canvas id=\"bashHistogram\" height=\"120\"></canvas>\n\t\t\t</div>\n\t\t</section>\n\t\t<section class=\"card rounded p-4\"><h2 class=\"font-semibold mb-3\">Tools</h2><div id=\"tools\"></div></section>\n\t\t<section class=\"card rounded p-4\">\n\t\t\t<h2 class=\"font-semibold mb-3\">Bash common commands (best effort)</h2>\n\t\t\t<div id=\"bash\" class=\"mt-4\"></div>\n\t\t</section>\n\t</main>\n"
    )
}

/// The brand roles the page paints its charts and rows with.
struct Paint<'a> {
    /// Bar fill.
    accent: &'a str,
    /// Chart tick and legend text.
    muted: &'a str,
    /// Chart grid and axis lines.
    border: &'a str,
    /// Chart text family.
    font: &'a str,
}

impl<'a> Paint<'a> {
    /// Read the roles from the page's declared custom properties.
    fn of(css: &'a BTreeMap<String, String>) -> Self {
        let role = |name: &str| css.get(name).map(String::as_str).unwrap_or_default();
        Self {
            accent: role("--maestro-color-accent"),
            muted: role("--maestro-color-muted"),
            border: role("--maestro-color-border"),
            font: role("--maestro-font-body"),
        }
    }
}

/// The page's script: the emitted data and the table, selector and chart logic.
fn script(data: &str, paint: &Paint<'_>) -> String {
    let Paint {
        accent,
        muted,
        border,
        font,
    } = paint;
    let (accent, muted, border, font) = (json!(accent), json!(muted), json!(border), json!(font));
    format!(
        "\t<script>\n\t\tconst data={data};\n\t\tconst accent={accent};\n\t\tChart.defaults.color={muted};\n\t\tChart.defaults.borderColor={border};\n\t\tChart.defaults.font.family={font};\n\t\tfunction fmt(n){{return Math.round(n).toLocaleString()}}\n\t\tfunction esc(s){{return String(s).replace(/[&<>\\\"]/g,c=>({{'&':'&amp;','<':'&lt;','>':'&gt;','\\\"':'&quot;'}}[c]))}}\n\t\tfunction table(rows,el){{\n\t\t\tdocument.getElementById(el).innerHTML='<table class=\"w-full text-sm\"><thead><tr><th class=\"text-left p-2\">Name</th><th class=\"text-right p-2\">Calls</th><th class=\"text-right p-2\">Results</th><th class=\"text-right p-2\">Est. tokens</th><th class=\"text-right p-2\">Avg/result</th><th class=\"text-left p-2 w-64\">Histogram</th></tr></thead><tbody>'+rows.map((r,i)=>'<tr class=\"border-t cursor-pointer\" data-row=\"'+i+'\"><td class=\"p-2 mono\">'+esc(r.name)+'</td><td class=\"p-2 text-right\">'+fmt(r.calls)+'</td><td class=\"p-2 text-right\">'+fmt(r.results??r.samples.length)+'</td><td class=\"p-2 text-right\">'+fmt(r.estimatedTokens)+'</td><td class=\"p-2 text-right\">'+fmt(r.avg)+'</td><td class=\"p-2\"><canvas id=\"'+el+'Hist'+i+'\" height=\"34\"></canvas></td></tr>').join('')+'</tbody></table>';\n\t\t\trows.forEach((r,i)=>new Chart(document.getElementById(el+'Hist'+i),{{type:'bar',data:{{labels:data.bucketLabels,datasets:[{{data:r.histogram,label:r.name,backgroundColor:accent}}]}},options:{{responsive:true,maintainAspectRatio:false,plugins:{{legend:{{display:false}},tooltip:{{callbacks:{{title:(items)=>data.bucketLabels[items[0].dataIndex]+' tokens'}}}}}},scales:{{x:{{display:false}},y:{{display:false}}}}}}}}));\n\t\t\tdocument.querySelectorAll('#'+el+' tr[data-row]').forEach(row=>row.addEventListener('click',()=>{{const select=document.getElementById(el==='tools'?'toolSelect':'bashSelect');select.value=row.dataset.row;select.dispatchEvent(new Event('change'))}}));\n\t\t}}\n\t\tfunction fillSelect(id,rows){{document.getElementById(id).innerHTML=rows.map((r,i)=>'<option value=\"'+i+'\">'+esc(r.name)+'</option>').join('')}}\n\t\tfunction summary(r){{return fmt(r.calls)+' calls, '+fmt(r.results??r.samples.length)+' results, '+fmt(r.estimatedTokens)+' estimated result tokens, '+fmt(r.avg)+' avg/result'}}\n\t\tfunction singleHistogram(canvasId,summaryId,selectId,rows){{\n\t\t\tlet chart;\n\t\t\tfunction update(){{\n\t\t\t\tconst row=rows[Number(document.getElementById(selectId).value)]??rows[0];\n\t\t\t\tdocument.getElementById(summaryId).textContent=row?summary(row):'No data';\n\t\t\t\tif(chart)chart.destroy();\n\t\t\t\tchart=new Chart(document.getElementById(canvasId),{{type:'bar',data:{{labels:data.bucketLabels,datasets:[{{label:row?.name??'',data:row?.histogram??[],backgroundColor:accent}}]}},options:{{plugins:{{legend:{{display:false}},tooltip:{{callbacks:{{title:(items)=>data.bucketLabels[items[0].dataIndex]+' tokens',label:(item)=>fmt(item.raw)+' results'}}}}}},scales:{{y:{{beginAtZero:true,title:{{display:true,text:'result count'}}}},x:{{title:{{display:true,text:'estimated tokens/result'}}}}}}}}}});\n\t\t\t}}\n\t\t\tdocument.getElementById(selectId).addEventListener('change',update);\n\t\t\tupdate();\n\t\t}}\n\t\ttable(data.tools,'tools');\n\t\ttable(data.bashCommands,'bash');\n\t\tfillSelect('toolSelect',data.tools);\n\t\tfillSelect('bashSelect',data.bashCommands);\n\t\tsingleHistogram('toolHistogram','toolSummary','toolSelect',data.tools);\n\t\tsingleHistogram('bashHistogram','bashSummary','bashSelect',data.bashCommands);\n\t\tnew Chart(document.getElementById('tokens'),{{type:'bar',data:{{labels:data.tools.map(r=>r.name),datasets:[{{label:'estimated tokens',data:data.tools.map(r=>r.estimatedTokens),backgroundColor:accent}}]}},options:{{plugins:{{legend:{{display:false}}}}}}}});\n\t\tnew Chart(document.getElementById('calls'),{{type:'bar',data:{{labels:data.tools.map(r=>r.name),datasets:[{{label:'calls',data:data.tools.map(r=>r.calls),backgroundColor:accent}}]}},options:{{plugins:{{legend:{{display:false}}}}}}}});\n\t</script>\n</body>\n</html>\n"
    )
}

/// The report's data record, in the order the page reads it.
fn data(report: &Report, sessions: &str, now: chrono::DateTime<chrono::Local>) -> Value {
    let stamp = now
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    json!({
        "generatedAt": stamp,
        "sessionsDir": sessions,
        "files": report.files,
        "parseErrors": report.parse_errors,
        "bucketLabels": bucket_labels(),
        "tools": rows(report, false),
        "bashCommands": rows(report, true),
    })
}

/// The tool or bash rows, most tokens first and ties in first-seen order.
fn rows(report: &Report, bash: bool) -> Vec<Value> {
    let mut rows = if bash {
        report
            .bash
            .iter()
            .map(|(name, totals)| Emitted {
                name,
                calls: totals.calls,
                results: None,
                tokens: totals.tokens,
                samples: &totals.samples,
                errors: None,
            })
            .collect::<Vec<_>>()
    } else {
        report
            .tools
            .iter()
            .map(|(name, totals)| Emitted {
                name,
                calls: totals.calls,
                results: Some(totals.results),
                tokens: totals.tokens,
                samples: &totals.samples,
                errors: Some(totals.errors),
            })
            .collect::<Vec<_>>()
    };
    rows.sort_by_key(|row| std::cmp::Reverse(row.tokens));
    if bash {
        rows.truncate(super::BASH_ROWS);
    }
    rows.iter().map(row).collect()
}

/// The counters one emitted row reports.
struct Emitted<'a> {
    /// The tool or command name.
    name: &'a str,
    /// Recorded calls.
    calls: u64,
    /// Recorded results, absent for bash rows.
    results: Option<u64>,
    /// Estimated result tokens.
    tokens: u64,
    /// One estimate per result.
    samples: &'a [u64],
    /// Recorded errors, absent for bash rows.
    errors: Option<u64>,
}

/// One emitted row: its name, counters, samples, average and histogram.
fn row(emitted: &Emitted<'_>) -> Value {
    let Emitted {
        name,
        calls,
        results,
        tokens,
        samples,
        errors,
    } = emitted;
    let count = results.unwrap_or(samples.len() as u64);
    let average = match (tokens.to_f64(), count.to_f64()) {
        (Some(tokens), Some(count)) if count > 0.0 => tokens / count,
        _ => 0.0,
    };
    let mut row = serde_json::Map::new();
    row.insert("name".to_owned(), Value::String((*name).to_owned()));
    row.insert("calls".to_owned(), json!(calls));
    if let Some(results) = results {
        row.insert("results".to_owned(), json!(results));
    }
    row.insert("estimatedTokens".to_owned(), json!(tokens));
    row.insert("samples".to_owned(), json!(samples));
    if let Some(errors) = errors {
        row.insert("errors".to_owned(), json!(errors));
    }
    row.insert("avg".to_owned(), json!(average));
    row.insert("histogram".to_owned(), json!(histogram(samples)));
    Value::Object(row)
}

/// The compact JSON text of the data, safe to sit inside a script element.
fn compact(data: &Value) -> String {
    maestro_json::compact_json(data)
        .unwrap_or_default()
        .replace('<', "\\u003c")
}
