//! FORK: the Insert Menu. Typing `/` as the only character of a paragraph
//! opens a list of block types under the caret. The text typed after the `/`
//! filters the list; picking an entry removes the typed text and turns the
//! paragraph into that block, as one undo step. The text stays in the block
//! until something is picked, so closing the menu loses nothing.

use gpui::{Context, Pixels, Point, SharedString, point, px};

use super::{
    command::EditorCommand,
    editor::BlockEditorModel,
    id::BlockId,
    kind::{BlockKind, BlockKindData, CalloutKind},
    position::BlockPosition,
    selection::BlockSelection,
    state::BlockEditorState,
    transaction::EditOrigin,
};

/// One entry of the Insert Menu: a label, the command it runs on the
/// paragraph, and optional extra search words and a Markdown hint.
#[derive(Clone, Debug, PartialEq)]
pub struct InsertMenuItem {
    label: SharedString,
    aliases: Vec<SharedString>,
    hint: Option<SharedString>,
    command: EditorCommand,
}

impl InsertMenuItem {
    pub fn new(label: impl Into<SharedString>, command: EditorCommand) -> Self {
        Self {
            label: label.into(),
            aliases: Vec::new(),
            hint: None,
            command,
        }
    }

    /// Extra words the filter matches, besides the label.
    pub fn aliases(mut self, aliases: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.aliases = aliases.into_iter().map(Into::into).collect();
        self
    }

    /// Text shown muted beside the label, such as the Markdown that makes
    /// the same block.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn hint_text(&self) -> Option<&SharedString> {
        self.hint.as_ref()
    }

    pub fn command(&self) -> &EditorCommand {
        &self.command
    }

    /// Case-insensitive substring match on the label or an alias. An empty
    /// query matches everything.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        std::iter::once(&self.label)
            .chain(&self.aliases)
            .any(|word| word.to_lowercase().contains(&query))
    }

    /// The built-in entries: every block the editor's input rules can make.
    pub fn defaults() -> Vec<Self> {
        let heading = |level: u8| {
            Self::new(
                format!("Heading {level}"),
                EditorCommand::SetBlockKind(BlockKind::Heading { level }),
            )
            .aliases([format!("h{level}"), "title".into()])
            .hint(format!("{} ", "#".repeat(level as usize)))
        };
        let callout = |kind: CalloutKind| {
            Self::new(kind.label(), EditorCommand::InsertCallout(kind))
                .aliases(["callout"])
                .hint(format!("> [!{}]", kind.marker()))
        };
        let mut items = vec![
            heading(1),
            heading(2),
            heading(3),
            Self::new("Bullet list", EditorCommand::ToggleList { ordered: false })
                .aliases(["unordered", "ul"])
                .hint("- "),
            Self::new("Numbered list", EditorCommand::ToggleList { ordered: true })
                .aliases(["ordered", "ol"])
                .hint("1. "),
            Self::new("Checklist", EditorCommand::ToggleTask)
                .aliases(["todo", "task", "checkbox"])
                .hint("[ ] "),
            Self::new("Quote", EditorCommand::ToggleBlockquote)
                .aliases(["blockquote"])
                .hint("> "),
        ];
        items.extend(CalloutKind::ALL.map(callout));
        items.extend([
            Self::new("Code block", EditorCommand::SetCodeBlock { lang: None })
                .aliases(["fence", "snippet"])
                .hint("```"),
            Self::new("Table", EditorCommand::InsertTable { rows: 2, cols: 2 }).aliases(["grid"]),
            Self::new(
                "Divider",
                EditorCommand::InsertBlock(BlockKind::HorizontalRule),
            )
            .aliases(["rule", "separator", "hr", "line"])
            .hint("---"),
        ]);
        items
    }
}

/// The open Insert Menu: the paragraph it belongs to and the highlighted
/// entry, an index into the filtered list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertMenu {
    pub(crate) block: BlockId,
    pub(crate) highlighted: usize,
}

