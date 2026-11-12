//! FORK: draws a top-level Callout: a tinted box with a coloured left edge
//! and a header row (the type's icon and name), its blocks inside.
//!
//! The editor draws the box itself instead of using gpui-kit's `Surface`
//! node (#16): a Surface draws nothing for an empty callout, so its caret
//! line would vanish, and its container can't tell which callout it is. The
//! quote's children are rendered with `BlockNode::render`, as gpui-kit's
//! Surface arm does, so caret, selection and clicks work as anywhere else.

use gpui::{
    AnyElement, App, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    Role, StatefulInteractiveElement as _, Styled as _, Window, div, px, rems,
};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::base::text::editor_host::{BlockNode, FlowPosition, RenderContext};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, tooltip::Tooltip, v_flex};

use crate::{
    color::{over, readable},
    frame::{BoundsSink, record_bounds},
    id::BlockId,
    kind::CalloutKind,
};

// Lucide icons (ISC, see assets/icons/LICENSE-LUCIDE), embedded like the
// toolbar's so apps need no asset-source wiring.
const ICON_NOTE: &[u8] = include_bytes!("../assets/icons/info.svg");
const ICON_TIP: &[u8] = include_bytes!("../assets/icons/lightbulb.svg");
const ICON_IMPORTANT: &[u8] = include_bytes!("../assets/icons/message-square-warning.svg");
const ICON_WARNING: &[u8] = include_bytes!("../assets/icons/triangle-alert.svg");
const ICON_CAUTION: &[u8] = include_bytes!("../assets/icons/octagon-alert.svg");
const ICON_CHEVRON: &[u8] = include_bytes!("../assets/icons/chevron-down.svg");

/// How strongly the type's colour tints the box.
pub(crate) const TINT: f32 = 0.08;

pub(crate) fn icon(kind: CalloutKind) -> &'static [u8] {
    match kind {
        CalloutKind::Note => ICON_NOTE,
        CalloutKind::Tip => ICON_TIP,
        CalloutKind::Important => ICON_IMPORTANT,
        CalloutKind::Warning => ICON_WARNING,
        CalloutKind::Caution => ICON_CAUTION,
    }
}

/// The type's colour.
pub(crate) fn color(kind: CalloutKind, cx: &App) -> Hsla {
    let theme = cx.theme();
    match kind {
        CalloutKind::Note => theme.info,
        CalloutKind::Tip => theme.success,
        // gpui-kit has no semantic "important" token, and the fork has no
        // theme layer of its own to add one, so this takes the palette's
        // magenta.
        CalloutKind::Important => theme.magenta,
        CalloutKind::Warning => theme.warning,
        CalloutKind::Caution => theme.danger,
    }
}

/// One top-level Callout to draw: the quote `id` of type `kind`, holding
/// `children`.
pub(crate) struct CalloutBox<'a> {
    pub(crate) kind: CalloutKind,
    pub(crate) id: BlockId,
    pub(crate) children: &'a [BlockNode],
    /// Drops the gap below it, as for the editor's last block.
    pub(crate) is_last: bool,
    pub(crate) node_cx: &'a RenderContext,
    /// Where the header's bounds are recorded this frame.
    pub(crate) headers: &'a BoundsSink,
}

pub(crate) fn render(callout: CalloutBox, window: &mut Window, cx: &mut App) -> AnyElement {
    let CalloutBox {
        kind,
        id,
        children,
        is_last,
        node_cx,
        headers,
    } = callout;
    let base = color(kind, cx);
    let theme = cx.theme();
    let tint = base.opacity(TINT);
    // The type colours are fill colours. In the default light theme `info`,
    // `success`, `warning` and `danger` are too light for 13px text on their
    // own 8% tint (yellow-500 is about 1.8:1), so the label takes a deeper
    // shade of the same hue. The icon and edge keep the base colour: they
    // aren't text.
    let label = readable(base, over(tint, theme.background));
    let muted = theme.muted_foreground;
    let radius = theme.radius;
    let gap = node_cx.style().paragraph_gap();

    let header = h_flex()
        .id(("callout-header", id.as_u64() as usize))
        .role(Role::Button)
        .test_support()
        .aria_label(format!("{} callout", kind.label()))
        // Clicking opens the type menu; the tooltip names it and its
        // otherwise hidden shortcut.
        .tooltip(|window, cx| {
            Tooltip::new("Change callout type")
                .action(&crate::state::OpenCalloutMenu, Some(crate::state::CONTEXT))
                .build(window, cx)
        })
        // No tab stop: the editor owns the only focus, so the keyboard
        // route is the `cmd-alt-c` shortcut with the caret in the callout.
        // This is #19's exception to CODING_STANDARDS.md's "everything
        // clickable is reachable from the keyboard: focusable, opens on
        // Enter".
        .relative()
        .self_start()
        // Icon, name and chevron are parts of one control.
        .gap_1()
        .mb(gap)
        // 13px at the default rem size.
        .text_size(rems(0.8125))
        .font_weight(FontWeight::MEDIUM)
        .text_color(label)
        .child(Icon::default().data(icon(kind)).small().text_color(base))
        .child(kind.label())
        .child(
            Icon::default()
                .data(ICON_CHEVRON)
                .xsmall()
                .text_color(muted),
        )
        // The type menu hit-tests these bounds (`FrameLayout::callout_headers`).
        .child(record_bounds(headers, id));

    let nodes: Vec<&BlockNode> = children.iter().collect();
    let blocks: Vec<AnyElement> = children
        .iter()
        .enumerate()
        .map(|(ix, child)| {
            let flow = FlowPosition::at(&nodes, ix, ix + 1 == children.len());
            child.render(ix, flow, node_cx, window, cx)
        })
        .collect();

    let callout = v_flex()
        .w_full()
        .min_w_0()
        .px_3()
        .py_2()
        .rounded(radius)
        .border_l(px(3.))
        .border_color(base)
        .bg(tint)
        .child(header)
        .children(blocks);
    if is_last {
        callout.into_any_element()
    } else {
        div().pb(gap).child(callout).into_any_element()
    }
}
