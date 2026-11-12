//! FORK: Links (`[[slug]]`) drawn as title chips.
//!
//! The model keeps `[[slug]]` as plain text. Only the projection swaps a
//! closed span for one chip object, so saving, copying, pasting and undo never
//! see it. This module finds the spans and converts between model offsets and
//! the renderer's, where a chip is a single `U+FFFC` (3 bytes).

use std::ops::Range;

use gpui::SharedString;
use gpui_kit::base::text::{MarkdownNode, editor_host::InlineNode};

use super::{
    document::EditorDocument, id::BlockId, inline::InlineText, markdown_source::wiki_link_spans,
    selection::BlockSelection,
};

/// The three parts of a Link's inner text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LinkParts<'a> {
    pub(crate) slug: &'a str,
    pub(crate) label: Option<&'a str>,
    pub(crate) heading: Option<&'a str>,
}

/// Splits the text between `[[` and `]]`. The slug is everything before the
/// first `|` or `#`, trimmed; the label is the text after the first `|`; the
/// heading is the text between the first `#` and the `|`, or the end.
pub(crate) fn link_parts(inner: &str) -> LinkParts<'_> {
    let (head, label) = match inner.split_once('|') {
        Some((head, label)) => (head, Some(label.trim())),
        None => (inner, None),
    };
    let (slug, heading) = match head.split_once('#') {
        Some((slug, heading)) => (slug, Some(heading.trim())),
        None => (head, None),
    };
    LinkParts {
        slug: slug.trim(),
        label: label.filter(|l| !l.is_empty()),
        heading: heading.filter(|h| !h.is_empty()),
    }
}

/// The Link spans of `inline` that show as chips: the ones the serializer
/// writes bare, in runs without the `code` or link mark. A span split across
/// mark runs is not one, because the serializer sees one run at a time.
pub(crate) fn chip_spans(inline: &InlineText) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    for (range, mark, atom) in inline.pieces() {
        if atom.is_some() || mark.code || mark.link.is_some() {
            continue;
        }
        let start = range.start;
        spans.extend(
            wiki_link_spans(&inline.text()[range])
                .into_iter()
                .map(|span| start + span.start..start + span.end),
        );
    }
    spans
}

/// What the plugin needs to draw one chip: the text between `[[` and `]]`.
pub(crate) struct ChipData {
    pub(crate) inner: SharedString,
}

/// The name of the chip's inline plugin.
pub(crate) const CHIP_NODE: &str = "sett-wiki-chip";

/// The chip object for the Link `source` (`[[slug]]`). Its fallback text and
/// accessibility label are the source.
pub(crate) fn chip_node(source: &str) -> InlineNode {
    let inner = source
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .unwrap_or(source);
    InlineNode::custom(
        MarkdownNode::new(
            CHIP_NODE,
            ChipData {
                inner: inner.to_string().into(),
            },
        )
        .text(source.to_string())
        .accessibility_label(source.to_string()),
    )
}

/// The Link spans shown as source: one that has a selection end strictly
/// inside it, or that the IME is composing in. View state only; nothing in
/// the document changes when it does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct OpenSpans(Vec<(BlockId, Range<usize>)>);

impl OpenSpans {
    pub(crate) fn new(
        doc: &EditorDocument,
        selection: &BlockSelection,
        marked: Option<(BlockId, Range<usize>)>,
    ) -> Self {
        let mut open: Vec<(BlockId, Range<usize>)> = Vec::new();
        let mut add = |block: BlockId, hit: &dyn Fn(&Range<usize>) -> bool| {
            let Some(inline) = doc.inline(block) else {
                return;
            };
            if !inline.text().contains("[[") {
                return;
            }
            for span in chip_spans(inline).into_iter().filter(|s| hit(s)) {
                if !open.contains(&(block, span.clone())) {
                    open.push((block, span));
                }
            }
        };
        if let BlockSelection::Text { anchor, head } = selection {
            for end in [anchor, head] {
                let at = end.offset;
                add(end.block, &|s| s.start < at && at < s.end);
            }
        }
        if let Some((block, range)) = marked {
            add(block, &|s| s.start < range.end && range.start < s.end);
        }
        open.sort_by_key(|(block, span)| (block.as_u64(), span.start));
        Self(open)
    }

    /// The open spans of `leaf`.
    pub(crate) fn of(&self, leaf: BlockId) -> Vec<Range<usize>> {
        self.0
            .iter()
            .filter(|(block, _)| *block == leaf)
            .map(|(_, span)| span.clone())
            .collect()
    }

