//! FORK: what the editor's menus share: the open menu's highlight, the popover
//! and option rows they are drawn with, and the open, click and pick plumbing
//! of the menus anchored to a control (Callout, Language).
//!
//! They are custom rather than gpui-kit's `PopupMenu` because focus has to
//! stay in the editor: the caret keeps blinking, typing keeps filtering the
//! Insert Menu, and the menu keys run through the editor's own key context.
//! A `PopupMenu` takes focus when it opens.

use gpui::{
    AnyElement, App, Bounds, Context, Entity, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, Pixels, Point, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled, Window, anchored, deferred, div, linear_color_stop,
    linear_gradient, point, prelude::FluentBuilder as _, px, rems,
};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};

use super::{
    editor::BlockEditorModel, id::BlockId, state::BlockEditorState, transaction::EditOrigin,
};

const ICON_CHECK: &[u8] = include_bytes!("../assets/icons/check.svg");

/// How far below its anchor a menu hangs.
const ANCHOR_GAP: Pixels = px(4.);
/// The closest a menu comes to the window's edges.
const WINDOW_MARGIN: Pixels = px(8.);
/// The tallest a menu grows before it scrolls, if the window has room. It
/// stops partway through a row (rows are about 1.94rem), so a cut-off row
/// shows there is more below: macOS hides the scrollbar at rest.
const MAX_HEIGHT_REMS: f32 = 19.;

/// An open menu: the block it belongs to, its highlighted option and, for a
/// menu that filters by typed text (the Language Menu), that query. Holding
/// the query here means it can't outlive the menu.
#[derive(Clone, Debug)]
pub(crate) struct OpenMenu {
    pub(crate) block: BlockId,
    pub(crate) highlighted: usize,
    pub(crate) query: String,
    /// Scroll position of the option list. Living here, it starts at the top
    /// each time a menu opens.
    scroll: ScrollHandle,
    /// The next frame scrolls the highlighted row into view. Set when the
    /// highlight moves by key (not by hover, which must not fight the wheel).
    reveal: std::cell::Cell<bool>,
    /// Whether the last frame was drawn knowing the rows overflow. The scroll
    /// handle learns its size only after layout, so a menu's first frame
    /// can't tell; a frame is requested when this and the measure differ.
    overflow_drawn: std::cell::Cell<bool>,
}

impl OpenMenu {
    pub(crate) fn new(block: BlockId, highlighted: usize) -> Self {
        Self {
            block,
            highlighted,
            query: String::new(),
            scroll: ScrollHandle::new(),
            reveal: std::cell::Cell::new(highlighted > 0),
            overflow_drawn: std::cell::Cell::new(false),
        }
    }

