//! Direct projection of the editing model into the shared renderer's
//! `BlockNode`s (design §7.3): no Markdown round trip, so no render-mode
//! placeholders and no re-parse of a whole list for an edit inside it.
//! Every text leaf (paragraph, heading, code block, table cell) gets its
//! `BlockId` as `editor_key`.

use std::{collections::HashMap, ops::Range};

use super::{
    document::{AtomData, BlockContent, EditorDocument},
    id::BlockId,
    inline::{InlineAtom, InlineText},
    kind::BlockKindData,
};
use gpui_kit::base::text::editor_host::{
    BlockNode, BlockState, CodeBlock, InlineNode, Paragraph, Span, Table, TextMark,
};

/// A paragraph holding `inline`, keyed by `id`. `open` holds the Link spans
/// shown as source; every other Link span is drawn as a chip.
pub(crate) fn paragraph(
    inline: &InlineText,
    id: BlockId,
    placeholders: bool,
    open: &[Range<usize>],
) -> Paragraph {
    let mut paragraph = Paragraph::default();
    // FORK: Links as chips. The text is cut at the Link spans.
    let spans = super::wiki_chip::chip_spans(inline);
    for (range, mark, atom) in inline.pieces() {
        match atom {
            Some(InlineAtom::Image(image)) if placeholders => {
                paragraph.push(image_placeholder(image).marks(vec![(0..1, mark.clone())]))
            }
            Some(InlineAtom::Image(image)) => paragraph.push(InlineNode::image(image.clone())),
            Some(InlineAtom::Custom(node)) => {
                let node = InlineNode::custom(node.clone());
                let len = node.text().len();
                paragraph.push(node.marks(vec![(0..len, mark.clone())]));
            }
            None => {
                let mut at = range.start;
                let push_text = |paragraph: &mut Paragraph, range: Range<usize>| {
                    let text = &inline.text()[range];
                    if !text.is_empty() {
                        paragraph.push(
                            InlineNode::new(text.to_string())
                                .marks(vec![(0..text.len(), mark.clone())]),
                        );
                    }
                };
                for span in spans
                    .iter()
                    .filter(|s| range.start <= s.start && s.end <= range.end)
                {
                    push_text(&mut paragraph, at..span.start);
                    let source = &inline.text()[span.clone()];
                    if open.contains(span) {
                        // FORK: an open Link is inline code, in the
                        // projection only.
                        let mark = TextMark {
                            code: true,
                            ..mark.clone()
                        };
                        paragraph.push(
                            InlineNode::new(source.to_string())
                                .marks(vec![(0..source.len(), mark)]),
                        );
                    } else {
                        let chip = super::wiki_chip::chip_node(
                            source,
                            super::wiki_chip::ChipKey::new(id, span.start),
                        );
                        let len = chip.text().len();
                        paragraph.push(chip.marks(vec![(0..len, mark.clone())]));
                    }
                    at = span.end;
                }
                push_text(&mut paragraph, at..range.end);
            }
        }
    }
    paragraph.with_leaf_key(id.as_u64())
}

/// An image atom shown as its alt text, never loading the image. It is a
/// custom inline object without a renderer, which draws its text fallback and
/// stays one atomic caret and selection unit.
fn image_placeholder(image: &gpui_kit::base::text::editor_host::ImageNode) -> InlineNode {
    let label: gpui::SharedString = image
        .alt
        .clone()
        .filter(|alt| !alt.is_empty())
        .unwrap_or_else(|| "Image".into());
    InlineNode::custom(
        gpui_kit::base::text::MarkdownNode::new("image-placeholder", ())
            .text(label.clone())
            .accessibility_label(label),
    )
}

/// Project one block (and its subtree), uncached.
#[cfg(test)]
pub(crate) fn project_block(doc: &EditorDocument, id: BlockId) -> Option<BlockNode> {
    NodeCache::default().node(doc, id)
}

/// Per-node projection cache (design §7.3). Every block and table row is
/// cached under its id with the block revision it was built at; `touch`
/// bubbles revisions to ancestors, so an edit rebuilds only the path from
/// the edited leaf to its top-level block and reuses every other subtree
/// (sharing its retained `InlineState`s).
#[derive(Default)]
pub(crate) struct NodeCache {
    /// Present image atoms as text placeholders: nothing is resolved,
    /// decoded or loaded (`ImagePolicy::Placeholder`).
    pub(crate) placeholders: bool,
    /// FORK: the Link spans shown as source, by leaf.
    open: super::wiki_chip::OpenSpans,
    nodes: HashMap<BlockId, (u64, BlockNode)>,
    rows: HashMap<BlockId, (u64, Vec<Paragraph>)>,
    /// Nodes and rows built (not reused), for the perf tests (D9).
    pub(crate) built: usize,
}

impl NodeCache {
    /// Drop entries of blocks that left the document, once the cache has
    /// grown well past the document (amortized O(1) per pass).
    pub(crate) fn collect_garbage(&mut self, doc: &EditorDocument) {
        if self.nodes.len() + self.rows.len() > 2 * doc.len() + 64 {
            self.nodes.retain(|id, _| doc.get(*id).is_some());
            self.rows.retain(|id, _| doc.get(*id).is_some());
        }
    }

