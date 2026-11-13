//! FORK: the Callout type menu. Clicking a callout's header, or the shortcut
//! with the caret in one, opens a list of the five types and "Plain quote"
//! under the header. Picking one retypes the callout as one undo step. The
//! editor keeps focus and the caret never moves, on pick or on close.

use gpui::{Context, Pixels, Point};

use super::{
    id::BlockId,
    kind::BlockKindData,
    kind::CalloutKind,
    menu::{self, MenuSlot, OpenMenu},
    state::BlockEditorState,
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
        if !self.begin_menu_open(cx) {
            return;
        }
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
        self.click_menu_anchor(MenuSlot::Callout, at, cx)
    }

    /// The shortcut: open the menu for the callout holding the caret.
    pub(crate) fn on_open_callout_menu(
        &mut self,
        _: &super::state::OpenCalloutMenu,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let block = self.model.caret_callout();
        self.open_menu_by_shortcut(MenuSlot::Callout, block, cx);
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
        self.pick_anchored(
            MenuSlot::Callout,
            |this, block| this.callout_kind(block) == kind,
            "set_callout",
            |model, block| model.retype_quote(block, kind).unwrap_or(false),
            cx,
        );
    }
}

/// The open type menu as a deferred, window-anchored popover.
pub(crate) fn render(
    state: &gpui::Entity<BlockEditorState>,
    window: &gpui::Window,
    cx: &gpui::App,
) -> Option<gpui::AnyElement> {
    use gpui::{IntoElement as _, Styled as _, div};
    use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};

    const ICON_QUOTE: &[u8] = include_bytes!("../assets/icons/text-quote.svg");

    let s = state.read(cx);
    let menu = s.callout_menu.as_ref()?;
    let at = s.anchor_below(MenuSlot::Callout, menu.block)?;
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
        menu::checked_option(
            state,
            menu::CheckedOption {
                slot: MenuSlot::Callout,
                ix,
                name: label.into(),
                label: label.into(),
                leading: Some(icon.into_any_element()),
                on,
                ticked: kind.is_some() && kind == current,
            },
            cx,
        )
    };
    let (types, plain) = CALLOUT_TYPES.split_at(CALLOUT_TYPES.len() - 1);
    let mut options: Vec<_> = types
        .iter()
        .enumerate()
        .map(|(ix, kind)| option(ix, *kind))
        .collect();
    options.push(div().my_1().h_px().bg(theme.border).into_any_element());
    options.push(option(types.len(), plain[0]));
    // The divider is a row of its own, so the last option sits one row down.
    let highlighted_row = highlighted + usize::from(highlighted >= types.len());
    Some(menu::popover(
        state,
        MenuSlot::Callout,
        at,
        menu::Content {
            header: None,
            rows: options,
            highlighted: Some(highlighted_row),
        },
        window,
        cx,
    ))
}
