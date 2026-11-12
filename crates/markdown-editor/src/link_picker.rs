//! FORK: the Link Picker. Typing `[[` anywhere in text, outside inline code
//! and code blocks, opens a list of the host's Items under the caret. The
//! text typed after the `[[` filters it; picking an Item replaces the typed
//! `[[query` with `[[slug]]` and puts the caret after it, as one undo step.
//! The text stays in the block until something is picked, so closing the
//! picker loses nothing.
//!
//! The host fills the list with [`LinkCandidate`]s through
//! `BlockEditorState::set_link_candidates`. The editor knows nothing about
//! Items beyond what a candidate says.

use std::cmp::Reverse;
use std::sync::Arc;
use std::time::SystemTime;

use gpui::{Context, Pixels, Point, SharedString, px};

use super::{
    editor::BlockEditorModel,
    id::BlockId,
    menu::{self, MenuSlot, OpenMenu},
    position::BlockPosition,
    selection::BlockSelection,
    state::BlockEditorState,
    transaction::EditOrigin,
};

/// What the Link Picker is open on: the offset of the first `[` of the typed
/// `[[`, the last offset the query ran to, and the query last matched.
#[derive(Clone, Debug, Default)]
pub(crate) struct LinkQuery {
    pub(crate) trigger: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
}

/// One Item the Link Picker can insert a Link to.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkCandidate {
    /// What the Link names: the text written between the brackets.
    pub slug: SharedString,
    pub title: SharedString,
    /// Muted text after the title, such as "Task · Kitchen reno".
    pub detail: SharedString,
    /// When the Item last changed; the most recent comes first.
    pub modified: Option<SystemTime>,
    /// Whether a Link to `slug` leads to this Item. When another Item shares
    /// the slug it doesn't, and the row can't be picked.
    pub reachable: bool,
}

impl LinkCandidate {
    pub fn new(
        slug: impl Into<SharedString>,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
    ) -> Self {
        Self {
            slug: slug.into(),
            title: title.into(),
            detail: detail.into(),
            modified: None,
            reachable: true,
        }
    }

    pub fn modified(mut self, modified: Option<SystemTime>) -> Self {
        self.modified = modified;
        self
    }

    pub fn reachable(mut self, reachable: bool) -> Self {
        self.reachable = reachable;
        self
    }
}

/// The candidates `query` matches in title or slug, ignoring case: title
/// prefixes first, then the rest, and within each the most recent first.
pub(crate) fn matching<'a>(candidates: &'a [LinkCandidate], query: &str) -> Vec<&'a LinkCandidate> {
    let query = query.to_lowercase();
    let mut out: Vec<(bool, &LinkCandidate)> = candidates
        .iter()
        .filter_map(|c| {
            let title = c.title.to_lowercase();
            if title.starts_with(&query) {
                Some((true, c))
            } else if title.contains(&query) || c.slug.to_lowercase().contains(&query) {
                Some((false, c))
            } else {
                None
            }
        })
        .collect();
    out.sort_by_key(|(prefix, c)| (!*prefix, Reverse(c.modified)));
    out.into_iter().map(|(_, c)| c).collect()
}

impl BlockEditorModel {
    /// The text typed after the `[[` at `trigger` in `block`, up to the
    /// caret, or to `composing_from` (where an IME composition starts) so
    /// uncommitted text never filters the picker.
    pub(crate) fn link_query(
        &self,
        block: BlockId,
        trigger: usize,
        composing_from: Option<usize>,
    ) -> Option<&str> {
        let caret = self.caret()?;
        let end = composing_from.map_or(caret.offset, |from| from.min(caret.offset));
        self.doc.leaf_text(block)?.get(trigger + 2..end)
    }

    /// Replace the typed `[[query` (from `trigger` to the caret) with
    /// `[[slug]]`, as one undo step, leaving the caret after it.
    pub(crate) fn apply_link_pick(&mut self, block: BlockId, trigger: usize, slug: &str) {
        let Some(caret) = self.caret().filter(|c| c.block == block) else {
            return;
        };
        self.break_coalescing();
        self.history.begin_transaction();
        let selected = self
            .set_selection(BlockSelection::Text {
                anchor: BlockPosition::new(block, trigger),
                head: caret,
            })
            .is_ok();
        if selected {
            let _ = self.insert_text(&format!("[[{slug}]]"), EditOrigin::Command("link_picker"));
        }
        self.history.commit_transaction();
        self.break_coalescing();
    }
}

impl BlockEditorState {
    /// Fill the Link Picker's list. The host calls this whenever its Items
    /// change.
    pub fn set_link_candidates(&mut self, candidates: Arc<[LinkCandidate]>) {
        self.link_candidates = candidates;
    }

    /// Whether the Link Picker is open.
    pub fn link_picker_open(&self) -> bool {
        self.link_picker.is_some()
    }

