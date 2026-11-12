//! FORK: copy and paste at a Code Block's edges (#68).
//!
//! A copy from inside one Code Block puts the raw code on the clipboard
//! (other apps see no fence) and tags it with the block's language and meta.
//! Pasting such a copy outside a Code Block rebuilds a tagged Code Block, and
//! pasting one fenced block into a Code Block inserts only its code.

use super::{
    command::CommandError, document::BlockContent, editor::BlockEditorModel, kind::BlockKindData,
    state::CopyKind, step::DetachedContent, transaction::EditOrigin,
};

/// The language and meta of a pasted fence, and its code.
struct Fenced {
    lang: Option<String>,
    meta: Option<String>,
    code: String,
}

impl BlockEditorModel {
    /// The selection as clipboard text, and what kind of copy it is. The
    /// editor's own copies are tagged ([`CopyKind`]) so paste can read them
    /// back (D17, D18).
    pub(crate) fn copy_selection(&self) -> (String, CopyKind) {
        if let Some(table) = self.cells_markdown() {
            return (table, CopyKind::Blocks);
        }
        if let Some((parent, range)) = self.selected_blocks() {
            let ids = &self.doc.children(parent)[range];
            return (
                super::serialize::join_blocks(&self.doc, ids),
                CopyKind::Blocks,
            );
        }
        let Ok(range) = self.doc.resolve_selection(self.selection()) else {
            return (String::new(), CopyKind::External);
        };
        // Several blocks: Markdown for the whole selection (D18).
        if range.start.block != range.end.block
            && let Some(markdown) = super::slice::selection_markdown(&self.doc, range)
        {
            return (markdown, CopyKind::Blocks);
        }
        // Inside one Code Block: the raw code, tagged with its info string.
        if range.start.block == range.end.block
            && let Some(BlockKindData::CodeBlock { lang, meta }) = self.doc.kind(range.start.block)
            && let Some(code) = self.doc.plain(range.start.block)
        {
            let text = code
                .get(range.start.offset..range.end.offset)
                .unwrap_or_default()
                .to_string();
            let kind = CopyKind::Code {
                lang: lang.clone(),
                meta: meta.clone(),
            };
            return (text, kind);
        }
        let mut inline_only = true;
        let text = self
            .text_ranges(range)
            .into_iter()
            .filter_map(|(id, lo, hi)| {
                // Marks and inline atoms copy as Markdown (an image as
                // `![alt](url)`, never the U+FFFC placeholder, #3113). A
                // partial selection serializes as its own inline, so
                // delimiters are always balanced at the cut edges.
                if let Some(inline) = self.doc.inline(id) {
                    return Some(super::serialize::inline_copy_markdown(
                        &inline.slice(lo..hi),
                    ));
                }
                inline_only = false;
                let text = self.doc.leaf_text(id)?;
                text.get(lo..hi).map(str::to_string)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let kind = if inline_only {
            CopyKind::Inline
        } else {
            CopyKind::External
        };
        (text, kind)
    }

    /// Paste clipboard text. `plain` always inserts it raw.
    pub(crate) fn paste_clipboard(
        &mut self,
        text: &str,
        plain: bool,
        copy: &CopyKind,
    ) -> Result<(), CommandError> {
        if plain {
            return self.insert_text(text, EditOrigin::Paste);
        }
        let in_code = self.code_block_at_selection();
        if let CopyKind::Code { lang, meta } = copy {
            // One line is text, and a Code Block takes its code raw. Several
            // lines outside a Code Block become one, placed like any pasted
            // block.
            if in_code.is_none() && text.contains('\n') {
                let fenced = super::serialize::fenced_code(lang.as_deref(), meta.as_deref(), text);
                if self.paste_blocks_markdown(&fenced)? {
                    return Ok(());
                }
            }
            return self.insert_text(text, EditOrigin::Paste);
        }
        if let Some((lang, meta)) = &in_code
            && let Some(fenced) = self.single_fence(text)
        {
            return self.paste_fenced_into_code(fenced, lang.is_none(), meta.is_none());
        }
        if *copy == CopyKind::Blocks && self.paste_blocks_markdown(text)? {
            return Ok(());
        }
        if *copy != CopyKind::External && self.paste_inline_markdown(text)? {
            return Ok(());
        }
        if self.paste_blocks(text)? {
            return Ok(());
        }
        self.insert_text(text, EditOrigin::Paste)
    }

    /// The language and meta of the Code Block the selection starts in, when
    /// a plain text selection is in one.
    fn code_block_at_selection(
        &self,
    ) -> Option<(Option<gpui::SharedString>, Option<gpui::SharedString>)> {
        if self.selected_cells().is_some() || self.selected_blocks().is_some() {
            return None;
        }
        let start = self.doc.resolve_selection(self.selection()).ok()?.start;
        match self.doc.kind(start.block)? {
            BlockKindData::CodeBlock { lang, meta } => Some((lang.clone(), meta.clone())),
            _ => None,
        }
    }

    /// `text` when it is exactly one closed fenced block (backticks or
    /// tildes) and nothing else.
    fn single_fence(&self, text: &str) -> Option<Fenced> {
        let trimmed = text.trim();
        let mut lines = trimmed.lines();
        let opening = lines.next()?.trim_start();
        let delimiter = opening.chars().next().filter(|c| matches!(c, '`' | '~'))?;
        if opening.chars().take_while(|c| *c == delimiter).count() < 3 {
            return None;
        }
        let closing = lines.next_back()?.trim();
        if closing.len() < 3 || !closing.chars().all(|c| c == delimiter) {
            return None;
        }
        let parsed =
            gpui_kit::base::text::editor_host::parse_markdown(text, self.doc.extensions.clone())
                .ok()?;
        let mut blocks = Vec::new();
        for node in parsed.iter() {
            super::import::import_block(node, text, &super::import::Defs::new(), &mut blocks);
        }
        let [block] = blocks.as_slice() else {
            return None;
        };
        let (
            BlockKindData::CodeBlock { lang, meta },
            DetachedContent::Leaf(BlockContent::Plain(code)),
        ) = (&block.kind, &block.content)
        else {
            return None;
        };
        Some(Fenced {
            lang: lang.as_ref().map(|l| l.to_string()),
            meta: meta.as_ref().map(|m| m.to_string()),
            code: code.clone(),
        })
    }

    /// Insert only the fence's code. A target without a language also takes
    /// the pasted language (and meta, when it has none), in the same undo
    /// step.
    fn paste_fenced_into_code(
        &mut self,
        fenced: Fenced,
        target_bare: bool,
        target_no_meta: bool,
    ) -> Result<(), CommandError> {
        self.history.begin_transaction();
        let result = (|| {
            self.insert_text(&fenced.code, EditOrigin::Paste)?;
            if target_bare && fenced.lang.is_some() {
                let meta = fenced.meta.filter(|_| target_no_meta);
                self.set_code_block_info(fenced.lang, meta)?;
            }
            Ok(())
        })();
        self.history.commit_transaction();
        result
    }
}
