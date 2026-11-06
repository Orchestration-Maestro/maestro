//! One syntax walk per parsed source that gathers the findings of every check.
//!
//! Each check keeps its own visitor state and rules; the walker owns the traversal and
//! the test context each state inherits, so a source's syntax tree is visited once for
//! all checks. The walk also records the file-level facts checks read from the parsed
//! file, so no syntax outlives it.

use std::collections::BTreeSet;
use std::ops::Range;

use syn::{spanned::Spanned, visit::Visit};

use super::{
    Member, Source, cfg_test, impl_item_attributes, item_attributes, test_file,
    trait_item_attributes,
};
use crate::boundaries::{application_exports, message_renderers};
use crate::graph::packages::{self, Exports};
use crate::poison;
use crate::{child_tests, duplicates, generated, integration_tests, quality, repetition};
use crate::{temp_directories, test_input_routes, time_limits};

/// Every check's findings for one parsed source.
pub(crate) struct Walk {
    /// Whether the file's inner attributes gate it on the test configuration.
    pub(crate) gated: bool,
    /// End line of the last item when that item is test-gated.
    pub(crate) test_end: Option<usize>,
    /// Whether a production source's top-level items declare or re-export the public factory.
    pub(crate) factory: bool,
    /// Module declarations of an application or domain source, with their visibility scopes.
    pub(crate) modules: application_exports::Scopes,
    /// Top-level module declarations of a member's integration-test root.
    pub(crate) root_modules: Vec<integration_tests::Module>,
    /// Disjoint spans of the test-gated items, sorted by start.
    pub(crate) test_spans: Vec<Range<(usize, usize)>>,
    /// First ignore-attribute ownership violation, with its line.
    pub(crate) ignored: Option<(usize, &'static str)>,
    /// Located message-renderer ownership violations in source order.
    pub(crate) renderers: Vec<((usize, usize), String)>,
    /// First test-module layout violation.
    pub(crate) layout: Option<String>,
    /// Located test-owned timer, polling and capture findings in source order.
    pub(crate) time_limits: Vec<((usize, usize), time_limits::Diagnostic)>,
    /// Production function bodies as block byte offsets with their signature lines.
    pub(crate) bodies: Vec<(usize, usize)>,
    /// Sorted lines naming the platform temporary directory in test code.
    pub(crate) temp_directories: Vec<usize>,
    /// Located repository-directory reads by test code.
    pub(crate) reads: BTreeSet<(usize, String)>,
    /// Imports of an application source in its logical module.
    pub(crate) imports: Option<Vec<application_exports::Import>>,
    /// Lint-level attributes and root layout of a guest source.
    pub(crate) guest: Option<generated::Attributes>,
    /// Literal extension exports outside test-gated items.
    pub(crate) exports: usize,
    /// Located independent recovery helpers in production code.
    pub(crate) poison: Vec<usize>,
}

/// Walk a member's parsed source once and gather every check's findings.
pub(super) fn walk(member: &Member, source: &Source, syntax: &syn::File) -> Walk {
    #[cfg(test)]
    super::SYNTAX_WALKS.set(super::SYNTAX_WALKS.get() + 1);
    let mut walker = Walker::new(member, source, syntax);
    walker.visit_file(syntax);
    walker.finish()
}

/// Item kinds by how far the checks inherit their test gate, weakest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gate {
    /// An item inside an `extern` block, which records a test span only.
    Foreign,
    /// An item inside an `impl` or `trait` block, whose syntax is test code.
    Associated,
    /// A module-level item, whose syntax is test code that also exports nothing.
    Item,
}

/// Test context and traversal position that the check states read.
#[derive(Clone, Copy, Default)]
struct Context {
    /// Strongest test gate around the current syntax; any gate means a recorded test span,
    /// so a gated item nested inside adds no span.
    gate: Option<Gate>,
    /// Inside an attribute.
    attribute: bool,
    /// Inside a `use` tree.
    importing: bool,
    /// Inside the attributes or the name of a method call, which the reference scan skips.
    method_parts: bool,
}

