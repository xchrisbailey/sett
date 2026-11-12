//! FORK: what the Insert Menu and the Callout type menu share: the open
//! menu's highlight, and the popover and option rows they are drawn with.
//!
//! They are custom rather than gpui-kit's `PopupMenu` because focus has to
//! stay in the editor: the caret keeps blinking, typing keeps filtering the
//! Insert Menu, and the menu keys run through the editor's own key context.
//! A `PopupMenu` takes focus when it opens.

use gpui::{
    AnyElement, App, Bounds, Context, Entity, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, Pixels, Point, Role, SharedString, StatefulInteractiveElement as _, Styled,
    anchored, deferred, div, point, prelude::FluentBuilder as _, px, rems,
};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::ActiveTheme as _;

use super::{id::BlockId, state::BlockEditorState};

/// How far below its anchor a menu hangs.
const ANCHOR_GAP: Pixels = px(4.);
/// The closest a menu comes to the window's edges.
const WINDOW_MARGIN: Pixels = px(8.);

/// An open menu: the block it belongs to, its highlighted option and, for a
/// menu that filters by typed text (the Language Menu), that query. Holding
/// the query here means it can't outlive the menu.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpenMenu {
    pub(crate) block: BlockId,
    pub(crate) highlighted: usize,
    pub(crate) query: String,
}

impl OpenMenu {
    pub(crate) fn new(block: BlockId, highlighted: usize) -> Self {
        Self {
            block,
            highlighted,
            query: String::new(),
        }
    }

    /// Move the highlight down (`forward`) or up among `count` options,
    /// wrapping at the ends.
    fn step(&mut self, forward: bool, count: usize) {
        if count == 0 {
            return;
        }
        self.highlighted = if forward {
            (self.highlighted + 1) % count
        } else {
            (self.highlighted + count - 1) % count
        };
    }
}

/// Which of the editor's menus a shared handler acts on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MenuSlot {
    Insert,
    Callout,
    Language,
    Link,
}

impl MenuSlot {
    /// The popover's element id and accessible name.
    fn popover(self) -> (&'static str, &'static str) {
        match self {
            Self::Insert => ("block-editor-insert-menu", "Insert Menu"),
            Self::Callout => ("block-editor-callout-menu", "Callout type"),
            Self::Language => ("block-editor-language-menu", "Language menu"),
            Self::Link => ("block-editor-link-picker", "Link Picker"),
        }
    }

    /// The element id prefix of the menu's options.
    fn option_id(self) -> &'static str {
        match self {
            Self::Insert => "insert-menu-option",
            Self::Callout => "callout-menu-option",
            Self::Language => "language-menu-option",
            Self::Link => "link-picker-option",
        }
    }

    fn pick(self, state: &mut BlockEditorState, ix: usize, cx: &mut Context<BlockEditorState>) {
        match self {
            Self::Insert => state.insert_menu_pick(ix, cx),
            Self::Callout => state.callout_menu_pick(ix, cx),
            Self::Language => state.language_menu_pick(ix, cx),
            Self::Link => state.link_picker_pick(ix, cx),
        }
    }
}

impl BlockEditorState {
    fn menu_slot(&mut self, slot: MenuSlot) -> &mut Option<OpenMenu> {
        match slot {
            MenuSlot::Insert => &mut self.insert_menu,
            MenuSlot::Callout => &mut self.callout_menu,
            MenuSlot::Language => &mut self.language_menu,
            MenuSlot::Link => &mut self.link_picker,
        }
    }

    pub(crate) fn close_menu(&mut self, slot: MenuSlot, cx: &mut Context<Self>) {
        if self.menu_slot(slot).take().is_some() {
            cx.notify();
        }
    }

    /// Move `slot`'s highlight among its `count` options, wrapping.
    pub(crate) fn step_menu(
        &mut self,
        slot: MenuSlot,
        forward: bool,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.menu_slot(slot) {
            menu.step(forward, count);
            cx.notify();
        }
    }

    fn highlight_menu_option(&mut self, slot: MenuSlot, ix: usize, cx: &mut Context<Self>) {
        // A Link Picker row that can't be picked can't be highlighted either.
        if slot == MenuSlot::Link && !self.link_row_pickable(ix) {
            return;
        }
        if let Some(menu) = self.menu_slot(slot)
            && menu.highlighted != ix
        {
            menu.highlighted = ix;
            cx.notify();
        }
    }
}

/// Where a menu hangs from: just under `anchor`.
pub(crate) fn below(anchor: Bounds<Pixels>) -> Point<Pixels> {
    point(anchor.left(), anchor.bottom() + ANCHOR_GAP)
}

/// A menu's text colour for an option, highlighted (`on`) or not.
pub(crate) fn option_color(on: bool, cx: &App) -> Hsla {
    let theme = cx.theme();
    if on {
        theme.accent_foreground
    } else {
        theme.popover_foreground
    }
}

/// Option `ix` of `slot`'s menu, highlighted when `on`: hovering highlights
/// it, and a click picks it. The caller adds its content.
pub(crate) fn option_row(
    state: &Entity<BlockEditorState>,
    slot: MenuSlot,
    ix: usize,
    label: impl Into<SharedString>,
    on: bool,
    cx: &App,
) -> impl IntoElement + ParentElement + Styled {
    let theme = cx.theme();
    let hover = state.clone();
    let click = state.clone();
    div()
        .id((slot.option_id(), ix))
        .role(Role::ListBoxOption)
        .test_support()
        .aria_label(label.into())
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(theme.radius)
        .text_color(option_color(on, cx))
        .when(on, |this| this.bg(theme.accent))
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                hover.update(cx, |state, cx| state.highlight_menu_option(slot, ix, cx));
            }
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            click.update(cx, |state, cx| slot.pick(state, ix, cx));
        })
}

/// `slot`'s menu as a deferred popover hanging from `at` (window
/// coordinates), kept inside the window. A click outside closes it.
pub(crate) fn popover(
    state: &Entity<BlockEditorState>,
    slot: MenuSlot,
    at: Point<Pixels>,
    options: impl IntoIterator<Item = AnyElement>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let (id, label) = slot.popover();
    let state = state.clone();
    deferred(
        anchored()
            .position(at)
            .position_mode(gpui::AnchoredPositionMode::Window)
            .snap_to_window_with_margin(WINDOW_MARGIN)
            .child(
                div()
                    .id(id)
                    .role(Role::ListBox)
                    .test_support()
                    .aria_label(label)
                    .occlude()
                    .min_w(rems(15.))
                    .p_1()
                    .flex()
                    .flex_col()
                    .bg(theme.popover)
                    .text_sm()
                    .border_1()
                    .border_color(theme.border)
                    .rounded(theme.radius)
                    .shadow_md()
                    .on_mouse_down_out(move |_, _, cx| {
                        state.update(cx, |state, cx| state.close_menu(slot, cx));
                    })
                    .children(options),
            ),
    )
    .with_priority(1)
    .into_any_element()
}