    /// The text typed after the `[[`, while the picker is open.
    pub fn link_picker_query(&self) -> Option<&str> {
        let menu = self.link_picker.as_ref()?;
        let composing_from = self
            .marked_range()
            .filter(|(block, _)| *block == menu.block)
            .map(|(_, range)| range.start);
        self.model
            .link_query(menu.block, self.link_query.trigger, composing_from)
    }

    /// The candidates the current query matches, in picker order.
    pub fn link_picker_matches(&self) -> Vec<&LinkCandidate> {
        let query = self.link_picker_query().unwrap_or_default();
        matching(&self.link_candidates, query)
    }

    /// The highlighted row's index in [`Self::link_picker_matches`].
    pub fn link_picker_highlighted(&self) -> Option<usize> {
        self.link_picker.as_ref().map(|m| m.highlighted)
    }

    /// Whether row `ix` can be highlighted and picked.
    pub(crate) fn link_row_pickable(&self, ix: usize) -> bool {
        self.link_picker_matches()
            .get(ix)
            .is_some_and(|c| c.reachable)
    }

    fn first_pickable_link_row(&self) -> usize {
        self.link_picker_matches()
            .iter()
            .position(|c| c.reachable)
            .unwrap_or(0)
    }

    /// Open the picker if the `[[` just typed ends at the caret in a text
    /// block, outside inline code. Called after typing, never during IME
    /// composition.
    pub(crate) fn maybe_open_link_picker(&mut self, cx: &mut Context<Self>) {
        if self.readonly || self.is_composing() {
            return;
        }
        let Some(caret) = self.model.caret() else {
            return;
        };
        let doc = &self.model.doc;
        let Some(trigger) = caret.offset.checked_sub(2) else {
            return;
        };
        let typed = doc
            .leaf_text(caret.block)
            .and_then(|t| t.get(trigger..caret.offset));
        let in_code = doc
            .inline(caret.block)
            .and_then(|inline| inline.mark_before(caret.offset))
            .is_some_and(|mark| mark.code);
        // A code block holds raw text (`plain`), and inline code is a mark.
        if typed != Some("[[") || in_code || doc.plain(caret.block).is_some() {
            return;
        }
        self.link_query = LinkQuery {
            trigger,
            end: caret.offset,
            text: String::new(),
        };
        self.link_picker = Some(OpenMenu::new(caret.block, 0));
        let first = self.first_pickable_link_row();
        if let Some(menu) = &mut self.link_picker {
            menu.highlighted = first;
        }
        cx.notify();
    }

    /// Refilter or close the picker. It stays open while the caret is in the
    /// `[[…` text of its block, and closes when the text is no longer there,
    /// the caret leaves it, or the query holds `]]`. `edited` is false for a
    /// plain caret move, which may not go past where the query ended.
    /// Nothing changes during IME composition; `end_composition` syncs again.
    pub(crate) fn sync_link_picker(&mut self, edited: bool) {
        let Some(menu) = &self.link_picker else {
            return;
        };
        if self.composition_in_progress() {
            return;
        }
        let trigger = self.link_query.trigger;
        let still = !self.readonly
            && self
                .model
                .doc
                .leaf_text(menu.block)
                .is_some_and(|t| t.get(trigger..trigger + 2) == Some("[["))
            && self.model.caret().is_some_and(|c| {
                c.block == menu.block
                    && c.offset >= trigger + 2
                    && (edited || c.offset <= self.link_query.end)
            });
        let query = still
            .then(|| self.link_picker_query().map(str::to_string))
            .flatten()
            .filter(|q| !q.contains("]]"));
        let Some(query) = query else {
            self.link_picker = None;
            return;
        };
        if edited && let Some(caret) = self.model.caret() {
            self.link_query.end = caret.offset;
        }
        if query != self.link_query.text {
            self.link_query.text = query;
            let first = self.first_pickable_link_row();
            if let Some(menu) = &mut self.link_picker {
                menu.highlighted = first;
            }
        }
    }

