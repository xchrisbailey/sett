//! FORK: the Callout type menu. Clicking a callout's header, or the shortcut
//! with the caret in one, opens a list of the five types and "Plain quote"
//! under the header. Picking one retypes the callout as one undo step. The
//! editor keeps focus and the caret never moves, on pick or on close.

use gpui::{Bounds, Context, Pixels, Point, point, px};

use super::{
    id::BlockId, kind::BlockKindData, kind::CalloutKind, state::BlockEditorState,
    transaction::EditOrigin,
};

/// The menu's entries in order: the five types, then a plain quote.
const ENTRIES: [Option<CalloutKind>; 6] = [
    Some(CalloutKind::Note),
    Some(CalloutKind::Tip),
    Some(CalloutKind::Important),
    Some(CalloutKind::Warning),
    Some(CalloutKind::Caution),
    None,
];

/// The open type menu: the callout it retypes and the highlighted entry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CalloutMenu {
    pub(crate) block: BlockId,
    pub(crate) highlighted: usize,
}

impl BlockEditorState {
    /// Whether the Callout type menu is open.
    pub fn callout_menu_open(&self) -> bool {
        self.callout_menu.is_some()
    }

    /// The type of the callout `block`, if it is one.
    pub(crate) fn callout_kind(&self, block: BlockId) -> Option<CalloutKind> {
        self.model
            .doc
            .kind(block)
            .and_then(BlockKindData::callout_kind)
    }

    /// Open the menu for the callout `block`, highlighting its type.
    pub(crate) fn open_callout_menu(&mut self, block: BlockId, cx: &mut Context<Self>) {
        let Some(kind) = self.callout_kind(block) else {
            return;
        };
        if self.readonly {
            return;
        }
        self.end_composition();
        self.insert_menu = None;
        self.close_table_menu(cx);
        self.callout_menu = Some(CalloutMenu {
            block,
            highlighted: ENTRIES.iter().position(|e| *e == Some(kind)).unwrap_or(0),
        });
        cx.notify();
    }

    /// A click on a callout's header opens its menu. Returns whether the
    /// click was on one.
    pub(crate) fn click_callout_header(
        &mut self,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(block) = self.callout_header_at(at) else {
            return false;
        };
        self.open_callout_menu(block, cx);
        true
    }

    /// The shortcut: open the menu for the callout holding the caret.
    /// Elsewhere the key passes on. A header that wasn't drawn in view last
    /// frame (scrolled away, or not laid out yet) opens nothing, since the
    /// menu would hang from somewhere the user can't see.
    pub(crate) fn on_open_callout_menu(
        &mut self,
        _: &super::state::OpenCalloutMenu,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        match self.model.caret_callout() {
            Some(block) if !self.readonly => {
                if self.callout_header_in_view(block) {
                    self.open_callout_menu(block, cx);
                }
            }
            _ => cx.propagate(),
        }
    }

    /// The callout `block`'s header bounds in the last painted frame.
    fn callout_header_bounds(&self, block: BlockId) -> Option<Bounds<Pixels>> {
        let frame = self.frame.borrow();
        let (_, bounds) = frame.callout_headers.iter().find(|(id, _)| *id == block)?;
        Some(*bounds)
    }

    /// Whether `block`'s header was drawn last frame inside the scrolled
    /// viewport (anywhere, with no scroll container).
    fn callout_header_in_view(&self, block: BlockId) -> bool {
        self.callout_header_bounds(block).is_some_and(|header| {
            self.scroll_handle
                .as_ref()
                .is_none_or(|scroll| scroll.bounds().intersects(&header))
        })
    }

    pub(crate) fn close_callout_menu(&mut self, cx: &mut Context<Self>) {
        if self.callout_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Move the highlight down (`forward`) or up, wrapping at the ends.
    pub(crate) fn callout_menu_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(menu) = &mut self.callout_menu else {
            return;
        };
        let count = ENTRIES.len();
        menu.highlighted = if forward {
            (menu.highlighted + 1) % count
        } else {
            (menu.highlighted + count - 1) % count
        };
        cx.notify();
    }