    pub(crate) fn clear(&mut self) {
        self.nodes.clear();
        self.rows.clear();
    }

    /// FORK: the open Link spans of `leaf`.
    pub(crate) fn open_of(&self, leaf: BlockId) -> Vec<Range<usize>> {
        self.open.of(leaf)
    }

    /// FORK: show these Link spans as source. Drops the cached projection of
    /// the leaves that opened or closed and their ancestors, and returns
    /// those blocks.
    pub(crate) fn set_open(
        &mut self,
        doc: &EditorDocument,
        open: super::wiki_chip::OpenSpans,
    ) -> Vec<BlockId> {
        if self.open == open {
            return Vec::new();
        }
        let old = std::mem::replace(&mut self.open, open);
        let mut dirty: Vec<BlockId> = old
            .leaves()
            .chain(self.open.leaves())
            .flat_map(|leaf| doc.path(leaf))
            .collect();
        dirty.sort_by_key(|id| id.as_u64());
        dirty.dedup();
        for id in &dirty {
            self.nodes.remove(id);
            self.rows.remove(id);
        }
        dirty
    }

    pub(crate) fn node(&mut self, doc: &EditorDocument, id: BlockId) -> Option<BlockNode> {
        let revision = doc.block_revision(id);
        let node = match self.nodes.get(&id) {
            Some((rev, node)) if *rev == revision => node.clone(),
            _ => {
                let node = self.build(doc, id)?;
                self.built += 1;
                self.nodes.insert(id, (revision, node.clone()));
                node
            }
        };
        Some(node)
    }

    fn row(&mut self, doc: &EditorDocument, id: BlockId) -> Vec<Paragraph> {
        let revision = doc.block_revision(id);
        match self.rows.get(&id) {
            Some((rev, row)) if *rev == revision => row.clone(),
            _ => {
                self.built += 1;
                let row: Vec<Paragraph> = doc
                    .children(id)
                    .iter()
                    .map(|cell| match doc.inline(*cell) {
                        Some(inline) => {
                            paragraph(inline, *cell, self.placeholders, &self.open.of(*cell))
                        }
                        None => paragraph(&InlineText::new(), *cell, self.placeholders, &[]),
                    })
                    .collect();
                self.rows.insert(id, (revision, row.clone()));
                row
            }
        }
    }

    fn build(&mut self, doc: &EditorDocument, id: BlockId) -> Option<BlockNode> {
        let entry = doc.get(id)?;
        let children = |this: &mut Self| -> Vec<BlockNode> {
            doc.children(id)
                .iter()
                .filter_map(|child| this.node(doc, *child))
                .collect()
        };
        let open = self.open.of(id);
        let inline = || match &entry.content {
            BlockContent::Inline(inline) => paragraph(inline, id, self.placeholders, &open),
            _ => paragraph(&InlineText::new(), id, self.placeholders, &[]),
        };
        Some(match &entry.kind {
            BlockKindData::Root => BlockNode::Root {
                children: children(self),
                span: None,
            },
            BlockKindData::Paragraph => BlockNode::Paragraph(inline()),
            BlockKindData::Heading { level } => BlockNode::Heading {
                level: *level,
                children: inline(),
                span: None,
            },
            // FORK: a Callout projects as a plain quote; `element.rs` draws
            // its box.
            BlockKindData::Blockquote { .. } => BlockNode::Blockquote {
                children: children(self),
                span: None,
            },
            BlockKindData::List { ordered, start, .. } => BlockNode::List {
                children: children(self),
                ordered: *ordered,
                start: *start,
                span: None,
            },
            BlockKindData::ListItem { checked, spread } => BlockNode::ListItem {
                children: children(self),
                spread: *spread,
                checked: *checked,
                // FORK PATCH: field added in gpui-kit 0.9.1; the projection has no source spans.
                task_marker: None,
                span: None,
            },
            BlockKindData::CodeBlock { lang, meta } => {
                let code = match &entry.content {
                    BlockContent::Plain(code) => code.clone(),
                    _ => String::new(),
                };
                let code = CodeBlock::new(code.into(), lang.clone(), None::<Span>)
                    .with_meta(meta.clone())
                    .with_leaf_key(id.as_u64());
                BlockNode::CodeBlock(code)
            }
            BlockKindData::Table { aligns, .. } => BlockNode::Table(Table::new(
                doc.children(id)
                    .iter()
                    .map(|row| self.row(doc, *row))
                    .collect(),
                aligns.clone(),
            )),
            BlockKindData::TableRow | BlockKindData::TableCell => return None,
            BlockKindData::HorizontalRule => BlockNode::HorizontalRule { span: None },
            BlockKindData::Break { html } => BlockNode::Break {
                html: *html,
                span: None,
            },
            BlockKindData::Custom => match &entry.content {
                BlockContent::Atom(AtomData::Custom(node)) => BlockNode::Custom {
                    node: node.clone(),
                    state: BlockState::default(),
                },
                _ => BlockNode::Unknown,
            },
            BlockKindData::Definition {
                identifier,
                url,
                title,
            } => BlockNode::Definition {
                identifier: identifier.clone(),
                url: url.clone(),
                title: title.clone(),
                span: None,
            },
            BlockKindData::Unknown => BlockNode::Unknown,
        })
    }
}
