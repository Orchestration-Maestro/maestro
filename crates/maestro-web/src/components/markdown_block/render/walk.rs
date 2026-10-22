//! Block walk over one prepared document, including nested containers.
use super::{Atoms, Flow, Policy, Writer, block_formula, escape, escape_authored};
use maestro_app::markdown::{Kind, Node};
use std::fmt::Write as _;

/// One pending step of a block walk, innermost last.
enum Step<'a, 'b> {
    /// Write the remaining nodes of one sibling run.
    Nodes(std::slice::Iter<'a, Node>, Atoms<'a, 'b>),
    /// Close a container whose children are written.
    Close(&'static str),
}
impl Writer<'_, '_> {
    /// Write block nodes in order.
    pub(super) fn blocks(&mut self, nodes: &[Node], atoms: Atoms<'_, '_>, out: &mut String) {
        let mut pending = vec![Step::Nodes(nodes.iter(), atoms)];
        self.drain(&mut pending, out);
    }
    /// Write queued steps until none remain.
    fn drain(&mut self, pending: &mut Vec<Step<'_, '_>>, out: &mut String) {
        while let Some(step) = pending.pop() {
            self.step(step, pending, out);
        }
    }
    /// Write one queued step and queue what follows it.
    fn step<'a, 'b>(
        &mut self,
        step: Step<'a, 'b>,
        pending: &mut Vec<Step<'a, 'b>>,
        out: &mut String,
    ) {
        match step {
            Step::Close(tag) => out.push_str(tag),
            Step::Nodes(mut nodes, atoms) => {
                let Some(node) = nodes.next() else {
                    return;
                };
                pending.push(Step::Nodes(nodes, atoms));
                self.block(node, atoms, out, pending);
            }
        }
    }
    /// Write one block node and queue the steps that follow it.
    fn block<'a, 'b>(
        &mut self,
        node: &'a Node,
        atoms: Atoms<'a, 'b>,
        out: &mut String,
        pending: &mut Vec<Step<'a, 'b>>,
    ) {
        match &node.kind {
            Kind::CustomBlock(kind, text) => {
                if let Some(formula) = block_formula(kind, text, node.authored.len()) {
                    out.push_str(&self.math(&formula));
                } else {
                    escape_authored(node, out);
                }
            }
            Kind::Paragraph(c) => self.flow(c, (&node.range, Flow::Paragraph), atoms, out),
            Kind::Inline(c) => self.flow(c, (&node.range, Flow::Item), atoms, out),
            Kind::Heading(level, c, _) => {
                let _ = write!(out, "<h{level}>");
                self.flow(c, (&node.range, Flow::Inline), atoms, out);
                let _ = writeln!(out, "</h{level}>");
            }
            Kind::List(start, items) => {
                let (tag, attr, close) = match start {
                    None => ("ul", String::new(), "</ul>\n"),
                    Some(1) => ("ol", String::new(), "</ol>\n"),
                    Some(n) => ("ol", format!(" start=\"{n}\""), "</ol>\n"),
                };
                let _ = writeln!(out, "<{tag}{attr}>");
                pending.push(Step::Close(close));
                pending.push(Step::Nodes(items.iter(), atoms));
            }
            Kind::Item(c) => {
                out.push_str("<li>");
                pending.push(Step::Close("</li>\n"));
                pending.push(Step::Nodes(c.iter(), atoms));
            }
            Kind::Quote(c, _) => {
                out.push_str("<blockquote>\n");
                pending.push(Step::Close("</blockquote>\n"));
                pending.push(Step::Nodes(c.iter(), atoms));
            }
            Kind::HtmlBlock(html) => match self.policy {
                Policy::Transcript { .. } => out.push_str(html),
                Policy::Export(_) => escape(html, out),
            },
            Kind::Rule => out.push_str("<hr>\n"),
            Kind::CodeBlock(code, info) => self.code(code, info.as_deref(), out),
            Kind::Table(table) => self.table(table, atoms, out),
            Kind::Gap => {}
            _ => self.inline(std::slice::from_ref(node), atoms, out),
        }
    }
}
