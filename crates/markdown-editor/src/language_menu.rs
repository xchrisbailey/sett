//! FORK: the Language Menu. Clicking a Code Block's language label, or the
//! shortcut with the caret in one, opens a list under the label: Plain text,
//! then the curated languages. Typed text goes to the menu's own query, shown
//! at the top of the popover, never into the code. Picking a row sets the
//! block's language as one undo step. The editor keeps focus and the caret
//! never moves, on pick or on close.

use gpui::{Context, Pixels, Point};

use super::{
    id::BlockId,
    kind::BlockKindData,
    language::{CODE_BLOCK_LANGUAGES, LanguageEntry},
    menu::{self, MenuSlot, OpenMenu},
    state::BlockEditorState,
};

/// One row of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row<'a> {
    /// No language.
    Plain,
    /// A curated language.
    Language(&'static LanguageEntry),
    /// The query itself, as typed, for a language the list lacks.
    Use(&'a str),
}

impl Row<'_> {
    fn label(&self) -> String {
        match self {
            Row::Plain => "Plain text".to_string(),
            Row::Language(entry) => entry.display.to_string(),
            Row::Use(query) => format!("Use \"{query}\""),
        }
    }

    /// The language this row writes.
    fn lang(&self) -> Option<String> {
        match self {
            Row::Plain => None,
            Row::Language(entry) => Some(entry.fence.to_string()),
            Row::Use(query) => Some((*query).to_string()),
        }
    }

    /// Whether this row is the block's current language `lang`, matched
    /// through aliases and ignoring case.
    fn is_current(&self, lang: Option<&str>) -> bool {
        let lang = lang.filter(|l| !l.is_empty());
        match (self, lang) {
            (Row::Plain, None) => true,
            (Row::Language(entry), Some(lang)) => {
                entry.fence.eq_ignore_ascii_case(lang)
                    || entry.aliases.iter().any(|a| a.eq_ignore_ascii_case(lang))
            }
            _ => false,
        }
    }
}

/// The rows `query` leaves, in menu order. The query filters by display
/// name, fence name and alias, ignoring case. `Use` comes last, unless the
/// trimmed query is empty, has whitespace in it, or already names a row.
pub(crate) fn rows(query: &str) -> Vec<Row<'_>> {
    let query = query.trim();
    let needle = query.to_lowercase();
    let mut rows = Vec::new();
    if "plain text".contains(&needle) {
        rows.push(Row::Plain);
    }
    rows.extend(
        CODE_BLOCK_LANGUAGES
            .iter()
            .filter(|e| {
                e.display.to_lowercase().contains(&needle)
                    || e.fence.contains(&needle)
                    || e.aliases.iter().any(|a| a.contains(&needle))
            })
            .map(Row::Language),
    );
    let named = CODE_BLOCK_LANGUAGES
        .iter()
        .any(|e| e.display.eq_ignore_ascii_case(query) || e.fence.eq_ignore_ascii_case(query));
    if !query.is_empty() && !query.contains(char::is_whitespace) && !named {
        rows.push(Row::Use(query));
    }
    rows
}

impl BlockEditorState {
    /// Whether the Language Menu is open.
    pub fn language_menu_open(&self) -> bool {
        self.language_menu.is_some()
    }

    /// The language of the Code Block `block`: `None` if it isn't one,
    /// `Some(None)` if it has no language.
    fn code_lang(&self, block: BlockId) -> Option<Option<String>> {
        match self.model.doc.kind(block)? {
            BlockKindData::CodeBlock { lang, .. } => Some(lang.as_ref().map(|l| l.to_string())),
            _ => None,
        }
    }

    /// Open the menu for the Code Block `block`, highlighting its language.
    pub(crate) fn open_language_menu(&mut self, block: BlockId, cx: &mut Context<Self>) {
        let Some(lang) = self.code_lang(block) else {
            return;
        };
        if !self.begin_menu_open(cx) {
            return;
        }
        let highlighted = rows("")
            .iter()
            .position(|row| row.is_current(lang.as_deref()))
            .unwrap_or(0);
        self.language_menu = Some(OpenMenu::new(block, highlighted));
        cx.notify();
    }

    /// A click on a Code Block's language label opens its menu. Returns
    /// whether the click was on one.
    pub(crate) fn click_code_label(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        self.click_menu_anchor(MenuSlot::Language, at, cx)
    }

    /// The shortcut: open the menu for the Code Block holding the caret.
    pub(crate) fn on_open_language_menu(
        &mut self,
        _: &super::state::OpenLanguageMenu,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let block = self
            .model
            .caret()
            .map(|pos| pos.block)
            .filter(|block| self.code_lang(*block).is_some());
        self.open_menu_by_shortcut(MenuSlot::Language, block, cx);
    }