    /// Pick entry `ix`: retype the callout (or make it a plain quote) as one
    /// undo step, leaving the caret where it is.
    pub(crate) fn callout_menu_pick(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(&kind) = ENTRIES.get(ix) else {
            return;
        };
        let Some(menu) = self.callout_menu.take() else {
            return;
        };
        cx.notify();
        if self.callout_kind(menu.block) == kind {
            return;
        }
        let revision = self.model.revision();
        self.model.break_coalescing();
        let _ = self.model.retype_quote(menu.block, kind);
        self.model.break_coalescing();
        if self.model.revision() != revision {
            self.after_edit(EditOrigin::Command("set_callout"), cx);
        }
    }

    /// Where the open menu hangs from: under its callout's header, in
    /// window coordinates, from the last painted frame.
    pub(crate) fn callout_menu_anchor(&self) -> Option<Point<Pixels>> {
        let bounds = self.callout_header_bounds(self.callout_menu.as_ref()?.block)?;
        Some(point(bounds.left(), bounds.bottom() + px(4.)))
    }
}

/// The open type menu as a deferred, window-anchored popover.
pub(crate) fn render(
    state: &gpui::Entity<BlockEditorState>,
    cx: &gpui::App,
) -> Option<gpui::AnyElement> {
    use gpui::prelude::FluentBuilder as _;
    use gpui::{
        InteractiveElement as _, IntoElement as _, ParentElement as _, Role,
        StatefulInteractiveElement as _, Styled as _, anchored, deferred, div,
    };
    use gpui_kit::base::TestSupportExt as _;
    use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};

    const ICON_CHECK: &[u8] = include_bytes!("../assets/icons/check.svg");
    const ICON_QUOTE: &[u8] = include_bytes!("../assets/icons/text-quote.svg");

    let s = state.read(cx);
    let menu = s.callout_menu.as_ref()?;
    let at = s.callout_menu_anchor()?;
    let current = s.callout_kind(menu.block);
    let highlighted = menu.highlighted;
    let theme = cx.theme();
    let item = |ix: usize, kind: Option<CalloutKind>| {
        let on = ix == highlighted;
        let (label, icon) = match kind {
            Some(kind) => (
                kind.label(),
                Icon::default()
                    .data(super::callout::icon(kind))
                    .small()
                    .text_color(super::callout::color(kind, cx)),
            ),
            None => (
                "Plain quote",
                Icon::default()
                    .data(ICON_QUOTE)
                    .small()
                    .text_color(theme.muted_foreground),
            ),
        };
        let fg = if on {
            theme.accent_foreground
        } else {
            theme.popover_foreground
        };
        let entity = state.clone();
        let hover_entity = state.clone();
        div()
            .id(("callout-menu-item", ix))
            .role(Role::ListBoxOption)
            .test_support()
            .aria_label(label)
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(theme.radius)
            .text_color(fg)
            .when(on, |this| this.bg(theme.accent))
            .child(icon)
            .child(div().flex_1().child(label))
            // A fixed slot keeps the names aligned whether or not they are
            // checked.
            .child(
                div()
                    .size_4()
                    .when(kind.is_some() && kind == current, |this| {
                        this.child(Icon::default().data(ICON_CHECK).small().text_color(fg))
                    }),
            )
            .on_hover(move |hovered, _, cx| {
                if *hovered {
                    hover_entity.update(cx, |state, cx| {
                        if let Some(menu) = &mut state.callout_menu
                            && menu.highlighted != ix
                        {
                            menu.highlighted = ix;
                            cx.notify();
                        }
                    });
                }
            })
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                entity.update(cx, |state, cx| state.callout_menu_pick(ix, cx));
            })
            .into_any_element()
    };
    let (types, plain) = ENTRIES.split_at(ENTRIES.len() - 1);
    let types = types
        .iter()
        .enumerate()
        .map(|(ix, kind)| item(ix, *kind))
        .collect::<Vec<_>>();
    let plain = item(types.len(), plain[0]);
    let entity = state.clone();
    Some(
        deferred(
            anchored()
                .position(at)
                .position_mode(gpui::AnchoredPositionMode::Window)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .id("block-editor-callout-menu")
                        .role(Role::ListBox)
                        .test_support()
                        .aria_label("Callout type")
                        .occlude()
                        .min_w(px(200.))
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
                            entity.update(cx, |state, cx| state.close_callout_menu(cx));
                        })
                        .children(types)
                        .child(div().my_1().h_px().bg(theme.border))
                        .child(plain),
                ),
        )
        .with_priority(1)
        .into_any_element(),
    )
}