    /// Scroll the highlighted row into view on the next frame.
    pub(crate) fn reveal_highlighted(&self) {
        self.reveal.set(true);
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
        self.reveal.set(true);
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

    /// Close the menus that belong to the document as it was when they
    /// opened: an edit or a caret move leaves them stale. The Insert Menu and
    /// Link Picker instead refilter (`sync_insert_menu`, `sync_link_picker`).
    pub(crate) fn close_block_menus(&mut self) {
        self.callout_menu = None;
        self.language_menu = None;
    }

    /// Close the editor's four menus (not the table menu, which is gpui-kit's
    /// popup and takes focus when it opens, so blur must leave it alone).
    pub(crate) fn close_editor_menus(&mut self) {
        self.insert_menu = None;
        self.link_picker = None;
        self.close_block_menus();
    }

    /// Close every menu, the table menu included. Each menu opener calls it
    /// first, so only one is ever open.
    pub(crate) fn close_all_menus(&mut self, cx: &mut Context<Self>) {
        self.close_editor_menus();
        self.close_table_menu(cx);
    }

    fn open_menu(&self, slot: MenuSlot) -> Option<&OpenMenu> {
        match slot {
            MenuSlot::Insert => &self.insert_menu,
            MenuSlot::Callout => &self.callout_menu,
            MenuSlot::Language => &self.language_menu,
            MenuSlot::Link => &self.link_picker,
        }
        .as_ref()
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

/// What the menus anchored to a control on a block (the Callout type menu on
/// its header, the Language Menu on a Code Block's label) share: finding the
/// control's bounds, opening from a click or the shortcut, and picking.
impl BlockEditorState {
    /// Open `slot`'s menu for `block`. Only the anchored menus open this way.
    fn open_anchored_menu(&mut self, slot: MenuSlot, block: BlockId, cx: &mut Context<Self>) {
        match slot {
            MenuSlot::Callout => self.open_callout_menu(block, cx),
            MenuSlot::Language => self.open_language_menu(block, cx),
            MenuSlot::Insert | MenuSlot::Link => {}
        }
    }

    /// The common start of opening an anchored menu: nothing opens read-only,
    /// and an IME composition ends. Returns whether to go on, with every other
    /// menu closed.
    pub(crate) fn begin_menu_open(&mut self, cx: &mut Context<Self>) -> bool {
        if self.readonly {
            return false;
        }
        self.end_composition();
        self.close_all_menus(cx);
        true
    }

    /// `block`'s anchor control for `slot`, in the last painted frame.
    fn anchor_bounds(&self, slot: MenuSlot, block: BlockId) -> Option<Bounds<Pixels>> {
        let frame = self.frame.borrow();
        let list = match slot {
            MenuSlot::Callout => &frame.callout_headers,
            MenuSlot::Language => &frame.code_labels,
            MenuSlot::Insert | MenuSlot::Link => return None,
        };
        let (_, bounds) = list.iter().find(|(id, _)| *id == block)?;
        Some(*bounds)
    }

    /// Whether `block`'s anchor control was drawn last frame inside the
    /// scrolled viewport (anywhere, with no scroll container).
    fn anchor_in_view(&self, slot: MenuSlot, block: BlockId) -> bool {
        self.anchor_bounds(slot, block).is_some_and(|anchor| {
            self.scroll_handle
                .as_ref()
                .is_none_or(|scroll| scroll.bounds().intersects(&anchor))
        })
    }

    /// Where `slot`'s menu hangs from: under `block`'s anchor control, in
    /// window coordinates, from the last painted frame.
    pub(crate) fn anchor_below(&self, slot: MenuSlot, block: BlockId) -> Option<Point<Pixels>> {
        self.anchor_bounds(slot, block).map(below)
    }

    /// A click on an anchor control opens its menu. Returns whether the
    /// click was on one.
    pub(crate) fn click_menu_anchor(
        &mut self,
        slot: MenuSlot,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let hit = {
            let frame = self.frame.borrow();
            match slot {
                MenuSlot::Callout => frame.callout_header_at(at),
                MenuSlot::Language => frame.code_label_at(at),
                MenuSlot::Insert | MenuSlot::Link => None,
            }
        };
        let Some(block) = hit else {
            return false;
        };
        self.open_anchored_menu(slot, block, cx);
        true
    }

    /// The shortcut: open `slot`'s menu for `block`, the block holding the
    /// caret if it is the right kind. Elsewhere (`None`) the key passes on.
    /// An anchor that wasn't drawn in view last frame (scrolled away, or not
    /// laid out yet) opens nothing, since the menu would hang from somewhere
    /// the user can't see.
    pub(crate) fn open_menu_by_shortcut(
        &mut self,
        slot: MenuSlot,
        block: Option<BlockId>,
        cx: &mut Context<Self>,
    ) {
        match block {
            Some(block) if !self.readonly => {
                if self.anchor_in_view(slot, block) {
                    self.open_anchored_menu(slot, block, cx);
                }
            }
            _ => cx.propagate(),
        }
    }

    /// Pick from `slot`'s menu: close it, and unless `unchanged` says the
    /// block already has the choice, run `edit` on the block as one undo
    /// step, leaving the caret where it is. `edit` returns whether it changed
    /// the document.
    pub(crate) fn pick_anchored(
        &mut self,
        slot: MenuSlot,
        unchanged: impl FnOnce(&Self, BlockId) -> bool,
        origin: &'static str,
        edit: impl FnOnce(&mut BlockEditorModel, BlockId) -> bool,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.menu_slot(slot).take() else {
            return;
        };
        cx.notify();
        if unchanged(self, menu.block) {
            return;
        }
        self.model.break_coalescing();
        let changed = edit(&mut self.model, menu.block);
        self.model.break_coalescing();
        // Any edit closes the menu, so `menu.block` is still the block it
        // opened on and the edit can't fail. If it ever did, the document is
        // unchanged and there is nothing to report.
        if changed {
            self.after_edit(EditOrigin::Command(origin), cx);
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

/// A row of a menu that ticks its current choice.
pub(crate) struct CheckedOption {
    pub(crate) slot: MenuSlot,
    pub(crate) ix: usize,
    /// The row's accessible name.
    pub(crate) name: SharedString,
    pub(crate) label: SharedString,
    /// An icon before the label.
    pub(crate) leading: Option<AnyElement>,
    /// Highlighted.
    pub(crate) on: bool,
    pub(crate) ticked: bool,
}

/// `option` as a row: the optional leading icon, the label, then a fixed
/// slot for the tick so names stay aligned whether or not they are ticked.
pub(crate) fn checked_option(
    state: &Entity<BlockEditorState>,
    option: CheckedOption,
    cx: &App,
) -> AnyElement {
    let CheckedOption {
        slot,
        ix,
        name,
        label,
        leading,
        on,
        ticked,
    } = option;
    let fg = option_color(on, cx);
    option_row(state, slot, ix, name, on, cx)
        .children(leading)
        .child(div().flex_1().child(label))
        .child(div().size_4().when(ticked, |this| {
            this.child(Icon::default().data(ICON_CHECK).small().text_color(fg))
        }))
        .into_any_element()
}

/// A fade over the bottom of the rows, as wide as the popover. It lets
/// clicks through to the rows beneath.
fn more_below_fade(background: Hsla) -> impl IntoElement {
    div()
        .id("insert-menu-more-below")
        .test_support()
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .h_6()
        .bg(linear_gradient(
            180.,
            linear_color_stop(background.opacity(0.), 0.),
            linear_color_stop(background, 1.),
        ))
}

/// What a popover shows: `header` stays put above the scrolling `rows`, and
/// `highlighted` is the index in `rows` of the highlighted one, which a key
/// move scrolls into view.
pub(crate) struct Content {
    pub(crate) header: Option<AnyElement>,
    pub(crate) rows: Vec<AnyElement>,
    pub(crate) highlighted: Option<usize>,
}

/// `slot`'s menu as a deferred popover hanging from `at` (window
/// coordinates), kept inside the window. A click outside closes it.
///
/// The popover is capped at `MAX_HEIGHT_REMS` or what the window allows, and
/// the rows scroll inside it.
pub(crate) fn popover(
    state: &Entity<BlockEditorState>,
    slot: MenuSlot,
    at: Point<Pixels>,
    content: Content,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let (id, label) = slot.popover();
    let scroll = state.read(cx).open_menu(slot).map(|menu| {
        if menu.reveal.take()
            && let Some(row) = content.highlighted
        {
            menu.scroll.scroll_to_item(row);
        }
        let overflows = menu.scroll.max_offset().y > px(0.);
        // The handle only measures overflow after layout, so redraw once when
        // the frame just drawn guessed wrong. `request_animation_frame` would
        // notify the view being rendered, which doesn't redraw this editor.
        if menu.overflow_drawn.replace(overflows) != overflows {
            let state = state.clone();
            window.on_next_frame(move |_, cx| state.update(cx, |_, cx| cx.notify()));
        }
        menu.scroll.clone()
    });
    let max_height = (window.viewport_size().height - WINDOW_MARGIN * 2.)
        .min(window.rem_size() * MAX_HEIGHT_REMS);
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
                    .max_h(max_height)
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
                    .children(content.header)
                    .child(
                        // The scrollbar overlays this wrapper rather than the
                        // scrolling div, or it would scroll away with the rows.
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .min_h_0()
                            .child(
                                div()
                                    .id((id, 1usize))
                                    .flex()
                                    .flex_col()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .when_some(scroll.clone(), |this, scroll| {
                                        this.track_scroll(&scroll)
                                    })
                                    .children(content.rows),
                            )
                            .when_some(scroll, |this, scroll| {
                                // macOS hides scrollbars at rest, so a fade
                                // also says there are more rows below.
                                let more_below = scroll.max_offset().y > px(0.)
                                    && scroll.offset().y > -scroll.max_offset().y;
                                this.vertical_scrollbar(&scroll).when(more_below, |this| {
                                    this.child(more_below_fade(theme.popover))
                                })
                            }),
                    ),
            ),
    )
    .with_priority(1)
    .into_any_element()
}