impl BlockEditorModel {
    /// The text typed after `block`'s `/`, up to the caret, or to
    /// `composing_from` (where an IME composition starts) so uncommitted text
    /// never filters the menu.
    pub(crate) fn insert_menu_query(
        &self,
        block: BlockId,
        composing_from: Option<usize>,
    ) -> Option<&str> {
        let caret = self.caret()?;
        let end = composing_from.map_or(caret.offset, |from| from.min(caret.offset));
        self.doc.leaf_text(block)?.get(1..end)
    }

    /// Clear `block`'s text (the typed `/query`) and run `command` on it, as
    /// one undo step. A divider replaces the paragraph: the rule goes in its
    /// place and the empty paragraph, with the caret, moves below it.
    pub(crate) fn apply_insert_item(&mut self, block: BlockId, command: EditorCommand) {
        let len = self.doc.leaf_len(block).unwrap_or(0);
        let divider = command == EditorCommand::InsertBlock(BlockKind::HorizontalRule);
        self.break_coalescing();
        self.history.begin_transaction();
        let cleared = self
            .set_selection(BlockSelection::Text {
                anchor: BlockPosition::new(block, 0),
                head: BlockPosition::new(block, len),
            })
            .is_ok()
            && (len == 0 || self.delete(false).is_ok());
        if cleared && self.run(command).is_ok() && divider {
            let _ = self.run(EditorCommand::MoveBlocks { down: true });
        }
        self.history.commit_transaction();
        self.break_coalescing();
    }
}

impl BlockEditorState {
    /// Whether the Insert Menu is open.
    pub fn insert_menu_open(&self) -> bool {
        self.insert_menu.is_some()
    }

    /// The text typed after the `/`, while the menu is open.
    pub fn insert_menu_query(&self) -> Option<&str> {
        let menu = self.insert_menu.as_ref()?;
        let composing_from = self
            .marked_range()
            .filter(|(block, _)| *block == menu.block)
            .map(|(_, range)| range.start);
        self.model.insert_menu_query(menu.block, composing_from)
    }

    /// The entries the current query matches, in menu order. Callouts are
    /// left out below the top level, where `InsertCallout` is refused.
    pub fn insert_menu_matches(&self) -> Vec<&InsertMenuItem> {
        let Some(query) = self.insert_menu_query() else {
            return Vec::new();
        };
        let callouts = self.model.can_insert_callout();
        self.insert_menu_items
            .iter()
            .filter(|item| callouts || !matches!(item.command(), EditorCommand::InsertCallout(_)))
            .filter(|item| item.matches(query))
            .collect()
    }

    /// The highlighted entry's index in [`Self::insert_menu_matches`].
    pub fn insert_menu_highlighted(&self) -> Option<usize> {
        self.insert_menu.as_ref().map(|m| m.highlighted)
    }

    /// Open the menu if the caret's paragraph holds exactly the `/` just
    /// typed. Called after typing, never during IME composition.
    pub(crate) fn maybe_open_insert_menu(&mut self, cx: &mut Context<Self>) {
        if self.readonly || self.is_composing() || self.insert_menu_items.is_empty() {
            return;
        }
        let Some(caret) = self.model.caret() else {
            return;
        };
        let doc = &self.model.doc;
        if doc.kind(caret.block) != Some(&BlockKindData::Paragraph)
            || doc.leaf_text(caret.block) != Some("/")
            || caret.offset != 1
        {
            return;
        }
        self.insert_menu = Some(InsertMenu {
            block: caret.block,
            highlighted: 0,
        });
        cx.notify();
    }