    /// Move the highlight down (`forward`) or up, wrapping at the ends.
    pub(crate) fn language_menu_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(menu) = &self.language_menu else {
            return;
        };
        let count = rows(&menu.query).len();
        self.step_menu(MenuSlot::Language, forward, count, cx);
    }

    /// Typed text joins the query and puts the highlight on the first row.
    pub(crate) fn language_query_type(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(menu) = self.language_menu.as_mut() {
            menu.query.push_str(text);
        }
        self.reset_language_highlight(cx);
    }

    /// Backspace drops the query's last character. Returns whether the menu
    /// took the key.
    pub(crate) fn language_query_backspace(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(menu) = self.language_menu.as_mut() else {
            return false;
        };
        menu.query.pop();
        self.reset_language_highlight(cx);
        true
    }

    fn reset_language_highlight(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.language_menu.as_mut() {
            menu.highlighted = 0;
        }
        cx.notify();
    }

    /// Pick row `ix`: set the block's language as one undo step, leaving
    /// the caret where it is. A row that is gone (the query moved on) does
    /// nothing.
    pub(crate) fn language_menu_pick(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(menu) = self.language_menu.as_ref() else {
            return;
        };
        let rows = rows(&menu.query);
        let Some(row) = rows.get(ix) else {
            return;
        };
        let lang = row.lang();
        self.pick_anchored(
            MenuSlot::Language,
            |this, block| this.code_lang(block).is_none_or(|old| old == lang),
            "set_code_lang",
            |model, block| model.set_code_lang(block, lang.clone()).unwrap_or(false),
            cx,
        );
    }
}

/// The open menu as a deferred, window-anchored popover.
pub(crate) fn render(
    state: &gpui::Entity<BlockEditorState>,
    cx: &gpui::App,
) -> Option<gpui::AnyElement> {
    use gpui::prelude::FluentBuilder as _;
    use gpui::{
        InteractiveElement as _, IntoElement as _, ParentElement as _, Role,
        StatefulInteractiveElement as _, Styled as _, div, rems,
    };
    use gpui_kit::base::TestSupportExt as _;
    use gpui_kit::component::ActiveTheme as _;

    /// The list scrolls past this height.
    const MAX_HEIGHT_REMS: f32 = 18.;

    let s = state.read(cx);
    let menu = s.language_menu.as_ref()?;
    let at = s.anchor_below(MenuSlot::Language, menu.block)?;
    let current = s.code_lang(menu.block)?;
    let highlighted = menu.highlighted;
    let query = menu.query.as_str();
    let theme = cx.theme();
    let muted = theme.muted_foreground;

    // The query row: the text typed so far, or a hint. Focus stays in the
    // editor, so this is a display, not an input.
    let query_row = div()
        .id("language-menu-query")
        .role(Role::Label)
        .test_support()
        .aria_label("Language query")
        .px_2()
        .py_1()
        .mb_1()
        .border_b_1()
        .border_color(theme.border)
        .when(query.is_empty(), |this| {
            this.text_color(muted).child("Search languages")
        })
        .when(!query.is_empty(), |this| {
            this.text_color(theme.popover_foreground)
                .child(query.to_string())
        });

    let rows = rows(query);
    let list = div()
        .id("language-menu-list")
        .flex()
        .flex_col()
        .max_h(rems(MAX_HEIGHT_REMS))
        .overflow_y_scroll();
    let list = if rows.is_empty() {
        list.child(
            div()
                .id("language-menu-empty")
                .role(Role::Label)
                .test_support()
                .aria_label("No matching languages")
                .px_2()
                .py_1()
                .text_color(muted)
                .child("No matching languages"),
        )
    } else {
        list.children(rows.iter().enumerate().map(|(ix, row)| {
            let on = ix == highlighted;
            let ticked = row.is_current(current.as_deref());
            let label = row.label();
            let name = if ticked {
                format!("{label}, current")
            } else {
                label.clone()
            };
            menu::checked_option(
                state,
                menu::CheckedOption {
                    slot: MenuSlot::Language,
                    ix,
                    name: name.into(),
                    label: label.into(),
                    leading: None,
                    on,
                    ticked,
                },
                cx,
            )
        }))
    };
    Some(menu::popover(
        state,
        MenuSlot::Language,
        at,
        [query_row.into_any_element(), list.into_any_element()],
        cx,
    ))
}

#[cfg(test)]
mod tests {
    use super::{Row, rows};

    fn labels(query: &str) -> Vec<String> {
        rows(query).iter().map(Row::label).collect()
    }

    #[test]
    fn an_empty_query_lists_plain_text_then_every_language() {
        let all = labels("");
        assert_eq!(all[0], "Plain text");
        assert_eq!(all[1], "Bash");
        assert_eq!(all.len(), 1 + super::CODE_BLOCK_LANGUAGES.len());
    }

    #[test]
    fn the_query_matches_display_fence_and_alias_ignoring_case() {
        assert_eq!(labels("TY"), ["TypeScript", "Use \"TY\""]);
        assert_eq!(labels("rs"), ["Rust", "Use \"rs\""]);
        assert_eq!(labels("c++"), ["C++"]);
    }

    #[test]
    fn use_is_hidden_for_whitespace_or_an_exact_name() {
        assert_eq!(labels("a b"), Vec::<String>::new());
        assert_eq!(labels("  RUST "), ["Rust"]);
        assert_eq!(labels("javascript"), ["JavaScript"]);
        assert_eq!(labels("haskell"), ["Use \"haskell\""]);
    }

    #[test]
    fn use_carries_the_trimmed_query_as_typed() {
        assert_eq!(rows(" Haskell ").last(), Some(&Row::Use("Haskell")));
    }

    #[test]
    fn the_current_language_matches_through_aliases() {
        let rust = rows("rust")[0];
        assert!(rust.is_current(Some("rs")));
        assert!(rust.is_current(Some("RUST")));
        assert!(!rust.is_current(None));
        assert!(Row::Plain.is_current(None));
        assert!(!Row::Plain.is_current(Some("rust")));
    }
}