    pub(crate) fn leaves(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.0.iter().map(|(block, _)| *block)
    }
}

/// Bytes a closed chip takes in the renderer's text: one `U+FFFC`.
pub(crate) const CHIP_LEN: usize = 3;

/// Converts between model offsets and the renderer's in one leaf, where each
/// closed chip is [`CHIP_LEN`] bytes however long its `[[slug]]` source is.
/// An offset inside a closed span snaps to its nearer edge. The open span, if
/// any, is plain text and not in the map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChipMap {
    /// Closed spans in model offsets, sorted and disjoint.
    spans: Vec<Range<usize>>,
}

impl ChipMap {
    /// `spans` are model ranges, sorted and disjoint. `None` when empty.
    pub(crate) fn new(spans: Vec<Range<usize>>) -> Option<Self> {
        (!spans.is_empty()).then_some(Self { spans })
    }

    /// Bytes the chips before the model offset `at` took off the renderer's text.
    fn shrink_before(&self, at: usize) -> usize {
        self.spans
            .iter()
            .take_while(|span| span.end <= at)
            .map(|span| span.len() - CHIP_LEN)
            .sum()
    }

    /// The renderer's index for a model offset.
    pub(crate) fn to_rendered(&self, offset: usize) -> usize {
        if let Some(span) = self
            .spans
            .iter()
            .find(|s| s.start < offset && offset < s.end)
        {
            let start = span.start - self.shrink_before(span.start);
            let near_end = (offset - span.start) * 2 > span.len();
            return start + if near_end { CHIP_LEN } else { 0 };
        }
        offset - self.shrink_before(offset)
    }

    /// The model offset for a renderer index. An index inside a chip's
    /// placeholder gives the nearer edge of its span.
    pub(crate) fn to_model(&self, index: usize) -> usize {
        let mut shrink = 0;
        for span in &self.spans {
            let start = span.start - shrink;
            if index <= start {
                break;
            }
            if index < start + CHIP_LEN {
                let near_end = (index - start) * 2 > CHIP_LEN;
                return if near_end { span.end } else { span.start };
            }
            shrink += span.len() - CHIP_LEN;
        }
        index + shrink
    }

    /// The renderer's range for a model range, widened to whole chips.
    pub(crate) fn range_to_rendered(&self, range: Range<usize>) -> Range<usize> {
        let widen = |offset: usize, up: bool| match self
            .spans
            .iter()
            .find(|s| s.start < offset && offset < s.end)
        {
            Some(span) => self.to_rendered(if up { span.end } else { span.start }),
            None => self.to_rendered(offset),
        };
        widen(range.start, false)..widen(range.end, true)
    }

    /// The span whose placeholder is the renderer's `rendered` range.
    pub(crate) fn span_at_rendered(&self, rendered: &Range<usize>) -> Option<Range<usize>> {
        self.spans
            .iter()
            .find(|span| {
                let start = span.start - self.shrink_before(span.start);
                start == rendered.start && rendered.len() == CHIP_LEN
            })
            .cloned()
    }

    /// The renderer's length for a leaf of `model_len` bytes.
    #[cfg(test)]
    pub(crate) fn rendered_len(&self, model_len: usize) -> usize {
        model_len - self.shrink_before(model_len)
    }
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)] // Test data: one-element lists of spans.
mod tests {
    use super::*;
    use gpui_kit::base::text::editor_host::{LinkMark, TextMark};

    fn plain(text: &str) -> InlineText {
        InlineText::plain(text, TextMark::default())
    }

    fn texts(inline: &InlineText) -> Vec<&str> {
        chip_spans(inline)
            .into_iter()
            .map(|r| &inline.text()[r])
            .collect()
    }

    #[test]
    fn finds_plain_labelled_and_heading_links() {
        let inline = plain("a [[one]] b [[two|the two]] c [[three#H]] d [[four#H|L]]");
        assert_eq!(
            texts(&inline),
            ["[[one]]", "[[two|the two]]", "[[three#H]]", "[[four#H|L]]"]
        );
    }