    /// Close the menu unless it still applies: the caret sits after the `/`
    /// in the menu's paragraph and the query matches something. Nothing
    /// changes during IME composition; `end_composition` syncs again.
    pub(crate) fn sync_insert_menu(&mut self) {
        let Some(menu) = &self.insert_menu else {
            return;
        };
        if self.composition_in_progress() {
            return;
        }
        let doc = &self.model.doc;
        let still = !self.readonly
            && doc.kind(menu.block) == Some(&BlockKindData::Paragraph)
            && doc
                .leaf_text(menu.block)
                .is_some_and(|t| t.starts_with('/'))
            && self
                .model
                .caret()
                .is_some_and(|c| c.block == menu.block && c.offset >= 1);
        let count = if still {
            self.insert_menu_matches().len()
        } else {
            0
        };
        match &mut self.insert_menu {
            Some(menu) if count > 0 => menu.highlighted = menu.highlighted.min(count - 1),
            _ => self.insert_menu = None,
        }
    }

    pub(crate) fn close_insert_menu(&mut self, cx: &mut Context<Self>) {
        if self.insert_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Move the highlight down (`forward`) or up, wrapping at the ends.
    pub fn insert_menu_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let count = self.insert_menu_matches().len();
        let Some(menu) = &mut self.insert_menu else {
            return;
        };
        if count == 0 {
            return;
        }
        menu.highlighted = if forward {
            (menu.highlighted + 1) % count
        } else {
            (menu.highlighted + count - 1) % count
        };
        cx.notify();
    }

    /// Pick entry `ix` of the filtered list: remove the typed `/query` and
    /// run the entry's command on the paragraph, as one undo step.
    pub fn insert_menu_pick(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(command) = self
            .insert_menu_matches()
            .get(ix)
            .map(|item| item.command().clone())
        else {
            return;
        };
        let Some(menu) = self.insert_menu.take() else {
            return;
        };
        self.model.apply_insert_item(menu.block, command);
        self.after_edit(EditOrigin::Command("insert_menu"), cx);
    }

    /// Where the open menu hangs from: under the start of its paragraph, in
    /// window coordinates, from the last painted frame.
    pub fn insert_menu_anchor(&self) -> Option<Point<Pixels>> {
        let menu = self.insert_menu.as_ref()?;
        let bounds = self
            .frame
            .borrow()
            .caret_bounds(BlockPosition::new(menu.block, 0), px(1.))?;
        Some(point(bounds.left(), bounds.bottom() + px(4.)))
    }
}

