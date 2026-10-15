//! Model-facing browser tool descriptions.
/// Guidance supplied to the model for document extraction.
pub const EXTRACT_DOCUMENT_DESCRIPTION: &str = r#"# Extract Document

Extract plain text from documents on the web (PDF, DOCX, XLSX, PPTX).

## When to Use
User wants you to read a document at a URL.

## Input
- { url: "https://example.com/document.pdf" } - URL to PDF, DOCX, XLSX, or PPTX

## Returns
Structured plain text with page/sheet/slide delimiters."#;

/// Guidance supplied to the model with admitted runtime helper descriptions.
#[must_use]
pub fn javascript_repl_tool_description(runtime_provider_descriptions: &[String]) -> String {
    let descriptions = runtime_provider_descriptions.join("\n\n");
    format!(
        r"# JavaScript REPL

## Purpose
Execute JavaScript code in a sandboxed browser environment with full Web APIs.

## When to Use
- Quick calculations or data transformations
- Testing JavaScript code snippets in isolation
- Processing data with libraries (XLSX, CSV, etc.)
- Creating artifacts from data

## Environment
- ES2023+ JavaScript (async/await, optional chaining, nullish coalescing, etc.)
- All browser APIs: DOM, Canvas, WebGL, Fetch, Web Workers, WebSockets, Crypto, etc.
- Import any npm package: await import('https://esm.run/package-name')

## Common Libraries
- XLSX: const XLSX = await import('https://esm.run/xlsx');
- CSV: const Papa = (await import('https://esm.run/papaparse')).default;
- Chart.js: const Chart = (await import('https://esm.run/chart.js/auto')).default;
- Three.js: const THREE = await import('https://esm.run/three');

## Persistence between tool calls
- Objects stored on global scope do not persist between calls.
- Use artifacts as a key-value JSON object store:
  - Use createOrUpdateArtifact(filename, content) to persist data between calls. JSON objects are auto-stringified.
  - Use listArtifacts() and getArtifact(filename) to read persisted data. JSON files are auto-parsed to objects.
  - Prefer to use a single artifact throughout the session to store intermediate data (e.g. 'data.json').

## Input
- You have access to the user's attachments via listAttachments(), readTextAttachment(id), and readBinaryAttachment(id)
- You have access to previously created artifacts via listArtifacts() and getArtifact(filename)

## Output
- All console.log() calls are captured for you to inspect. The user does not see these logs.
- Create artifacts for file results (images, JSON, CSV, etc.) which persiste throughout the
  session and are accessible to you and the user.

## Example
const data = [10, 20, 15, 25];
const sum = data.reduce((a, b) => a + b, 0);
const avg = sum / data.length;
console.log('Sum:', sum, 'Average:', avg);

## Important Notes
- Graphics: Use fixed dimensions (800x600), NOT window.innerWidth/Height
- Chart.js: Set options: {{ responsive: false, animation: false }}
- Three.js: renderer.setSize(800, 600) with matching aspect ratio

## Helper Functions (Automatically Available)

These functions are injected into the execution environment and available globally:

{descriptions}
"
    )
}
