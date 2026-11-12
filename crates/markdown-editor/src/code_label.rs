//! FORK: a Code Block's language label, in its top-right corner.
//!
//! gpui-base draws its own plain label inside `CodeBlock::render` and offers
//! no switch for it (`RenderContext` has no `code_block_actions` setter). The
//! narrowest patch is to leave the block to gpui-base, with its top padding
//! raised so the corner is clear even without a language, and paint our label
//! over its label: a patch in the block's own background colour, then the
//! control. Highlighting, selection and the caret stay gpui-base's.
//!
//! The label shows the curated display name ("Rust" for `rs`), "Plain text"
//! for a bare fence, or an amber dot and the text as written when the lookup
//! says the language won't highlight. Its bounds are recorded each frame in
//! `FrameLayout::code_labels` for hit-testing (`BlockEditorState::code_label_at`).

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Role, StatefulInteractiveElement as _, Styled as _, Window, canvas, div,
    prelude::FluentBuilder as _, px,
};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::base::text::editor_host::{BlockNode, FlowPosition, RenderContext};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, tooltip::Tooltip};

use crate::{id::BlockId, language};

const ICON_CHEVRON: &[u8] = include_bytes!("../assets/icons/chevron-down.svg");

/// Space above the code, so the first line clears the corner.
const TOP_PADDING: f32 = 28.;

/// Labels drawn this frame, by Code Block, in window coordinates.
pub(crate) type LabelSink = Rc<RefCell<Vec<(BlockId, Bounds<Pixels>)>>>;

/// Fence names that mean "no language", on purpose.
const PLAIN_NAMES: [&str; 4] = ["text", "plain", "plaintext", "txt"];

/// What a Code Block's label says.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Label {
    /// No language.
    PlainText,
    /// A language that highlights, as shown.
    Known(String),
    /// A language that won't highlight, as written.
    Unknown(String),
}

impl Label {
    pub(crate) fn of(lang: Option<&str>) -> Self {
        match lang.filter(|l| !l.is_empty()) {
            None => Label::PlainText,
            // Plain on purpose (decision #60), not an unknown language.
            Some(l) if PLAIN_NAMES.iter().any(|p| p.eq_ignore_ascii_case(l)) => Label::PlainText,
            // Highlighting decides first: a search-only alias such as `shell`
            // has a display name but still won't highlight.
            Some(l) if !language::highlights(l) => Label::Unknown(l.to_string()),
            Some(l) => Label::Known(language::display_name(l).unwrap_or(l).to_string()),
        }
    }

    pub(crate) fn text(&self) -> &str {
        match self {
            Label::PlainText => "Plain text",
            Label::Known(t) | Label::Unknown(t) => t,
        }
    }

    pub(crate) fn aria(&self) -> String {
        format!("Language: {}", self.text())
    }
}

/// One Code Block to draw.
pub(crate) struct CodeBox<'a> {
    pub(crate) node: &'a BlockNode,
    pub(crate) id: BlockId,
    pub(crate) ix: usize,
    pub(crate) flow: FlowPosition,
    pub(crate) node_cx: &'a RenderContext,
    /// Where the label's bounds are recorded this frame.
    pub(crate) labels: &'a LabelSink,
}

pub(crate) fn render(code_box: CodeBox, window: &mut Window, cx: &mut App) -> AnyElement {
    let CodeBox {
        node,
        id,
        ix,
        flow,
        node_cx,
        labels: sink,
    } = code_box;
    let BlockNode::CodeBlock(code) = node else {
        return node.render(ix, flow, node_cx, window, cx);
    };
    let lang = code.lang();
    let label = Label::of(lang.as_deref());

    let mut refinement = node_cx.style().code_block().clone();
    refinement.padding.top = Some(px(TOP_PADDING).into());
    let local = node_cx
        .clone()
        .with_style(node_cx.style().clone().with_code_block(refinement));
    let block = node.render(ix, flow, &local, window, cx);

    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let hover_bg = {
        // The ghost Button's hover.
        if theme.mode.is_dark() {
            theme.accent.opacity(0.5)
        } else {
            theme.accent
        }
    };
    let hover_fg = theme.foreground;
    let warning = theme.warning;
    let radius = theme.radius;
    let font = theme.font_family.clone();
    let text_color = match label {
        Label::PlainText => muted.opacity(0.75),
        _ => muted,
    };

    // Covers gpui-base's own label, which has no switch. Only a block with a
    // language has one. The top padding is empty, so the patch is a strip
    // across the block inside its border, as tall as that padding: a long
    // language name can't leak out to the left of a fixed-width box.
    // FORK: a `code_block()` background refinement would need this patch to
    // follow it, since it takes `code_background()` as the block's fill.
    let patch = lang.as_deref().is_some_and(|l| !l.is_empty()).then(|| {
        div()
            .absolute()
            .top(px(1.))
            .left(px(1.))
            .right(px(1.))
            .h(px(TOP_PADDING))
            .bg(node_cx.style().code_background())
    });

    let tip = match &label {
        Label::Unknown(t) => format!("`{t}` isn't highlighted \u{b7} Change language"),
        _ => "Change language".to_string(),
    };
    let unknown = matches!(label, Label::Unknown(_));
    let sink = sink.clone();
    let control = h_flex()
        .id(("code-language", id.as_u64() as usize))
        .role(Role::Button)
        .test_support()
        .aria_label(label.aria())
        // Not a tab stop, like the Callout header: the editor owns the one
        // focus and `cmd-alt-l` is the keyboard route.
        .tooltip(move |window, cx| {
            Tooltip::new(tip.clone())
                .action(&crate::state::OpenLanguageMenu, Some(crate::state::CONTEXT))
                .build(window, cx)
        })
        .absolute()
        .top(px(5.))
        .right(px(8.))
        .gap_1()
        .px_1p5()
        .py_0p5()
        .rounded(radius)
        .font_family(font)
        .text_xs()
        .text_color(text_color)
        .hover(move |this| this.bg(hover_bg).text_color(hover_fg))
        .when(unknown, |this| {
            this.child(
                div()
                    .id("code-language-unknown-dot")
                    .test_support()
                    .size(px(6.))
                    .rounded_full()
                    .bg(warning),
            )
        })
        .child(label.text().to_string())
        .child(Icon::default().data(ICON_CHEVRON).xsmall())
        .child(
            canvas(
                move |bounds, _, _| sink.borrow_mut().push((id, bounds)),
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );

    div()
        .relative()
        .w_full()
        .child(block)
        .children(patch)
        .child(control)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_read_per_language_kind() {
        assert_eq!(Label::of(Some("rs")).text(), "Rust");
        assert_eq!(Label::of(Some("rust")).aria(), "Language: Rust");
        assert_eq!(Label::of(None).text(), "Plain text");
        assert_eq!(Label::of(Some("")).text(), "Plain text");
        // Highlights but isn't in the curated list: as written.
        assert_eq!(Label::of(Some("haskell")), Label::Known("haskell".into()));
        assert_eq!(Label::of(Some("foo")), Label::Unknown("foo".into()));
    }

    #[test]
    fn a_search_only_alias_reads_as_unknown() {
        // `shell` finds Bash in the menu but doesn't highlight.
        assert_eq!(Label::of(Some("shell")), Label::Unknown("shell".into()));
    }

    #[test]
    fn plain_on_purpose_names_read_plain_text() {
        for name in ["text", "plain", "plaintext", "txt", "Text", "PLAINTEXT"] {
            assert_eq!(Label::of(Some(name)), Label::PlainText, "{name}");
        }
    }
}
