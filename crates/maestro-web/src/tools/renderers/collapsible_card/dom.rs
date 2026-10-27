//! Page-mounted disclosures: the status header, its animated body and the files outside it.
use super::{Body, CollapsibleCard, Content, Phase};
use crate::{
    SharedAttachment,
    browser::attachment_content::browser::{element, host_error},
    components::attachment_tile::AttachmentTile,
    dom::{DomContent, DomRoot, DomView, MountedContent},
    tools::{
        renderer_registry::ToolHeaderRefs,
        renderers::{
            card_content::dom::{ErrorText, console, header},
            default_renderer::Listing,
        },
        types::ToolUiError,
    },
};
use std::{
    cell::RefCell,
    sync::{Arc, RwLock},
};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement, Node};

/// Native roots a mounted disclosure owns for its selected template.
#[derive(Default)]
struct Mounted {
    /// Selected template; a change rebuilds every child, as a new template does.
    phase: Option<Phase>,
    /// Status header, a toggle while a body exists.
    header: MountedContent,
    /// Live targets the header reads when activated.
    refs: ToolHeaderRefs,
    /// Collapsed body and its ordered URL, code, difference, console and error children.
    body: Option<(HtmlElement, [MountedContent; 5])>,
    /// Plain error outside the disclosure.
    error: MountedContent,
    /// File row and each positional tile with the record it shows.
    files: Option<(Element, Vec<(SharedAttachment, MountedContent)>)>,
}
impl<R: 'static> DomContent for Content<R, Box<dyn DomContent>> {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-tool-disclosure")?.into(),
            owner: Some(Box::new(RefCell::new(Mounted::default()))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let owner = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<Mounted>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing disclosure state".into()))?;
        let mut mounted = owner.take();
        let result = self.card.show(&mut mounted, &root.node, view);
        let old = owner.replace(mounted);
        drop(old);
        result
    }
}
impl CollapsibleCard<Box<dyn DomContent>> {
    /// Change the selected template's children in place, rebuilding them when it changes.
    fn show(&self, mounted: &mut Mounted, root: &Node, view: &DomView) -> Result<(), ToolUiError> {
        let phase = self
            .body
            .as_ref()
            .map(|body| body.phase)
            .or(self.plain_phase);
        let rebuilt = mounted.phase != phase || mounted.body.is_some() != self.body.is_some();
        if rebuilt {
            root.unchecked_ref::<Element>()
                .replace_children_with_node_0();
            *mounted = Mounted {
                phase,
                ..Mounted::default()
            };
        }
        let refs = self.body.is_some().then(|| mounted.refs.clone());
        let header = header(self.state, self.glyph, &self.caption, view, refs)?;
        mounted.header.show(header.as_ref(), root, view)?;
        if rebuilt && self.body.is_some() {
            let body: HtmlElement =
                element(&view.document, "div", "maestro-tool-body max-h-0")?.unchecked_into();
            root.append_child(&body)
                .map_err(|error| host_error(&error))?;
            mounted.refs.content.replace(Some(body.clone()));
            mounted.body = Some((body, Default::default()));
        }
        if let (Some(body), Some((node, slots))) = (&self.body, &mut mounted.body) {
            body.show(slots, node, view)?;
        }
        let error = self
            .error
            .as_deref()
            .map(|error| output(error, Some("An error occurred"), &view.i18n).map(ErrorText))
            .transpose()?;
        let files = mounted.files.as_ref().map(|(row, _)| row.clone().into());
        show_ordered(
            std::slice::from_mut(&mut mounted.error),
            [error.as_ref().map(|error| error as &dyn DomContent)],
            (root, files),
            view,
        )?;
        self.files(mounted, root, view)
    }
    /// Keep tiles outside the body, reusing each one whose original record is unchanged.
    fn files(&self, mounted: &mut Mounted, root: &Node, view: &DomView) -> Result<(), ToolUiError> {
        if self.attachments.is_empty() {
            if let Some((row, _)) = mounted.files.take() {
                row.remove();
            }
            return Ok(());
        }
        let (row, tiles) = match &mut mounted.files {
            Some(files) => files,
            files @ None => {
                let row = element(&view.document, "div", "maestro-tool-files")?;
                root.append_child(&row)
                    .map_err(|error| host_error(&error))?;
                files.insert((row, Vec::new()))
            }
        };
        for (_, mut tile) in tiles.drain(self.attachments.len().min(tiles.len())..) {
            tile.clear();
        }
        for (index, download) in self.attachments.iter().enumerate() {
            if let Some((record, _)) = tiles.get(index)
                && download.matches(record)?
            {
                continue;
            }
            let record = Arc::new(RwLock::new(download.record()));
            let tile = AttachmentTile::from(Arc::clone(&record));
            if index == tiles.len() {
                tiles.push((Arc::clone(&record), MountedContent::default()));
            }
            if let Some((shown, slot)) = tiles.get_mut(index) {
                *shown = record;
                slot.show(&tile, row, view)?;
            }
        }
        Ok(())
    }
}
impl Body<Box<dyn DomContent>> {
    /// Show the selected children in order, retaining each root while its branch stays selected.
    fn show(
        &self,
        slots: &mut [MountedContent; 5],
        body: &HtmlElement,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        let text = |text: &str| output(text, self.fallback, &view.i18n);
        let url = self.url.clone().map(Url);
        let code = (self.code.as_ref())
            .map(|(code, language)| text(code).map(|code| Listing { code, language }))
            .transpose()?;
        let shown = (self.console.as_ref())
            .map(|(content, variant)| text(content).map(|content| console(content, *variant)))
            .transpose()?;
        let error = self.error.as_deref().map(text).transpose()?.map(ErrorText);
        show_ordered(
            slots,
            [
                url.as_ref().map(|url| url as &dyn DomContent),
                code.as_ref().map(|code| code as &dyn DomContent),
                self.diff.as_deref(),
                shown.as_ref().map(|console| console as &dyn DomContent),
                error.as_ref().map(|error| error as &dyn DomContent),
            ],
            (body, None),
            view,
        )
    }
}
/// Show each present child in its slot and remove absent ones; a first root goes before the
/// next shown slot, or before `end`.
fn show_ordered<const N: usize>(
    slots: &mut [MountedContent],
    children: [Option<&dyn DomContent>; N],
    (parent, end): (&Node, Option<Node>),
    view: &DomView,
) -> Result<(), ToolUiError> {
    let mut anchor = end;
    for (slot, child) in slots.iter_mut().zip(children).rev() {
        match child {
            Some(child) => slot.show_before(child, parent, anchor.as_ref(), view)?,
            None => slot.clear(),
        }
        if let Some(root) = slot.root() {
            anchor = Some(root.clone());
        }
    }
    Ok(())
}
/// Literal URL row after its bold label.
struct Url(String);
impl DomContent for Url {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let row = element(&view.document, "div", "maestro-tool-url")?;
        let label = view
            .document
            .create_element("strong")
            .map_err(|error| host_error(&error))?;
        label.set_text_content(Some("URL:"));
        row.append_with_node_2(&label, &view.document.create_text_node(""))
            .map_err(|error| host_error(&error))?;
        let root = DomRoot::new(row);
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        if let Some(value) = root.node.last_child() {
            value.set_text_content(Some(&format!(" {}", self.0)));
        }
        Ok(())
    }
}

impl super::Download {
    /// Compare caller-visible input, excluding generated identity and derived text.
    fn matches(&self, record: &crate::SharedAttachment) -> Result<bool, ToolUiError> {
        let record = record.try_read().map_err(|_| {
            ToolUiError::Attachment(
                crate::dialogs::attachment_overlay::AttachmentUiError::BorrowConflict,
            )
        })?;
        let original = &self.attachment;
        Ok(record.file_name == original.file_name
            && record.mime_type == original.mime_type
            && record.size == original.size
            && record.content == original.content)
    }
}
/// Translate only a selected empty-output fallback, never caller text.
fn output(
    text: &str,
    fallback: Option<&'static str>,
    i18n: &crate::I18n,
) -> Result<String, ToolUiError> {
    if text.is_empty()
        && let Some(key) = fallback
    {
        return Ok(i18n.text(key)?);
    }
    Ok(text.into())
}
