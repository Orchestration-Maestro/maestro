//! Recorded standalone formatting operations.
mod workbooks;
use maestro_app::browser::ssf::{CellValue, FormatOptions, NumberFormat, Ssf};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    op: String,
    args: Value,
    expected: Value,
}

fn selector(value: &Value) -> NumberFormat {
    match value {
        Value::String(s) => NumberFormat::Code(s.clone()),
        _ => NumberFormat::Id(u32::try_from(value.as_u64().unwrap()).unwrap()),
    }
}

fn scalar(value: &Value) -> CellValue {
    if let Some(n) = value.get("number") {
        CellValue::Number(n.as_str().unwrap().parse().unwrap())
    } else if let Some(s) = value.get("text") {
        CellValue::Text(s.as_str().unwrap().to_owned())
    } else if let Some(b) = value.get("boolean") {
        CellValue::Boolean(b.as_bool().unwrap())
    } else if let Some(date) = value.get("date") {
        CellValue::Date(
            chrono::DateTime::parse_from_rfc3339(date.as_str().unwrap())
                .unwrap()
                .to_utc(),
        )
    } else if value["empty"] == "null" {
        CellValue::Null
    } else {
        assert_eq!(value["empty"], "missing");
        CellValue::Missing
    }
}

pub fn run(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/ssf/completion.json")).unwrap();
    let map: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_str(include_str!("../fixtures/ssf/completion-map.json")).unwrap();
    let ids = map
        .iter()
        .find(|(key, _)| key.ends_with(&format!("::{name}")))
        .unwrap()
        .1;
    for id in ids {
        let case = cases.iter().find(|case| &case.id == id).unwrap();
        let actual = match case.op.as_str() {
            "format" | "format_seeded" => observe_format(&case.args),
            "table_sequence" => observe_table(&case.args),
            "load_sequence" => load_empty(&case.args),
            "owner_load_table" => load_owner(&case.args),
            "owner_format" => format_owner(&case.args),
            "owner_format_pair" => owner_pair(&case.args),
            "format_cell" | "format_cell_seeded" => observe_cell(&case.args),
            "read" => workbooks::observe(&case.args),
            "read_seeded" => workbooks::seeded(&case.args, false),
            "read_reset" => workbooks::seeded(&case.args, true),
            "read_format_codes" => workbooks::format_codes(&case.args),
            _ => panic!("unsupported operation {}", case.op),
        };
        assert_eq!(actual, case.expected, "{id}");
    }
}

fn outcome(result: Result<String, maestro_app::browser::ssf::FormatError>) -> Value {
    use maestro_app::browser::ssf::FormatError;
    use serde_json::json;
    match result {
        Ok(s) => json!({"ok": s}),
        Err(FormatError::TooManySections) => json!({"error": {"kind": "too_many_sections"}}),
        Err(FormatError::UnterminatedBlock) => json!({"error": {"kind": "unterminated_block"}}),
        Err(FormatError::UnterminatedString) => json!({"error": {"kind": "unterminated_string"}}),
        Err(FormatError::UnsupportedNumberFormat) => {
            json!({"error": {"kind": "unsupported_number_format"}})
        }
        Err(FormatError::UnrecognizedCharacter(c)) => {
            json!({"error": {"kind": "unrecognized_character", "character": c}})
        }
        Err(FormatError::InvalidDateFormat(_)) => panic!("unexpected calendar error"),
    }
}

fn entries(value: &Value) -> std::collections::BTreeMap<u32, String> {
    value
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| (key.parse().unwrap(), value.as_str().unwrap().to_owned()))
        .collect()
}

fn observe_format(args: &Value) -> Value {
    let ssf = Ssf::default();
    if let Some(seed) = args.get("seed") {
        ssf.get_table().write().unwrap().extend(entries(seed));
    }
    let table = args["options"]
        .get("table")
        .map(|value| std::sync::Arc::new(std::sync::RwLock::new(entries(value))));
    let options = FormatOptions {
        date1904: args["options"]["date1904"].as_bool().unwrap_or(false),
        date_nf: args["options"]["dateNF"].as_str(),
        table: table.as_ref(),
    };
    outcome(ssf.format(
        &selector(&args["format"]),
        &scalar(&args["value"]),
        &options,
    ))
}

