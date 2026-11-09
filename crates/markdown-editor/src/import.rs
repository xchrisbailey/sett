//! `markdown::parse` output (`BlockNode`) into the editor model.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::SharedString;

use super::{
    document::{AtomData, BlockContent, EditorDocument},
    inline::{InlineAtom, InlineText},
    kind::{BlockKindData, default_list_marker},
    step::DetachedBlock,
    transaction::TransactionBuilder,
};
use gpui_kit::base::text::{
    MarkdownExtensions,
    editor_host::{BlockNode, InlineNode, LinkMark, Paragraph, Span, TextMark, parse_markdown},
};

pub(crate) fn parse_document(
    source: &str,
    extensions: Arc<MarkdownExtensions>,
) -> Result<EditorDocument, SharedString> {
    let parsed = parse_markdown(source, with_soft_breaks(&extensions))?;
    let mut doc = EditorDocument::empty(extensions);
    let root = doc.root;
    // Drop the placeholder paragraph of the empty document.
    doc.detach(root, 0);
    let mut defs = Defs::new();
    collect_definitions(&parsed, &mut defs);
    let mut blocks = Vec::new();
    for node in parsed.iter() {
        import_block(node, source, &defs, &mut blocks);
    }
    for (ix, block) in blocks.into_iter().enumerate() {
        doc.attach(root, ix, block);
    }
    // Same structural rules as after every transaction (empty list items
    // get a paragraph, an empty document gets one leaf). The steps of this
    // pass are not history.
    let mut tx = TransactionBuilder::new(&mut doc);
    tx.normalize()
        .map_err(|e| SharedString::from(format!("{e:?}")))?;
    drop(tx);
    doc.structure_dirty = true;
    doc.refresh_order();
    // Markdown parsers omit final blank paragraphs. Reconstruct that editable
    // boundary as real document nodes, never a mutable raw-source shadow.
    // Primary AST spans distinguish exterior footer trivia from line endings
    // owned by an unterminated code fence/body.
    let exterior_start = parsed
        .last()
        .and_then(BlockNode::span)
        .map_or(0, |span| span.end);
    let exterior = source.get(exterior_start..).unwrap_or_default();
    let trimmed_spaces = exterior.trim_end_matches([' ', '\t']);
    let stripped = trimmed_spaces.trim_end_matches(['\r', '\n']);
    let line_ends = &trimmed_spaces[stripped.len()..];
    let ending_count = if line_ends.contains('\n') {
        line_ends.bytes().filter(|b| *b == b'\n').count()
    } else {
        line_ends.len()
    };
    // FORK: a pair of line endings is a blank line, which restores as an
    // empty paragraph (an editing boundary). An odd one out just terminates
    // the last line; keep it as trivia rather than inventing a blank line,
    // which would grow the file by one newline on every save.
    if ending_count % 2 == 1 {
        doc.final_line_ending = if line_ends.ends_with("\r\n") {
            "\r\n"
        } else if line_ends.ends_with('\n') {
            "\n"
        } else {
            "\r"
        };
    }
    for _ in 0..ending_count / 2 {
        let index = doc.children(root).len();
        doc.attach(
            root,
            index,
            DetachedBlock::leaf(
                BlockKindData::Paragraph,
                BlockContent::Inline(InlineText::default()),
            ),
        );
    }
    doc.refresh_order();

    // CommonMark ignores terminal paragraph spaces, but in a live editing
    // buffer they separate the next typed/dictated word. Restore that trivia
    // into the document, not a second mutable source buffer. Code retains its
    // exact own body and must not inherit trivia after its closing fence.
    let trimmed = source.trim_end_matches([' ', '\t']);
    let mut tail = &source[trimmed.len()..];
    let last = *doc.leaves().last().unwrap();
    if (matches!(doc.kind(last), Some(BlockKindData::Heading { .. }))
        || doc.index_in_parent(last).is_some_and(|(parent, _)| {
            matches!(doc.kind(parent), Some(BlockKindData::ListItem { .. }))
        }))
        && doc.inline(last).is_some_and(|inline| inline.is_empty())
    {
        // One space is the empty heading's structural marker separator.
        tail = tail.strip_prefix(' ').unwrap_or(tail);
    }
    if !tail.is_empty()
        && let Some(entry) = doc.get_mut(last)
        && let BlockContent::Inline(inline) = &mut entry.content
    {
        inline.push(InlineText::plain(tail, TextMark::default()));
    }
    doc.revision = 0;
    Ok(doc)
}