    /// Move the highlight down (`forward`) or up, wrapping at the ends and
    /// skipping rows that can't be picked.
    pub fn link_picker_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(from) = self.link_picker_highlighted() else {
            return;
        };
        let reachable: Vec<bool> = self
            .link_picker_matches()
            .iter()
            .map(|c| c.reachable)
            .collect();
        let count = reachable.len();
        let mut ix = from;
        for _ in 0..count {
            ix = if forward {
                (ix + 1) % count
            } else {
                (ix + count - 1) % count
            };
            if reachable[ix] {
                if let Some(menu) = &mut self.link_picker {
                    menu.highlighted = ix;
                }
                self.link_scroll.scroll_to_item(ix);
                cx.notify();
                return;
            }
        }
    }

    /// Pick row `ix` of the filtered list: replace the typed `[[query` with
    /// `[[slug]]`, as one undo step. A row that can't be picked does nothing.
    pub fn link_picker_pick(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.composition_in_progress() {
            return;
        }
        let Some(slug) = self
            .link_picker_matches()
            .get(ix)
            .filter(|c| c.reachable)
            .map(|c| c.slug.clone())
        else {
            return;
        };
        let Some(menu) = self.link_picker.take() else {
            return;
        };
        self.model
            .apply_link_pick(menu.block, self.link_query.trigger, &slug);
        self.after_edit(EditOrigin::Command("link_picker"), cx);
    }

    pub(crate) fn on_link_picker_up(
        &mut self,
        _: &super::state::LinkPickerUp,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.link_picker_move(false, cx);
    }

    pub(crate) fn on_link_picker_down(
        &mut self,
        _: &super::state::LinkPickerDown,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.link_picker_move(true, cx);
    }

    pub(crate) fn on_link_picker_pick(
        &mut self,
        _: &super::state::LinkPickerPick,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ix) = self.link_picker_highlighted() {
            self.link_picker_pick(ix, cx);
        }
    }

    pub(crate) fn on_link_picker_close(
        &mut self,
        _: &super::state::LinkPickerClose,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(MenuSlot::Link, cx);
    }

    /// Where the open picker hangs from: under the `[[`, in window
    /// coordinates, from the last painted frame.
    pub fn link_picker_anchor(&self) -> Option<Point<Pixels>> {
        let menu = self.link_picker.as_ref()?;
        let bounds = self.frame.borrow().caret_bounds(
            BlockPosition::new(menu.block, self.link_query.trigger),
            px(1.),
        )?;
        Some(menu::below(bounds))
    }
}

/// The open Link Picker as a deferred, window-anchored popover.
pub(crate) fn render(
    state: &gpui::Entity<BlockEditorState>,
    cx: &gpui::App,
) -> Option<gpui::AnyElement> {
    use gpui::prelude::FluentBuilder as _;
    use gpui::{
        InteractiveElement as _, IntoElement as _, ParentElement as _, Role,
        StatefulInteractiveElement as _, Styled as _, div,
    };
    use gpui_kit::base::TestSupportExt as _;
    use gpui_kit::component::ActiveTheme as _;

    let s = state.read(cx);
    let at = s.link_picker_anchor()?;
    let highlighted = s.link_picker_highlighted()?;
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let rows = s.link_picker_matches();
    let list = div()
        .id("link-picker-list")
        .flex()
        .flex_col()
        .max_h(px(236.))
        .overflow_y_scroll()
        .track_scroll(&s.link_scroll);
    let list = if rows.is_empty() {
        list.child(
            div()
                .id("link-picker-empty")
                .role(Role::Label)
                .test_support()
                .aria_label("No matching Items")
                .px_2()
                .py_1()
                .text_color(muted)
                .child("No matching Items"),
        )
    } else {
        list.children(rows.into_iter().enumerate().map(|(ix, c)| {
            let on = ix == highlighted && c.reachable;
            let hint = format!("Another Item has the slug `{}`", c.slug);
            menu::option_row(state, MenuSlot::Link, ix, c.title.clone(), on, cx)
                .flex_col()
                .items_start()
                .gap_0()
                .when(!c.reachable, |this| this.opacity(0.5).cursor_default())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(c.title.clone())
                        .child(div().text_xs().text_color(muted).child(c.detail.clone())),
                )
                .when(!c.reachable, |this| {
                    this.child(
                        div()
                            .id(("link-picker-hint", ix))
                            .role(Role::Label)
                            .test_support()
                            .aria_label(hint.clone())
                            .text_xs()
                            .text_color(muted)
                            .child(hint),
                    )
                })
                .into_any_element()
        }))
    };
    Some(menu::popover(
        state,
        MenuSlot::Link,
        at,
        [list.into_any_element()],
        cx,
    ))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{LinkCandidate, matching};

    fn candidate(slug: &str, title: &str, secs: u64) -> LinkCandidate {
        LinkCandidate::new(slug, title, "Note")
            .modified(Some(UNIX_EPOCH + Duration::from_secs(secs)))
    }

    fn titles(candidates: &[LinkCandidate], query: &str) -> Vec<String> {
        matching(candidates, query)
            .into_iter()
            .map(|c| c.title.to_string())
            .collect()
    }

    #[test]
    fn matches_title_or_slug_ignoring_case() {
        let all = [
            candidate("kitchen-reno", "Reno plan", 1),
            candidate("other", "Other", 2),
        ];
        assert_eq!(titles(&all, "RENO"), ["Reno plan"]);
        assert_eq!(titles(&all, "kitchen"), ["Reno plan"]);
        assert!(titles(&all, "zzz").is_empty());
    }

    #[test]
    fn title_prefixes_come_first_then_the_most_recent() {
        let all = [
            candidate("a", "My quotes", 90),
            candidate("b", "Quotes", 10),
            candidate("c", "Quick", 20),
            candidate("d", "Other", 100).modified(None),
        ];
        assert_eq!(titles(&all, "qu"), ["Quick", "Quotes", "My quotes"]);
        // Empty query: everything, newest first, undated last.
        assert_eq!(titles(&all, ""), ["My quotes", "Quick", "Quotes", "Other"]);
    }
}
