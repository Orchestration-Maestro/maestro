//! Shared integration-test executable.
#![cfg(test)]

#[path = "docx_support/body_tree.rs"]
mod body_tree;
mod browser_decimal;
mod docx_body;
mod docx_comments;
mod docx_drawing_records;
mod docx_formatting;
mod docx_metadata;
mod docx_metadata_packages;
mod docx_numbering_fonts;
mod docx_packages;
mod docx_paragraph_properties;
mod docx_parts;
mod docx_properties;
mod docx_section_properties;
mod docx_support;
mod docx_tables;
mod docx_values;
mod docx_xml;
#[path = "docx_support/font_resources.rs"]
mod font_resources;
mod legacy_workbook_support;
mod legacy_workbooks;
#[path = "docx_support/metadata.rs"]
mod metadata;
#[path = "docx_support/numbering.rs"]
mod numbering_observation;
mod spreadsheet_html;
mod spreadsheet_support;
mod ssf_detection;
mod ssf_format;
mod ssf_support;
mod ssf_values;
mod ssf_writers;
mod workbook_fixture;
mod workbooks;