fn leaf(kind: BlockKindData, content: BlockContent) -> DetachedBlock {
    DetachedBlock::leaf(kind, content)
}

fn import_children(nodes: &[BlockNode], source: &str, defs: &Defs) -> Vec<DetachedBlock> {
    let mut out = Vec::new();
    for node in nodes {
        import_block(node, source, defs, &mut out);
    }
    out
}

/// `source` is the Markdown `node` was parsed from; its spans index into it.
pub(crate) fn import_block(
    node: &BlockNode,
    source: &str,
    defs: &Defs,
    out: &mut Vec<DetachedBlock>,
) {
    let block = match node {
        // An HTML container: splice its children in place.
        BlockNode::Root { children, .. } => {
            for child in children {
                import_block(child, source, defs, out);
            }
            return;
        }
        BlockNode::Paragraph(paragraph) => leaf(
            BlockKindData::Paragraph,
            BlockContent::Inline(import_inline(paragraph, defs)),
        ),
        BlockNode::Heading {
            level, children, ..
        } => leaf(
            BlockKindData::Heading { level: *level },
            BlockContent::Inline(import_inline(children, defs)),
        ),
        BlockNode::Blockquote { children, .. } => DetachedBlock::container(
            BlockKindData::Blockquote,
            import_children(children, source, defs),
        ),
        BlockNode::List {
            children,
            ordered,
            start,
            span,
        } => DetachedBlock::container(
            BlockKindData::List {
                ordered: *ordered,
                start: *start,
                marker: list_marker(source, *span, *ordered),
            },
            import_children(children, source, defs),
        ),
        BlockNode::ListItem {
            children,
            spread,
            checked,
            ..
        } => {
            let mut children = import_children(children, source, defs);
            // A parsed bare marker is an editable empty item, not an empty
            // container to prune during normalization.
            if children.is_empty() {
                children.push(DetachedBlock::paragraph(InlineText::new()));
            }
            DetachedBlock::container(
                BlockKindData::ListItem {
                    checked: *checked,
                    spread: *spread,
                },
                children,
            )
        }
        BlockNode::CodeBlock(code) => {
            let text = code.code().to_string();
            leaf(
                BlockKindData::CodeBlock {
                    lang: code.lang(),
                    meta: code.meta(),
                },
                BlockContent::Plain(text),
            )
        }
        BlockNode::Custom { node, .. } => leaf(
            BlockKindData::Custom,
            BlockContent::Atom(AtomData::Custom(node.clone())),
        ),
        BlockNode::Table(table) => {
            let rows = table
                .rows()
                .map(|row| {
                    DetachedBlock::container(
                        BlockKindData::TableRow,
                        row.map(|cell| {
                            leaf(
                                BlockKindData::TableCell,
                                BlockContent::Inline(import_inline(cell, defs)),
                            )
                        })
                        .collect(),
                    )
                })
                .collect();
            DetachedBlock::container(
                BlockKindData::Table {
                    aligns: table.column_aligns().to_vec(),
                },
                rows,
            )
        }
        BlockNode::Break { html, .. } => leaf(
            BlockKindData::Break { html: *html },
            BlockContent::Atom(AtomData::None),
        ),
        BlockNode::HorizontalRule { .. } => leaf(
            BlockKindData::HorizontalRule,
            BlockContent::Atom(AtomData::None),
        ),
        BlockNode::Definition {
            identifier,
            url,
            title,
            ..
        } => leaf(
            BlockKindData::Definition {
                identifier: identifier.clone(),
                url: url.clone(),
                title: title.clone(),
            },
            BlockContent::Atom(AtomData::None),
        ),
        BlockNode::Unknown | _ => leaf(BlockKindData::Unknown, BlockContent::Atom(AtomData::None)),
    };
    out.push(block);
}

/// FORK: the marker the list starting at `span` was written with: the
/// bullet character, or the delimiter after an ordered list's number.
fn list_marker(source: &str, span: Option<Span>, ordered: bool) -> char {
    let written = span
        .and_then(|span| source.get(span.start..))
        .map(|rest| rest.trim_start_matches([' ', '\t']))
        .and_then(|rest| {
            if ordered {
                rest.trim_start_matches(|ch: char| ch.is_ascii_digit())
                    .chars()
                    .next()
                    .filter(|ch| matches!(ch, '.' | ')'))
            } else {
                rest.chars()
                    .next()
                    .filter(|ch| matches!(ch, '-' | '*' | '+'))
            }
        });
    written.unwrap_or(default_list_marker(ordered))
}

