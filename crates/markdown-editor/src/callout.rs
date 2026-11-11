//! FORK: draws a top-level Callout: a tinted box with a coloured left edge
//! and a header row (the type's icon and name), its blocks inside.
//!
//! The editor draws the box itself instead of using gpui-kit's `Surface`
//! node (#16): a Surface draws nothing for an empty callout, so its caret
//! line would vanish, and its container can't tell which callout it is. The
//! quote's children are rendered with `BlockNode::render`, as gpui-kit's
//! Surface arm does, so caret, selection and clicks work as anywhere else.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Role, StatefulInteractiveElement as _, Styled as _, Window, canvas,
    div, px,
};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::base::text::editor_host::{BlockNode, FlowPosition, RenderContext};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};

use crate::{id::BlockId, kind::CalloutKind};

// Lucide icons (ISC, see assets/icons/LICENSE-LUCIDE), embedded like the
// toolbar's so apps need no asset-source wiring.
const ICON_NOTE: &[u8] = include_bytes!("../assets/icons/info.svg");
const ICON_TIP: &[u8] = include_bytes!("../assets/icons/lightbulb.svg");
const ICON_IMPORTANT: &[u8] = include_bytes!("../assets/icons/message-square-warning.svg");
const ICON_WARNING: &[u8] = include_bytes!("../assets/icons/triangle-alert.svg");
const ICON_CAUTION: &[u8] = include_bytes!("../assets/icons/octagon-alert.svg");
const ICON_CHEVRON: &[u8] = include_bytes!("../assets/icons/chevron-down.svg");

/// Header rows drawn this frame, by callout, in window coordinates. Filled
/// while the editor prepaints, then moved into the `FrameLayout`.
pub(crate) type HeaderSink = Rc<RefCell<Vec<(BlockId, Bounds<Pixels>)>>>;

/// How strongly the type's colour tints the box.
const TINT: f32 = 0.08;
/// WCAG AA for normal-size text.
const MIN_CONTRAST: f32 = 4.5;

fn icon(kind: CalloutKind) -> &'static [u8] {
    match kind {
        CalloutKind::Note => ICON_NOTE,
        CalloutKind::Tip => ICON_TIP,
        CalloutKind::Important => ICON_IMPORTANT,
        CalloutKind::Warning => ICON_WARNING,
        CalloutKind::Caution => ICON_CAUTION,
    }
}

/// The type's colour. Important has no semantic token, so it takes the
/// palette's magenta.
fn color(kind: CalloutKind, cx: &App) -> Hsla {
    let theme = cx.theme();
    match kind {
        CalloutKind::Note => theme.info,
        CalloutKind::Tip => theme.success,
        CalloutKind::Important => theme.magenta,
        CalloutKind::Warning => theme.warning,
        CalloutKind::Caution => theme.danger,
    }
}

/// Draws the callout `id` holding `children`. `is_last` drops the gap
/// below it, as for the editor's last block.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    kind: CalloutKind,
    id: BlockId,
    children: &[BlockNode],
    is_last: bool,
    node_cx: &RenderContext,
    headers: &HeaderSink,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let base = color(kind, cx);
    let theme = cx.theme();
    let tint = base.opacity(TINT);
    let label = readable(base, over(tint, theme.background));
    let muted = theme.muted_foreground;
    let radius = theme.radius;
    let gap = node_cx.style().paragraph_gap();

    let sink = headers.clone();
    let header = h_flex()
        .id(("callout-header", id.as_u64() as usize))
        .role(Role::Label)
        .test_support()
        .aria_label(format!("{} callout", kind.label()))
        .relative()
        .self_start()
        // Icon, name and chevron are parts of one control.
        .gap_1()
        .mb(gap)
        .text_size(px(13.))
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

/// `top`, with its alpha, laid over the opaque `bottom`.
fn over(top: Hsla, bottom: Hsla) -> Hsla {
    let (t, b) = (top.to_rgb(), bottom.to_rgb());
    let mix = |t: f32, b: f32| t * top.a + b * (1. - top.a);
    gpui::Rgba {
        r: mix(t.r, b.r),
        g: mix(t.g, b.g),
        b: mix(t.b, b.b),
        a: 1.,
    }
    .into()
}

/// `color`, darkened (on a light box) or lightened (on a dark one) until it
/// reads as text on `background`.
///
/// The type colours are fill colours. In the default light theme `info`,
/// `success`, `warning` and `danger` are too light for 13px text on their own
/// 8% tint (yellow-500 is about 1.8:1), so the label takes a deeper shade of
/// the same hue. The icon and edge keep the base colour: they aren't text.
fn readable(color: Hsla, background: Hsla) -> Hsla {
    let darken = luminance(background) > 0.5;
    let mut text = color;
    text.a = 1.;
    for _ in 0..50 {
        if contrast(text, background) >= MIN_CONTRAST {
            break;
        }
        text.l = if darken {
            (text.l - 0.02).max(0.)
        } else {
            (text.l + 0.02).min(1.)
        };
    }
    text
}

/// WCAG relative luminance.
fn luminance(color: Hsla) -> f32 {
    let rgb = color.to_rgb();
    let channel = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
}

/// WCAG contrast ratio.
fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::rgb;

    /// Default light: white page; default dark: neutral-950.
    const LIGHT: u32 = 0xffffff;
    const DARK: u32 = 0x0a0a0a;
    /// The default themes' info, success, magenta, warning and danger.
    const LIGHT_COLORS: [u32; 5] = [0x06b6d4, 0x22c55e, 0x9333ea, 0xeab308, 0xef4444];
    const DARK_COLORS: [u32; 5] = [0x22d3ee, 0x4ade80, 0xc084fc, 0xfacc15, 0xf87171];

    #[test]
    fn header_text_reads_on_its_tint_in_the_default_themes() {
        for (page, colors) in [(LIGHT, LIGHT_COLORS), (DARK, DARK_COLORS)] {
            for c in colors {
                let base: Hsla = rgb(c).into();
                let box_bg = over(base.opacity(TINT), rgb(page).into());
                let label = readable(base, box_bg);
                assert!(
                    contrast(label, box_bg) >= MIN_CONTRAST,
                    "{c:06x} on {page:06x}: {}",
                    contrast(label, box_bg)
                );
            }
        }
    }
}
