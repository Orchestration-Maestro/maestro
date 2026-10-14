//! Model-facing document tool description.
/// Guidance supplied to the model for document extraction.
pub const EXTRACT_DOCUMENT_DESCRIPTION: &str = r#"# Extract Document

Extract plain text from documents on the web (PDF, DOCX, XLSX, PPTX).

## When to Use
User wants you to read a document at a URL.

## Input
- { url: "https://example.com/document.pdf" } - URL to PDF, DOCX, XLSX, or PPTX

## Returns
Structured plain text with page/sheet/slide delimiters."#;