/// Convert a paragraph's inline nodes into normalized runs. `InlineNode.marks`
/// may overlap (`**b*c*d**` gives bold 0..3 and italic 1..2), so split at
/// every boundary and merge the marks covering each piece.
///
/// Reference links (`[text][id]`) arrive with an empty url and an
/// identifier; they are resolved against the document's definitions so the
/// canonical output is an inline link. Unresolved references (and footnote
/// links) lose their link mark: documented lossiness.
pub(crate) fn import_inline(paragraph: &Paragraph, defs: &Defs) -> InlineText {
    let mut out = InlineText::new();
    for node in paragraph.children() {
        out.push(import_inline_node(node));
    }
    let len = out.len();
    out.map_marks(0..len, |mark| match &mark.link {
        Some(link) if link.url.is_empty() => {
            let link = link
                .identifier
                .as_ref()
                .and_then(|id| defs.get(&SharedString::from(id.to_lowercase())))
                .map(|(url, title)| LinkMark {
                    url: url.clone(),
                    identifier: None,
                    title: title.clone(),
                });
            TextMark {
                link,
                ..mark.clone()
            }
        }
        _ => mark.clone(),
    });
    out
}

pub(crate) type Defs = HashMap<SharedString, (SharedString, Option<SharedString>)>;

pub(crate) fn collect_definitions(nodes: &[BlockNode], defs: &mut Defs) {
    for node in nodes {
        match node {
            BlockNode::Definition {
                identifier,
                url,
                title,
                ..
            } => {
                defs.insert(
                    identifier.to_lowercase().into(),
                    (url.clone(), title.clone()),
                );
            }
            BlockNode::Root { children, .. }
            | BlockNode::Blockquote { children, .. }
            | BlockNode::List { children, .. }
            | BlockNode::ListItem { children, .. } => collect_definitions(children, defs),
            _ => {}
        }
    }
}

fn import_inline_node(node: &InlineNode) -> InlineText {
    let merged_marks = || {
        let mut mark = TextMark::default();
        for (_, layer) in node.mark_runs() {
            mark.merge(layer.clone());
        }
        mark
    };
    if let Some(custom) = node.as_custom() {
        if custom.name() == SOFT_BREAK_TEXT {
            return soft_break_text(custom.as_text(), merged_marks());
        }
        return InlineText::atom(InlineAtom::Custom(custom.clone()), merged_marks());
    }
    if let Some(image) = node.as_image() {
        return InlineText::atom(InlineAtom::Image(image.clone()), merged_marks());
    }
    let text: &str = node.text();
    let mut cuts = vec![0, text.len()];
    for (range, _) in node.mark_runs() {
        cuts.push(range.start.min(text.len()));
        cuts.push(range.end.min(text.len()));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = InlineText::new();
    for pair in cuts.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        if lo >= hi || !text.is_char_boundary(lo) || !text.is_char_boundary(hi) {
            continue;
        }
        let mut mark = TextMark::default();
        for (range, layer) in node.mark_runs() {
            if range.start <= lo && hi <= range.end {
                mark.merge(layer.clone());
            }
        }
        out.push(InlineText::plain(&text[lo..hi], mark));
    }
    out
}

/// FORK: the inline atom a soft line break imports as. It shows as a space
/// (the paragraph still reflows) and serializes as its original line ending,
/// so hard-wrapped Markdown keeps its lines on save.
pub(crate) const SOFT_BREAK: &str = "editor-soft-break";
const SOFT_BREAK_TEXT: &str = "editor-soft-break-text";

