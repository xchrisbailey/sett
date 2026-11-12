//! FORK: the Callout type menu. Clicking a callout's header, or the shortcut
//! with the caret in one, opens a list of the five types and "Plain quote"
//! under the header. Picking one retypes the callout as one undo step. The
//! editor keeps focus and the caret never moves, on pick or on close.

use gpui::{Bounds, Context, Pixels, Point};

use super::{
    id::BlockId,
    kind::BlockKindData,
    kind::CalloutKind,
    menu::{self, MenuSlot, OpenMenu},
    state::BlockEditorState,
    transaction::EditOrigin,
};

/// The menu's options in order: the five types, then a plain quote.
const CALLOUT_TYPES: [Option<CalloutKind>; 6] = [
    Some(CalloutKind::Note),
    Some(CalloutKind::Tip),
    Some(CalloutKind::Important),
    Some(CalloutKind::Warning),
    Some(CalloutKind::Caution),
    None,
];

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
        let highlighted = CALLOUT_TYPES
            .iter()
            .position(|option| *option == Some(kind))
            .unwrap_or(0);
        self.callout_menu = Some(OpenMenu::new(block, highlighted));
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

    /// Move the highlight down (`forward`) or up, wrapping at the ends.
    pub(crate) fn callout_menu_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.step_menu(MenuSlot::Callout, forward, CALLOUT_TYPES.len(), cx);
    }

    /// Pick option `ix`: retype the callout (or make it a plain quote) as
    /// one undo step, leaving the caret where it is.
    pub(crate) fn callout_menu_pick(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(&kind) = CALLOUT_TYPES.get(ix) else {
            return;
        };
        let Some(menu) = self.callout_menu.take() else {
            return;
        };
        cx.notify();
        if self.callout_kind(menu.block) == kind {
            return;
        }
        self.model.break_coalescing();
        let retyped = self.model.retype_quote(menu.block, kind);
        self.model.break_coalescing();
        // Any edit closes the menu, so `menu.block` is still the quote it
        // opened on and this can't fail. If it ever did, the document is
        // unchanged and there is nothing to report.
        if retyped.unwrap_or(false) {
            self.after_edit(EditOrigin::Command("set_callout"), cx);
        }
    }

    /// Where the open menu hangs from: under its callout's header, in
    /// window coordinates, from the last painted frame.
    pub(crate) fn callout_menu_anchor(&self) -> Option<Point<Pixels>> {
        let bounds = self.callout_header_bounds(self.callout_menu.as_ref()?.block)?;
        Some(menu::below(bounds))
    }
}

/// The open type menu as a deferred, window-anchored popover.
pub(crate) fn render(
    state: &gpui::Entity<BlockEditorState>,
    cx: &gpui::App,
) -> Option<gpui::AnyElement> {
    use gpui::prelude::FluentBuilder as _;
    use gpui::{IntoElement as _, ParentElement as _, Styled as _, div};
    use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};

    const ICON_CHECK: &[u8] = include_bytes!("../assets/icons/check.svg");
    const ICON_QUOTE: &[u8] = include_bytes!("../assets/icons/text-quote.svg");

    let s = state.read(cx);
    let menu = s.callout_menu.as_ref()?;
    let at = s.callout_menu_anchor()?;
    let current = s.callout_kind(menu.block);
    let highlighted = menu.highlighted;
    let theme = cx.theme();
    let option = |ix: usize, kind: Option<CalloutKind>| {
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
        let fg = menu::option_color(on, cx);
        menu::option_row(state, MenuSlot::Callout, ix, label, on, cx)
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
            .into_any_element()
    };
    let (types, plain) = CALLOUT_TYPES.split_at(CALLOUT_TYPES.len() - 1);
    let mut options: Vec<_> = types
        .iter()
        .enumerate()
        .map(|(ix, kind)| option(ix, *kind))
        .collect();
    options.push(div().my_1().h_px().bg(theme.border).into_any_element());
    options.push(option(types.len(), plain[0]));
    Some(menu::popover(state, MenuSlot::Callout, at, options, cx))
}