/// The open Insert Menu as a deferred, window-anchored popover.
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
    use gpui_kit::component::ActiveTheme as _;

    let s = state.read(cx);
    let at = s.insert_menu_anchor()?;
    let highlighted = s.insert_menu_highlighted()?;
    let theme = cx.theme();
    let items = s
        .insert_menu_matches()
        .into_iter()
        .enumerate()
        .map(|(ix, item)| {
            let on = ix == highlighted;
            let entity = state.clone();
            let hover_entity = state.clone();
            div()
                .id(("insert-menu-item", ix))
                .role(Role::ListBoxOption)
                .test_support()
                .aria_label(item.label().clone())
                .flex()
                .justify_between()
                .items_center()
                .gap_4()
                .px_2()
                .py_1()
                .rounded(theme.radius)
                .text_color(if on {
                    theme.accent_foreground
                } else {
                    theme.popover_foreground
                })
                .when(on, |this| this.bg(theme.accent))
                .child(item.label().clone())
                .when_some(item.hint_text().cloned(), |this, hint| {
                    this.child(
                        div()
                            .text_xs()
                            .font_family(theme.mono_font_family.clone())
                            .text_color(theme.muted_foreground)
                            .child(hint),
                    )
                })
                .on_hover(move |hovered, _, cx| {
                    if *hovered {
                        hover_entity.update(cx, |state, cx| {
                            if let Some(menu) = &mut state.insert_menu
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
                    entity.update(cx, |state, cx| state.insert_menu_pick(ix, cx));
                })
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let entity = state.clone();
    Some(
        deferred(
            anchored()
                .position(at)
                .position_mode(gpui::AnchoredPositionMode::Window)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .id("block-editor-insert-menu")
                        .role(Role::ListBox)
                        .test_support()
                        .aria_label("Insert Menu")
                        .occlude()
                        .min_w(px(240.))
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
                            entity.update(cx, |state, cx| state.close_insert_menu(cx));
                        })
                        .children(items),
                ),
        )
        .with_priority(1)
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::InsertMenuItem;

    fn labels(query: &str) -> Vec<String> {
        InsertMenuItem::defaults()
            .into_iter()
            .filter(|item| item.matches(query))
            .map(|item| item.label().to_string())
            .collect()
    }

    #[test]
    fn an_empty_query_lists_every_default() {
        assert_eq!(labels("").len(), 15);
    }

    #[test]
    fn the_filter_matches_labels_and_aliases_ignoring_case() {
        assert_eq!(labels("CHE"), ["Checklist"]);
        assert_eq!(labels("todo"), ["Checklist"]);
        assert_eq!(labels("h2"), ["Heading 2"]);
        assert_eq!(
            labels("list"),
            ["Bullet list", "Numbered list", "Checklist"]
        );
        assert!(labels("zzz").is_empty());
    }

    #[test]
    fn callout_lists_the_five_callout_types() {
        assert_eq!(
            labels("callout"),
            ["Note", "Tip", "Important", "Warning", "Caution"]
        );
        assert_eq!(labels("warn"), ["Warning"]);
    }

    /// An IME composition after the `/` is not part of the query until it is
    /// committed, so the list neither refilters nor closes while composing.
    #[test]
    fn an_ime_composition_is_left_out_of_the_query() {
        let mut model = crate::tests::model("/he");
        let pos = crate::tests::caret_in(&mut model, 0, 3);
        model
            .insert_text("日本", crate::transaction::EditOrigin::Ime)
            .unwrap();
        assert_eq!(model.insert_menu_query(pos.block, Some(3)), Some("he"));
        assert_eq!(model.insert_menu_query(pos.block, None), Some("he日本"));
    }

    /// Picking each default from `/query` in an empty paragraph saves the
    /// Markdown for its block, and one undo brings the `/query` back.
    #[test]
    fn every_default_makes_its_block_in_one_undo_step() {
        let expected = [
            ("Heading 1", "# "),
            ("Heading 2", "## "),
            ("Heading 3", "### "),
            ("Bullet list", "- "),
            ("Numbered list", "1. "),
            ("Checklist", "- [ ] "),
            // An empty quote saves as nothing, as it does after typing `> `.
            ("Quote", ""),
            ("Note", "> [!NOTE]"),
            ("Tip", "> [!TIP]"),
            ("Important", "> [!IMPORTANT]"),
            ("Warning", "> [!WARNING]"),
            ("Caution", "> [!CAUTION]"),
            ("Code block", "```\n\n```"),
            ("Table", "|  |  |\n| --- | --- |\n|  |  |"),
            ("Divider", "---\n\n"),
        ];
        let typed = "before\n\n/x\n\nafter";
        for (item, (label, middle)) in InsertMenuItem::defaults().into_iter().zip(expected) {
            assert_eq!(item.label(), label);
            let mut model = crate::tests::model(typed);
            let pos = crate::tests::caret_in(&mut model, 1, 2);
            model.apply_insert_item(pos.block, item.command().clone());
            crate::tests::assert_invariants(&model);
            assert_eq!(
                model.source(),
                format!("before\n\n{middle}\n\nafter"),
                "{label}"
            );
            if label == "Quote" {
                assert!(model.in_blockquote());
            }
            if label == "Divider" || middle.starts_with("> [!") {
                // The caret is on the empty line under the rule, or inside
                // the callout.
                let caret = model.caret().unwrap();
                assert_eq!(model.doc().leaf_text(caret.block), Some(""));
            }
            assert!(model.undo(), "{label}");
            assert_eq!(model.source(), typed, "{label}");
        }
    }
}
