# Word table contract fixture

`packages/table-contracts.docx` is an unchanged 5,023-byte producer-written
package from [the upstream table span sample](https://github.com/VolodymyrBaydalka/docxjs/blob/c533f383069008d4a1e05c6171ef8777fd078385/tests/render-test/table-spans/document.docx).
It was introduced by commit `a3ea7c54d3d99df72fb82ed4eaf2515a330a086e`.

- SHA-256: `7cb203433e01f90128d50c1c557669dff7181ac005e1563b169cafa2d3a4cf3f`.
- Saved `docProps/app.xml` identifies LibreOffice/7.2.2.2 Windows_X86_64,
  build `02b2acce88a210515b4a5bb2e46cbfb63fe97d56`; this is retained producer
  metadata, not a claim of local generation.
- The table's immediate properties contain `tblLayout type="fixed"` and
  `tblInd w="0" type="dxa"`.
- Copyright Volodymyr Baydalka. Licensed under
  [Apache-2.0](https://github.com/VolodymyrBaydalka/docxjs/blob/c533f383069008d4a1e05c6171ef8777fd078385/LICENSE);
  see [the repository notice](../../../../../THIRD-PARTY-NOTICES.md#word-package-reading).

The accompanying JSON records exercise the shared property and named-style
readers, including ordered maps and callback observations. They do not describe a
rendered preview.