fn observe_table(args: &Value) -> Value {
    use serde_json::json;
    let ssf = Ssf::default();
    let table = ssf.get_table();
    let observations: Vec<Value> = args["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| match step["kind"].as_str().unwrap() {
            "load" => json!(ssf.load(
                step["format"].as_str().unwrap(),
                step["index"].as_u64().map(|n| u32::try_from(n).unwrap())
            )),
            "format" => json!(
                ssf.format(
                    &selector(&step["format"]),
                    &CellValue::Number(step["value"].as_f64().unwrap()),
                    &FormatOptions::default()
                )
                .unwrap()
            ),
            "mutate" => {
                table.write().unwrap().insert(
                    u32::try_from(step["index"].as_u64().unwrap()).unwrap(),
                    step["format"].as_str().unwrap().to_owned(),
                );
                Value::Null
            }
            "fill" => {
                table
                    .write()
                    .unwrap()
                    .extend((0..392).map(|n| (n, format!("fmt-{n}"))));
                Value::Null
            }
            "load_table" => {
                let supplied = std::sync::Arc::new(std::sync::RwLock::new(entries(&step["table"])));
                ssf.load_table(&supplied);
                Value::Null
            }
            _ => panic!("unknown table operation"),
        })
        .collect();
    let entries: Vec<_> = table
        .read()
        .unwrap()
        .iter()
        .map(|(n, s)| (*n, s.clone()))
        .collect();
    json!({"ok": {"observations": observations, "same": std::sync::Arc::ptr_eq(&table, &ssf.get_table()), "entries": entries}})
}