/// The single visitor that drives every check's state through one traversal.
struct Walker<'a> {
    /// Current test context and traversal position.
    context: Context,
    /// Whether the whole source belongs to tests.
    scope: bool,
    /// Whether the file's inner attributes gate it on the test configuration.
    gated: bool,
    /// End line of a test-gated last item.
    test_end: Option<usize>,
    /// Whether a production source exposes the public factory.
    factory: bool,
    /// Application or domain module declarations.
    modules: application_exports::Scopes,
    /// Top-level module declarations, for an integration-test root.
    root_modules: Vec<integration_tests::Module>,
    /// Outermost test-gated item spans in source order.
    test_spans: Vec<Range<(usize, usize)>>,
    /// Ignore-attribute ownership.
    ignored: child_tests::Ignored,
    /// Message-renderer ownership.
    guard: message_renderers::Guard<'a>,
    /// Test-module layout.
    layout: repetition::TestLayout<'a>,
    /// Test-owned timers, polling and captures.
    calls: time_limits::Calls<'a>,
    /// Production function bodies.
    bodies: duplicates::Bodies,
    /// Platform temporary directory references, for checked members.
    references: Option<temp_directories::References>,
    /// Repository-directory reads by test code.
    reads: test_input_routes::Reads,
    /// Application imports, for application sources.
    uses: Option<application_exports::Uses>,
    /// Lint-level attributes, for guest sources.
    attributes: Option<generated::Attributes>,
    /// Extension exports, for production sources that spell the export macro.
    exports: Option<Exports<'a>>,
    /// Independent recovery helpers.
    poison: poison::Definitions,
}

impl<'a> Walker<'a> {
    /// Record the file-level facts and prepare the states that serve this member's source.
    fn new(member: &'a Member, source: &'a Source, file: &syn::File) -> Self {
        let gated = cfg_test(&file.attrs);
        let scope = test_file(member, &source.path) || gated;
        Self {
            context: Context::default(),
            scope,
            gated,
            test_end: quality::test_end(file),
            factory: !scope && packages::factory(file),
            modules: application_exports::file_modules(member, source, file),
            root_modules: integration_tests::modules(member, source, file),
            test_spans: Vec::new(),
            ignored: child_tests::Ignored::new(member, source),
            guard: message_renderers::Guard::new(member, source),
            layout: repetition::TestLayout::new(member, source),
            calls: time_limits::Calls::new(member),
            bodies: duplicates::Bodies::default(),
            references: temp_directories::checked(member)
                .then(temp_directories::References::default),
            reads: test_input_routes::Reads::default(),
            uses: application_exports::Uses::new(member, source),
            attributes: (member.name == generated::GUEST).then(|| generated::Attributes::new(file)),
            exports: Exports::new(source, scope),
            poison: poison::Definitions::default(),
        }
    }

    /// Whether the current syntax belongs to tests, by its source or an inherited gate.
    fn testing(&self) -> bool {
        self.scope || self.context.gate >= Some(Gate::Associated)
    }

    /// Whether the directory-read scan sees the current syntax: test code outside attributes.
    fn reading(&self) -> bool {
        self.testing() && !self.context.attribute
    }

    /// Whether the reference scan sees the current syntax.
    fn referencing(&self) -> bool {
        !self.context.method_parts && (self.testing() || self.context.importing)
    }

    /// Visit one subtree under a changed context, then restore the outer context.
    fn nested(&mut self, change: impl FnOnce(&mut Context), visit: impl FnOnce(&mut Self)) {
        let outer = self.context;
        change(&mut self.context);
        visit(self);
        self.context = outer;
    }

    /// Visit an item inside the test gate its attributes declare, recording the outermost span.
    fn gate(
        &mut self,
        kind: Gate,
        attributes: &[syn::Attribute],
        item: &impl Spanned,
        visit: impl FnOnce(&mut Self),
    ) {
        let gated = cfg_test(attributes);
        if gated && self.context.gate.is_none() {
            self.test_spans.push(span_positions(item.span()));
        }
        self.nested(
            |context| {
                if gated {
                    context.gate = context.gate.max(Some(kind));
                }
            },
            visit,
        );
    }

    /// Exempt a declaration's own name from the reference scan; its body stays visited.
    fn declaring(&mut self, ident: &proc_macro2::Ident) {
        if let Some(references) = &mut self.references {
            references.skip(ident);
        }
    }

