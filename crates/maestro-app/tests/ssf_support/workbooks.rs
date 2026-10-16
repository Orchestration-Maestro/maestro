//! Complete workbook observations from supplied byte fixtures.
use maestro_app::browser::ssf::{self, CellFormatOptions, CellValue, Ssf};
use serde_json::{Value, json};

pub(super) fn bytes(file: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workbooks")
            .join(file),
    )
    .unwrap()
}

pub(super) fn observe(args: &Value) -> Value {
    let file = args["file"].as_str().unwrap();
    let mut ssf = Ssf::default();
    match ssf::read(&bytes(file), file, &mut ssf) {
        Ok(book) => json!({"ok": project(book, &ssf, false)}),
        Err(error) => {
            assert!(matches!(error, ssf::WorkbookError::Reader(_)));
            assert!(error.to_string().contains(file));
            json!({"error": {"kind": "reader", "workbook": file}})
        }
    }
}

fn value(value: &CellValue, typed: bool) -> Value {
    match value {
        CellValue::Number(n) => json!({"number": n.to_string()}),
        CellValue::Text(s) if typed => json!({"text": s}),
        CellValue::Boolean(b) if typed => json!({"boolean": b}),
        CellValue::Text(s) => json!(s),
        CellValue::Boolean(b) => json!(b),
        CellValue::Missing => json!({"missing": true}),
        _ => panic!("unexpected workbook scalar"),
    }
}

pub(super) fn project(book: ssf::WorkBook, ssf: &Ssf, typed: bool) -> Value {
    let sheets: Vec<_> = book.sheets.into_iter().map(|(name, sheet)| {
        let cells: Vec<_> = sheet.cells.into_iter().map(|(address, mut cell)| {
            let kind = match cell.t.unwrap() {
                ssf::ExcelDataType::Number => "n", ssf::ExcelDataType::Text => "s",
                ssf::ExcelDataType::Boolean => "b", ssf::ExcelDataType::Error => "e",
                _ => panic!("unexpected cell type"),
            };
            let raw = value(&cell.v, typed);
            let before = cell.w.clone();
            let text = ssf::format_cell(Some(&mut cell), None, &CellFormatOptions::default(), ssf);
            assert_eq!(value(&cell.v, typed), raw);
            let text = value(&text, false);
            if typed {
                assert_eq!(cell.w, before);
                json!({"address": address, "type": kind, "value": raw, "w": before, "text": text})
            } else {
                json!({"address": address, "type": kind, "value": raw, "w_after_read": before, "text": text, "w_after_format": cell.w})
            }
        }).collect();
        let merges: Vec<_> = sheet.merges.iter().map(|range| json!({"s": {"r": range.s.r, "c": range.s.c}, "e": {"r": range.e.r, "c": range.e.c}})).collect();
        json!({"name": name, "range": sheet.range.map(ssf::encode_range), "merges": merges, "cells": cells})
    }).collect();
    if typed {
        json!({"date1904": book.date1904, "sheets": sheets})
    } else {
        assert!(!book.date1904);
        json!({"sheets": sheets})
    }
}

pub(super) fn seeded(args: &Value, reset: bool) -> Value {
    let mut ssf = Ssf::default();
    let old = ssf.get_table();
    old.write().unwrap().extend(super::entries(&args["seed"]));
    let file = args["file"].as_str().unwrap();
    let book = ssf::read(&bytes(file), file, &mut ssf).unwrap();
    let current = ssf.get_table();
    let same = std::sync::Arc::ptr_eq(&old, &current);
    if reset {
        assert_eq!(ssf.load("\"new\"0", Some(164)), 164);
        let old_value = old.read().unwrap().get(&164).cloned();
        let current_value = current.read().unwrap().get(&164).cloned();
        json!({"ok": {"same": same, "old": old_value, "current": current_value, "snapshot": book.ssf.get(&164)}})
    } else {
        let code = current.read().unwrap().get(&0).cloned();
        json!({"ok": {"same": same, "format": code, "book": project(book, &ssf, true)}})
    }
}

pub(super) fn format_codes(args: &Value) -> Value {
    let mut ssf = Ssf::default();
    let file = args["file"].as_str().unwrap();
    let book = ssf::read(&bytes(file), file, &mut ssf).unwrap();
    assert!(!book.date1904);
    assert_eq!(book.sheets.len(), 1);
    let sheet = book.sheets.into_values().next().unwrap();
    let cells: Vec<_> = sheet.cells.into_iter().enumerate().map(|(i, (address, mut cell))| {
        assert_eq!(cell.t, Some(ssf::ExcelDataType::Number));
        let CellValue::Number(raw) = cell.v else { panic!("expected numeric operand") };
        let cache = cell.w.clone();
        let CellValue::Text(text) = ssf::format_cell(Some(&mut cell), None, &CellFormatOptions::default(), &ssf) else { panic!("expected formatted text") };
        assert_eq!(cell.w, cache);
        assert_eq!(cell.v, CellValue::Number(raw));
        json!({"address": address, "code": book.ssf[&(164 + u32::try_from(i).unwrap())], "value": raw, "w": cache, "text": text})
    }).collect();
    json!({"ok": {"cells": cells}})
}