fn owner_pair(args: &Value) -> Value {
    use std::sync::{Arc, Barrier, mpsc};
    let left = Ssf::default();
    let right = Ssf::default();
    left.get_table()
        .write()
        .unwrap()
        .extend(entries(&args["left"]));
    right
        .get_table()
        .write()
        .unwrap()
        .extend(entries(&args["right"]));
    let left_table = left.get_table();
    let right_table = right.get_table();
    let same = Arc::ptr_eq(&left_table, &right_table);
    let start = Arc::new(Barrier::new(3));
    let (tx, rx) = mpsc::channel();
    let jobs: Vec<_> = [(left, right_table), (right, left_table)]
        .into_iter()
        .enumerate()
        .map(|(i, (ssf, table))| {
            let start = Arc::clone(&start);
            let tx = tx.clone();
            let format = selector(&args["format"]);
            let value = CellValue::Number(args["values"][i].as_f64().unwrap());
            std::thread::spawn(move || {
                start.wait();
                let text = ssf
                    .format(
                        &format,
                        &value,
                        &FormatOptions {
                            table: Some(&table),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                tx.send((i, text)).unwrap();
            })
        })
        .collect();
    start.wait();
    drop(tx);
    let mut texts = [String::new(), String::new()];
    for (i, text) in rx {
        texts[i] = text;
    }
    for job in jobs {
        job.join().unwrap();
    }
    serde_json::json!({"ok": {"same": same, "texts": texts}})
}

fn observe_cell(args: &Value) -> Value {
    use maestro_app::browser::ssf::{
        CellFormatOptions, CellObject, ExcelDataType, Xf, format_cell,
    };
    use serde_json::json;
    let ssf = Ssf::default();
    if let Some(seed) = args.get("seed") {
        ssf.get_table().write().unwrap().extend(entries(seed));
    }
    let input = &args["cell"];
    let kind = input["t"].as_str().map(|s| match s {
        "n" => ExcelDataType::Number,
        "s" => ExcelDataType::Text,
        "b" => ExcelDataType::Boolean,
        "d" => ExcelDataType::Date,
        "e" => ExcelDataType::Error,
        "z" => ExcelDataType::Stub,
        _ => panic!("unknown cell classification"),
    });
    let mut cell = CellObject {
        t: kind,
        v: scalar(&input["v"]),
        w: input["w"].as_str().map(str::to_owned),
        z: input.get("z").map(selector),
        xf: input.get("XF").map(|xf| Xf {
            num_fmt_id: u32::try_from(xf["numFmtId"].as_u64().unwrap()).unwrap(),
        }),
        ..Default::default()
    };
    let value = scalar(&args["value"]);
    let date_nf = args["options"].get("dateNF").map(selector);
    let text = format_cell(
        Some(&mut cell),
        Some(&value),
        &CellFormatOptions {
            date_nf: date_nf.as_ref(),
        },
        &ssf,
    );
    assert_eq!(cell.v, scalar(&input["v"]));
    let shown = match text {
        CellValue::Text(s) => json!(s),
        CellValue::Number(n) => serde_json::from_str(&n.to_string()).unwrap(),
        _ => panic!("unexpected return value"),
    };
    let z = cell.z.as_ref().map(|z| match z {
        NumberFormat::Code(s) => json!(s),
        NumberFormat::Id(id) => json!(id),
    });
    json!({"ok": {"text": shown, "w": cell.w, "z": z, "v_after": encoded_scalar(&cell.v)}})
}

fn encoded_scalar(value: &CellValue) -> Value {
    use serde_json::json;
    match value {
        CellValue::Number(n) => json!({"number": n.to_string()}),
        CellValue::Text(s) => json!({"text": s}),
        CellValue::Boolean(b) => json!({"boolean": b}),
        CellValue::Date(date) => {
            json!({"date": date.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)})
        }
        CellValue::Null => json!({"empty": "null"}),
        CellValue::Missing => json!({"empty": "missing"}),
    }
}

fn load_empty(args: &Value) -> Value {
    let ssf = Ssf::default();
    let steps = args["steps"].as_array().unwrap();
    let explicit = ssf.load(steps[0]["format"].as_str().unwrap(), Some(164));
    assert_eq!(steps[0]["index"], 164);
    assert!(steps[1]["index"].is_null());
    let auto = ssf.load(steps[1]["format"].as_str().unwrap(), None);
    let code = ssf.get_table().read().unwrap().get(&auto).cloned();
    let text = ssf
        .format(
            &NumberFormat::Id(auto),
            &CellValue::Number(7.0),
            &FormatOptions::default(),
        )
        .unwrap();
    serde_json::json!({"ok": {"explicit": explicit, "auto": auto, "value": code, "text": text}})
}

fn load_owner(args: &Value) -> Value {
    assert_eq!(args["table"], "same");
    let ssf = Ssf::default();
    let table = ssf.get_table();
    table.write().unwrap().extend(entries(&args["seed"]));
    ssf.load_table(&table);
    let text = ssf
        .format(
            &NumberFormat::Id(164),
            &CellValue::Number(7.0),
            &FormatOptions::default(),
        )
        .unwrap();
    let values = table.read().unwrap();
    serde_json::json!({"ok": {"same": std::sync::Arc::ptr_eq(&table, &ssf.get_table()), "custom": values.get(&164), "outside": values.get(&392), "text": text}})
}

fn format_owner(args: &Value) -> Value {
    assert_eq!(args["table"], "same");
    let ssf = Ssf::default();
    let table = ssf.get_table();
    table.write().unwrap().extend(entries(&args["seed"]));
    let options = FormatOptions {
        table: Some(&table),
        ..Default::default()
    };
    let text = ssf
        .format(
            &selector(&args["format"]),
            &scalar(&args["value"]),
            &options,
        )
        .unwrap();
    serde_json::json!({"ok": {"same": std::sync::Arc::ptr_eq(&table, &ssf.get_table()), "text": text}})
}
