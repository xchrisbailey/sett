use std::cmp::Reverse;
use std::io;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, TestSupportExt as _, Window, div, prelude::FluentBuilder as _,
};
use sett_store::{ItemId, Note, Space};

use crate::app_model::{AppModel, Selection};
use crate::confirm_delete::{ITEM_CONSEQUENCE, confirm_delete};
use crate::item_editor::{ItemEditor, ItemRef};

/// Which Notes a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotesScope {
    /// The Space-level Notes.
    Space,
    /// The Notes of whichever Project the nav has selected.
    SelectedProject,
}

/// A flat list of Notes, most recently edited first, beside the editor for
/// the one open.
pub(crate) struct NotesView {
    model: Entity<AppModel>,
    scope: NotesScope,
    /// What the list shows: the Space, and the Project or `None` for
    /// Space-level Notes. Changing it closes the open Note.
    shown: Option<(Space, Option<String>)>,
    editor: Entity<ItemEditor>,
    _observe: Subscription,
}

impl NotesView {
    pub(crate) fn new(
        model: Entity<AppModel>,
        scope: NotesScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| ItemEditor::new(model.clone(), window, cx));
        let observe = cx.observe_in(&model, window, |this, _, window, cx| this.sync(window, cx));
        let mut view = NotesView {
            model,
            scope,
            shown: None,
            editor,
            _observe: observe,
        };
        view.sync(window, cx);
        view
    }

    /// Saves the open Note's pending edits, for when the list is hidden.
    pub(crate) fn flush(&self, window: &mut Window, cx: &mut App) {
        self.editor
            .update(cx, |editor, cx| editor.flush(window, cx));
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model = self.model.read(cx);
        let space = model.space();
        let shown = match (self.scope, model.selection()) {
            (NotesScope::Space, _) => Some((space, None)),
            (NotesScope::SelectedProject, Selection::Project(slug)) => Some((space, Some(slug))),
            (NotesScope::SelectedProject, _) => None,
        };
        if shown != self.shown {
            self.shown = shown;
            self.editor.update(cx, |editor, cx| {
                editor.flush(window, cx);
                editor.discard(cx);
            });
        }
    }

    fn project(&self) -> Option<&str> {
        self.shown.as_ref().and_then(|(_, p)| p.as_deref())
    }

    fn open_note(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((space, project)) = self.shown.clone() else {
            return;
        };
        let item = ItemRef::note(space, project.as_deref(), slug);
        self.editor
            .update(cx, |editor, cx| editor.open(item, window, cx));
        let id = ItemId::Note {
            project,
            slug: slug.to_string(),
        };
        self.model
            .update(cx, |model, _| model.set_open_item(Some(id)));
    }

    /// Opens a Note of whatever the list now shows, which may have just
    /// changed.
    pub(crate) fn reveal(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.sync(window, cx);
        self.open_note(slug, window, cx);
    }

    fn create_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.project().map(str::to_string);
        let result = self.model.update(cx, |w, cx| {
            w.create_note(project.as_deref(), "Untitled", cx)
        });
        match result {
            Ok(slug) => {
                self.open_note(&slug, window, cx);
                self.editor
                    .update(cx, |editor, cx| editor.focus_title(window, cx));
            }
            Err(err) => show_error("Couldn\u{2019}t create the Note", err, window, cx),
        }
    }

    fn confirm_delete(&self, slug: &str, title: &str, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let slug = slug.to_string();
        confirm_delete(title, ITEM_CONSEQUENCE, window, cx, move |window, cx| {
            this.update(cx, |this, cx| this.delete(&slug, window, cx))
                .ok();
        });
    }

    fn delete(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((space, project)) = self.shown.clone() else {
            return;
        };
        let item = ItemRef::note(space, project.as_deref(), slug);
        self.editor.update(cx, |editor, cx| {
            if editor.item() == Some(&item) {
                editor.discard(cx);
            }
        });
        let result = self
            .model
            .update(cx, |w, cx| w.trash_note(project.as_deref(), slug, cx));
        if let Err(err) = result {
            show_error("Couldn\u{2019}t delete the Note", err, window, cx);
        }
    }

    fn render_row(&self, note: &Note, open: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (hover, accent) = (theme.muted, theme.accent);
        let title = SharedString::from(note.title.clone());
        let (open_slug, menu_slug) = (note.slug.clone(), note.slug.clone());
        let menu_title = note.title.clone();
        let this = cx.entity().downgrade();
        div()
            .id(ElementId::Name(format!("note-{}", note.slug).into()))
            .role(Role::ListItem)
            .test_support()
            .aria_label(title.clone())
            .px_2()
            .py_1()
            .rounded(theme.radius)
            .text_sm()
            .truncate()
            .when(open, |row| row.bg(accent))
            .when(!open, |row| row.hover(move |style| style.bg(hover)))
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_note(&open_slug, window, cx)),
            )
            .child(title)
            .context_menu(move |menu, _, _| {
                let (this, slug, title) = (this.clone(), menu_slug.clone(), menu_title.clone());
                menu.item(
                    PopupMenuItem::new("Delete\u{2026}").on_click(move |_, window, cx| {
                        this.update(cx, |this, cx| {
                            this.confirm_delete(&slug, &title, window, cx)
                        })
                        .ok();
                    }),
                )
            })
    }

    fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = match self.editor.read(cx).item() {
            Some(ItemRef::Note { slug, .. }) => Some(slug.clone()),
            _ => None,
        };
        let mut notes: Vec<Note> = self
            .model
            .read(cx)
            .index()
            .notes(self.project())
            .into_iter()
            .cloned()
            .collect();
        // Most recently edited first; files without a readable time last.
        notes.sort_by(|a, b| {
            (Reverse(a.modified), &a.title, &a.slug).cmp(&(Reverse(b.modified), &b.title, &b.slug))
        });
        let theme = cx.theme();
        let list = if notes.is_empty() {
            div()
                .p_2()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("No Notes yet.")
                .into_any_element()
        } else {
            let rows: Vec<AnyElement> = notes
                .iter()
                .map(|n| {
                    self.render_row(n, open.as_ref() == Some(&n.slug), cx)
                        .into_any_element()
                })
                .collect();
            v_flex().gap_px().children(rows).into_any_element()
        };
        v_flex()
            .w_64()
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                h_flex().p_2().child(
                    Button::new("new-note")
                        .ghost()
                        .small()
                        .icon(IconName::Plus)
                        .label("New note")
                        .on_click(cx.listener(|this, _, window, cx| this.create_note(window, cx))),
                ),
            )
            .child(
                div()
                    .id("notes-scroll")
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .pb_2()
                    .child(div().id("notes-list").child(list))
                    .overflow_y_scrollbar(),
            )
            .into_any_element()
    }
}

fn show_error(what: &str, err: io::Error, window: &mut Window, cx: &mut App) {
    window.push_notification(Notification::error(format!("{what}. {err}.")), cx);
}

impl Render for NotesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.shown.is_none() {
            return div().into_any_element();
        }
        let editor = if self.editor.read(cx).item().is_some() {
            self.editor.clone().into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Select a Note or create one.")
                .into_any_element()
        };
        h_flex()
            .flex_1()
            .min_h_0()
            .size_full()
            .items_stretch()
            .child(self.render_list(cx))
            .child(div().flex_1().min_w_0().h_full().child(editor))
            .into_any_element()
    }
}