    /// Record a production function body for the duplicate check.
    fn body(&mut self, signature: &syn::Signature, block: &syn::Block) {
        if self.context.gate < Some(Gate::Associated) {
            self.bodies.function(signature, block);
        }
    }

    /// Finish every state into the source's owned findings.
    fn finish(mut self) -> Walk {
        self.test_spans.sort_unstable_by_key(|span| span.start);
        Walk {
            gated: self.gated,
            test_end: self.test_end,
            factory: self.factory,
            modules: self.modules,
            root_modules: self.root_modules,
            test_spans: self.test_spans,
            ignored: self.ignored.error,
            renderers: self.guard.finish(),
            layout: self.layout.error,
            time_limits: self.calls.finish(),
            bodies: self.bodies.0,
            temp_directories: self
                .references
                .map(temp_directories::References::finish)
                .unwrap_or_default(),
            reads: self.reads.finish(),
            imports: self.uses.map(|uses| uses.imports),
            guest: self.attributes,
            exports: self.exports.map_or(0, Exports::finish),
            poison: self.poison.finish(),
        }
    }
}

impl<'a> Visit<'a> for Walker<'a> {
    fn visit_item(&mut self, item: &'a syn::Item) {
        self.gate(Gate::Item, item_attributes(item), item, |walker| {
            syn::visit::visit_item(walker, item);
        });
    }

    fn visit_impl_item(&mut self, item: &'a syn::ImplItem) {
        self.gate(
            Gate::Associated,
            impl_item_attributes(item),
            item,
            |walker| {
                syn::visit::visit_impl_item(walker, item);
            },
        );
    }

    fn visit_trait_item(&mut self, item: &'a syn::TraitItem) {
        self.gate(
            Gate::Associated,
            trait_item_attributes(item),
            item,
            |walker| {
                syn::visit::visit_trait_item(walker, item);
            },
        );
    }

    fn visit_foreign_item(&mut self, item: &'a syn::ForeignItem) {
        let attributes: &[syn::Attribute] = match item {
            syn::ForeignItem::Fn(item) => &item.attrs,
            syn::ForeignItem::Static(item) => &item.attrs,
            syn::ForeignItem::Type(item) => &item.attrs,
            syn::ForeignItem::Macro(item) => &item.attrs,
            _ => &[],
        };
        self.gate(Gate::Foreign, attributes, item, |walker| {
            syn::visit::visit_foreign_item(walker, item);
        });
    }

    fn visit_attribute(&mut self, attribute: &'a syn::Attribute) {
        self.ignored.attribute(attribute);
        if !self.context.attribute
            && let Some(attributes) = &mut self.attributes
        {
            attributes.attribute(attribute);
        }
        self.nested(
            |context| context.attribute = true,
            |walker| syn::visit::visit_attribute(walker, attribute),
        );
    }

    fn visit_item_fn(&mut self, function: &'a syn::ItemFn) {
        self.declaring(&function.sig.ident);
        self.ignored.item_fn(function);
        self.body(&function.sig, &function.block);
        if !self.testing() {
            self.poison.body(&function.block);
        }
        syn::visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, function: &'a syn::ImplItemFn) {
        self.declaring(&function.sig.ident);
        self.body(&function.sig, &function.block);
        if !self.testing() {
            self.poison.body(&function.block);
        }
        syn::visit::visit_impl_item_fn(self, function);
    }

    fn visit_trait_item_fn(&mut self, function: &'a syn::TraitItemFn) {
        self.declaring(&function.sig.ident);
        if let Some(block) = &function.default {
            self.body(&function.sig, block);
            if !self.testing() {
                self.poison.body(block);
            }
        }
        syn::visit::visit_trait_item_fn(self, function);
    }

    /// A closure body that only acquires a lock is the same helper as a function.
    fn visit_expr_closure(&mut self, closure: &'a syn::ExprClosure) {
        if !self.testing() {
            self.poison.closure(closure);
        }
        syn::visit::visit_expr_closure(self, closure);
    }

    /// A foreign function declares its name the same way; its remaining names stay visited.
    fn visit_foreign_item_fn(&mut self, function: &'a syn::ForeignItemFn) {
        self.declaring(&function.sig.ident);
        syn::visit::visit_foreign_item_fn(self, function);
    }

    fn visit_ident(&mut self, ident: &'a proc_macro2::Ident) {
        if self.referencing()
            && let Some(references) = &mut self.references
        {
            references.ident(ident);
        }
    }

    /// Visits the parts in the standard order; the reference scan skips the attributes and
    /// the method name, the receiver's own API rather than a platform-directory spelling.
    fn visit_expr_method_call(&mut self, call: &'a syn::ExprMethodCall) {
        if self.testing() {
            self.calls.method_call(call);
            if self.reading() {
                self.reads.method_call(call);
            }
        } else {
            self.guard.raw(&call.method);
        }
        self.nested(
            |context| context.method_parts = true,
            |walker| {
                for attribute in &call.attrs {
                    walker.visit_attribute(attribute);
                }
            },
        );
        self.visit_expr(&call.receiver);
        self.nested(
            |context| context.method_parts = true,
            |walker| walker.visit_ident(&call.method),
        );
        if let Some(turbofish) = &call.turbofish {
            self.visit_angle_bracketed_generic_arguments(turbofish);
        }
        for argument in &call.args {
            self.visit_expr(argument);
        }
    }

    fn visit_use_tree(&mut self, tree: &'a syn::UseTree) {
        self.nested(
            |context| context.importing = true,
            |walker| syn::visit::visit_use_tree(walker, tree),
        );
    }

    fn visit_item_use(&mut self, item: &'a syn::ItemUse) {
        if !self.testing() {
            self.guard.item_use(item);
        }
        self.calls.item_use(item);
        if let Some(exports) = &mut self.exports {
            exports.item_use(item);
        }
        if let Some(uses) = &mut self.uses {
            uses.item_use(item);
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_macro(&mut self, invocation: &'a syn::Macro) {
        if self.referencing()
            && let Some(references) = &mut self.references
        {
            references.macro_tokens(invocation);
        }
        if self.testing() {
            self.calls.macro_tokens(invocation);
        }
        if self.reading() {
            self.reads.macro_tokens(invocation);
        }
        if self.context.gate != Some(Gate::Item)
            && let Some(exports) = &mut self.exports
        {
            exports.invocation(invocation);
        }
        if !self.testing() && defines_helper(invocation) {
            self.poison.macro_tokens(invocation);
        }
        syn::visit::visit_macro(self, invocation);
    }

    fn visit_path(&mut self, path: &'a syn::Path) {
        if !self.testing() {
            self.guard.path(path);
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_item_struct(&mut self, item: &'a syn::ItemStruct) {
        if !self.testing() {
            self.guard.declaration(&item.ident);
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'a syn::ItemEnum) {
        if !self.testing() {
            self.guard.declaration(&item.ident);
        }
        syn::visit::visit_item_enum(self, item);
    }

    fn visit_item_type(&mut self, item: &'a syn::ItemType) {
        if !self.testing() {
            self.guard.declaration(&item.ident);
        }
        syn::visit::visit_item_type(self, item);
    }

    fn visit_item_mod(&mut self, item: &'a syn::ItemMod) {
        self.layout.item_mod(item);
        if let Some(uses) = &mut self.uses {
            uses.enter(item);
        }
        syn::visit::visit_item_mod(self, item);
        if let Some(uses) = &mut self.uses {
            uses.leave();
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'a syn::ItemExternCrate) {
        if let Some(uses) = &mut self.uses {
            uses.extern_crate(item);
        }
        syn::visit::visit_item_extern_crate(self, item);
    }

    fn visit_expr_call(&mut self, call: &'a syn::ExprCall) {
        if self.testing() {
            self.calls.call(call);
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_lit(&mut self, expression: &'a syn::ExprLit) {
        if self.reading() {
            self.reads.lit(expression);
        }
        syn::visit::visit_expr_lit(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'a syn::ExprPath) {
        if self.reading() {
            self.reads.path(expression);
        }
        syn::visit::visit_expr_path(self, expression);
    }
}

/// Whether a macro invocation declares items, which is where a helper body hides.
fn defines_helper(invocation: &syn::Macro) -> bool {
    invocation
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "macro_rules")
}

/// Convert a syntax span into comparable line and character coordinates.
fn span_positions(span: proc_macro2::Span) -> Range<(usize, usize)> {
    let start = span.start();
    let end = span.end();
    (start.line, start.column)..(end.line, end.column)
}
