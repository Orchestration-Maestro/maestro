//! HTML table projection under the selected rendering policy.
use super::{Atoms, Policy, Writer};
use maestro_app::markdown::Table;

impl Writer<'_, '_> {
    /// Write a table inside its own horizontal scroller.
    pub(super) fn table(&mut self, table: &Table, atoms: Atoms<'_, '_>, out: &mut String) {
        if matches!(self.policy, Policy::Transcript { .. }) {
            out.push_str("<div class=\"maestro-table-scroll\">");
        }
        out.push_str("<table>\n<thead>\n");
        self.row(&table.header, (&table.alignments, "th"), atoms, out);
        out.push_str("</thead>\n");
        let body = !table.rows.is_empty() || matches!(self.policy, Policy::Transcript { .. });
        if body {
            out.push_str("<tbody>");
        }
        for row in &table.rows {
            self.row(row, (&table.alignments, "td"), atoms, out);
        }
        if body {
            out.push_str("</tbody>");
        }
        out.push_str("</table>\n");
        if matches!(self.policy, Policy::Transcript { .. }) {
            out.push_str("</div>");
        }
    }
}