/// FORK: gpui-kit's parser folds soft line breaks into spaces before the
/// importer sees them. A parse-only inline plugin claims every text node
/// that holds a line ending first, and [`import_inline_node`] expands it into
/// text runs and [`SOFT_BREAK`] atoms. It is not part of the document's
/// extensions, so external pastes keep their one-block-per-line behaviour.
pub(crate) fn with_soft_breaks(extensions: &MarkdownExtensions) -> Arc<MarkdownExtensions> {
    use ::markdown::mdast::Node;
    use gpui_kit::base::text::{MarkdownNode, MarkdownParseContext, MarkdownPlugin};

    struct SoftBreakText;
    impl MarkdownPlugin for SoftBreakText {
        fn name(&self) -> &str {
            SOFT_BREAK_TEXT
        }
        fn parse(&self, node: &Node, _cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
            let Node::Text(text) = node else {
                return None;
            };
            text.value
                .contains(['\n', '\r'])
                .then(|| MarkdownNode::new(SOFT_BREAK_TEXT, ()).text(text.value.clone()))
        }
    }
    Arc::new(extensions.clone().plugin(SoftBreakText))
}

fn soft_break_text(text: &str, mark: TextMark) -> InlineText {
    let mut out = InlineText::new();
    let mut rest = text;
    while let Some(at) = rest.find(['\n', '\r']) {
        let ending = if rest[at..].starts_with("\r\n") { 2 } else { 1 };
        out.push(InlineText::plain(&rest[..at], mark.clone()));
        let node = gpui_kit::base::text::MarkdownNode::new(SOFT_BREAK, ())
            .text(" ")
            .markdown(rest[at..at + ending].to_string());
        out.push(InlineText::atom(InlineAtom::Custom(node), mark.clone()));
        rest = &rest[at + ending..];
    }
    out.push(InlineText::plain(rest, mark));
    out
}

/// Footnotes and unsupported HTML are outside the editable block vocabulary. Keep their original
/// source in opaque, selectable atoms rather than accepting the display
/// parser's decorated-prose projection. Uses the same AST/parser pipeline as
/// every other editor node; no second parser or mutable source mirror.
pub(crate) fn preserve_source_extensions(extensions: MarkdownExtensions) -> MarkdownExtensions {
    use ::markdown::mdast::Node;
    use gpui_kit::base::text::{MarkdownNode, MarkdownParseContext, MarkdownPlugin};

    struct SourceAtom {
        block: bool,
    }
    impl MarkdownPlugin for SourceAtom {
        fn name(&self) -> &str {
            if self.block {
                "editor-source-block"
            } else {
                "editor-source-inline"
            }
        }
        fn is_block(&self) -> bool {
            self.block
        }
        fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
            let known_html = if let Node::Html(html) = node {
                let lower = html.value.to_ascii_lowercase();
                let tag = lower
                    .trim_start_matches('<')
                    .trim_start_matches('/')
                    .split(|ch: char| !ch.is_ascii_alphabetic())
                    .next()
                    .unwrap_or_default();
                matches!(tag, "u" | "mark" | "em" | "strong" | "s" | "del" | "br")
            } else {
                false
            };
            let accepts = if self.block {
                matches!(node, Node::FootnoteDefinition(_))
                    || (matches!(node, Node::Html(_)) && !known_html)
            } else {
                matches!(node, Node::FootnoteReference(_))
                    || (matches!(node, Node::Html(_)) && !known_html)
            };
            if !accepts {
                return None;
            }
            let source = cx.node_source(node)?;
            // Block spans may include separator line endings. The root owns
            // those boundaries; retaining them here adds a blank line on every
            // export/import cycle. Internal content remains verbatim.
            let source = if self.block {
                source.trim_end_matches(['\r', '\n'])
            } else {
                source
            };
            let source = if self.block {
                // The AST span starts after the first container marker, but
                // later source lines still include it. Strip only matching
                // continuation prefixes, preserving body indentation and lazy
                // lines. The root serializer will add the containers once.
                let start = node.position()?.start.offset;
                let line_start = cx.source()[..start].rfind('\n').map_or(0, |ix| ix + 1);
                let prefix = &cx.source()[line_start..start];
                let continuation = prefix
                    .chars()
                    .map(|ch| if ch == '>' { '>' } else { ' ' })
                    .collect::<String>();
                source
                    .split('\n')
                    .enumerate()
                    .map(|(ix, line)| {
                        if ix == 0 || continuation.is_empty() {
                            line
                        } else if line.trim_end() == continuation.trim_end() {
                            ""
                        } else {
                            line.strip_prefix(&continuation).unwrap_or(line)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                source.to_owned()
            };
            Some(
                MarkdownNode::new(self.name(), ())
                    .text(source.clone())
                    .markdown(source),
            )
        }
    }
    extensions
        .plugin(SourceAtom { block: true })
        .plugin(SourceAtom { block: false })
}