    #[test]
    fn splits_inner_text() {
        assert_eq!(
            link_parts(" budget #Quotes |the sheet"),
            LinkParts {
                slug: "budget",
                label: Some("the sheet"),
                heading: Some("Quotes")
            }
        );
        assert_eq!(
            link_parts("a|b#c"),
            LinkParts {
                slug: "a",
                label: Some("b#c"),
                heading: None
            }
        );
        assert_eq!(
            link_parts("plain"),
            LinkParts {
                slug: "plain",
                label: None,
                heading: None
            }
        );
    }

    #[test]
    fn skips_code_and_link_runs() {
        let mut inline = plain("x [[a]] y");
        inline.map_marks(2..7, |m| TextMark {
            code: true,
            ..m.clone()
        });
        assert!(chip_spans(&inline).is_empty());
        let mut inline = plain("x [[a]] y");
        inline.map_marks(2..7, |m| TextMark {
            link: Some(LinkMark {
                url: "https://x.test".into(),
                identifier: None,
                title: None,
            }),
            ..m.clone()
        });
        assert!(chip_spans(&inline).is_empty());
    }

    #[test]
    fn skips_a_span_split_across_mark_runs() {
        let mut inline = plain("[[ab]]");
        inline.map_marks(0..3, |m| TextMark {
            bold: true,
            ..m.clone()
        });
        assert!(chip_spans(&inline).is_empty());
        let mut inline = plain("[[ab]] [[cd]]");
        inline.map_marks(9..11, |m| TextMark {
            bold: true,
            ..m.clone()
        });
        assert_eq!(texts(&inline), ["[[ab]]"]);
    }

    #[test]
    fn offsets_are_in_the_whole_leaf() {
        let mut inline = plain("ab [[c]]");
        inline.map_marks(0..2, |m| TextMark {
            bold: true,
            ..m.clone()
        });
        assert_eq!(chip_spans(&inline), [3..8]);
    }

    fn map(spans: &[Range<usize>]) -> ChipMap {
        ChipMap::new(spans.to_vec()).unwrap()
    }

    #[test]
    fn maps_offsets_around_a_chip() {
        // "ab [[slug]] cd": the chip is bytes 3..11.
        let map = map(&[3..11]);
        assert_eq!(map.to_rendered(0), 0);
        assert_eq!(map.to_rendered(3), 3);
        assert_eq!(map.to_rendered(11), 6);
        assert_eq!(map.to_rendered(14), 9);
        assert_eq!(map.to_model(3), 3);
        assert_eq!(map.to_model(6), 11);
        assert_eq!(map.to_model(9), 14);
        // Inside the chip: the nearer edge.
        assert_eq!(map.to_rendered(5), 3);
        assert_eq!(map.to_rendered(9), 6);
        assert_eq!(map.to_model(4), 3);
        assert_eq!(map.to_model(5), 11);
    }

    #[test]
    fn widens_selection_ranges_to_whole_chips() {
        let map = map(&[3..11]);
        assert_eq!(map.range_to_rendered(0..5), 0..6);
        assert_eq!(map.range_to_rendered(7..14), 3..9);
        assert_eq!(map.range_to_rendered(1..2), 1..2);
        assert_eq!(map.range_to_rendered(3..11), 3..6);
    }

    #[test]
    fn finds_a_chip_by_its_placeholder() {
        let map = map(&[0..8, 10..20]);
        assert_eq!(map.span_at_rendered(&(0..3)), Some(0..8));
        assert_eq!(map.span_at_rendered(&(5..8)), Some(10..20));
        assert_eq!(map.span_at_rendered(&(3..5)), None);
        assert_eq!(map.span_at_rendered(&(3..6)), None);
    }

    #[test]
    fn round_trips_every_offset_to_itself_or_the_nearer_edge() {
        let layouts: [&[Range<usize>]; 4] =
            [&[0..7], &[2..9, 10..18], &[3..11, 11..16, 20..28], &[5..12]];
        for spans in layouts {
            let map = map(spans);
            let len = spans.last().unwrap().end + 4;
            for offset in 0..=len {
                let back = map.to_model(map.to_rendered(offset));
                match spans.iter().find(|s| s.start < offset && offset < s.end) {
                    None => assert_eq!(back, offset, "{spans:?} at {offset}"),
                    Some(s) => {
                        let nearer = if (offset - s.start) * 2 > s.len() {
                            s.end
                        } else {
                            s.start
                        };
                        assert_eq!(back, nearer, "{spans:?} at {offset}");
                    }
                }
            }
            for index in 0..=map.rendered_len(len) {
                assert!(map.to_rendered(map.to_model(index)) <= map.rendered_len(len));
            }
        }
    }
}
