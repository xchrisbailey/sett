use std::collections::HashMap;
use std::io;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _,
    Window, actions, div, prelude::FluentBuilder as _,
};
use sett_store::Template;

use crate::app_model::AppModel;
use crate::confirm_delete::{TEMPLATE_CONSEQUENCE, confirm_delete};
use crate::item_editor::{ItemEditor, ItemRef};

actions!(sett, [OpenTemplate]);

const ROW_CONTEXT: &str = "TemplateRow";

const EMPTY_TEXT: &str =
    "Templates are starter bodies for Tasks and Notes. Type / in a body to use one.";

/// What a new Template is called until the user types a title.
const NEW_TITLE: &str = "Untitled template";

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("enter", OpenTemplate, Some(ROW_CONTEXT))]);
}

/// The Space's Templates sorted by title, beside the editor for the one open.
pub(crate) struct TemplatesView {
    model: Entity<AppModel>,
    editor: Entity<ItemEditor>,
    /// Lets Tab reach each row and Enter open it, by Template slug.
    row_focus: HashMap<String, FocusHandle>,
    _observe: Subscription,
}

impl TemplatesView {
    pub(crate) fn new(
        model: Entity<AppModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| ItemEditor::new(model.clone(), window, cx));
        let observe = cx.observe_in(&model, window, |this, _, _, cx| this.sync(cx));
        let mut view = TemplatesView {
            model,
            editor,
            row_focus: HashMap::new(),
            _observe: observe,
        };
        view.sync(cx);
        view
    }

    /// Saves the open Template's pending edits, for when the list is hidden.
    pub(crate) fn flush(&self, window: &mut Window, cx: &mut App) {
        self.editor
            .update(cx, |editor, cx| editor.flush(window, cx));
    }

    /// The open Template's title as the editor has it, if one is open.
    pub(crate) fn open_title(&self, cx: &App) -> Option<SharedString> {
        let editor = self.editor.read(cx);
        editor.item()?;
        Some(editor.title_input().read(cx).value())
    }

    /// Closes the open Template, saving it first.
    pub(crate) fn close(&self, window: &mut Window, cx: &mut App) {
        self.editor.update(cx, |editor, cx| {
            editor.flush(window, cx);
            editor.discard(cx);
        });
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let slugs: Vec<String> = self
            .model
            .read(cx)
            .templates()
            .iter()
            .map(|t| t.slug.clone())
            .collect();
        self.row_focus.retain(|slug, _| slugs.contains(slug));
        for slug in slugs {
            self.row_focus
                .entry(slug)
                .or_insert_with(|| cx.focus_handle().tab_stop(true));
        }
    }

    fn open_template(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        let space = self.model.read(cx).space();
        let item = ItemRef::Template {
            space,
            slug: slug.to_string(),
        };
        self.editor
            .update(cx, |editor, cx| editor.open(item, window, cx));
        cx.notify();
    }

    fn create_template(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = self
            .model
            .update(cx, |model, cx| model.create_template(NEW_TITLE, cx));
        match result {
            Ok(slug) => {
                self.open_template(&slug, window, cx);
                self.editor
                    .update(cx, |editor, cx| editor.focus_title(window, cx));
            }
            Err(err) => show_error("Couldn\u{2019}t create the Template", err, window, cx),
        }
    }

    fn confirm_delete(&self, slug: &str, title: &str, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let slug = slug.to_string();
        confirm_delete(
            title,
            TEMPLATE_CONSEQUENCE,
            window,
            cx,
            move |window, cx| {
                this.update(cx, |this, cx| this.delete(&slug, window, cx))
                    .ok();
            },
        );
    }

    fn delete(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| {
            if matches!(editor.item(), Some(ItemRef::Template { slug: open, .. }) if open == slug) {
                editor.discard(cx);
            }
        });
        let result = self
            .model
            .update(cx, |model, cx| model.trash_template(slug, cx));
        if let Err(err) = result {
            show_error("Couldn\u{2019}t delete the Template", err, window, cx);
        }
    }

    fn render_row(&self, template: &Template, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (hover, accent, ring) = (theme.muted, theme.accent, theme.ring);
        let title = SharedString::from(template.title.clone());
        let (open_slug, key_slug, menu_slug) = (
            template.slug.clone(),
            template.slug.clone(),
            template.slug.clone(),
        );
        let focus = self.row_focus.get(&template.slug).cloned();
        let menu_title = template.title.clone();
        let this = cx.entity().downgrade();
        div()
            .id(ElementId::Name(
                format!("template-{}", template.slug).into(),
            ))
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
            .when_some(focus, |row, focus| row.track_focus(&focus))
            .key_context(ROW_CONTEXT)
            .border_1()
            .border_color(gpui_kit::transparent_black())
            .focus_visible(move |style| style.border_color(ring))
            .on_action(cx.listener(move |this, _: &OpenTemplate, window, cx| {
                this.open_template(&key_slug, window, cx)
            }))
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_template(&open_slug, window, cx)),
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
            .into_any_element()
    }

    fn new_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("new-template")
            .small()
            .icon(IconName::Plus)
            .label("New template")
            .on_click(cx.listener(|this, _, window, cx| this.create_template(window, cx)))
    }

    fn render_list(&self, templates: &[Template], cx: &mut Context<Self>) -> AnyElement {
        let open = match self.editor.read(cx).item() {
            Some(ItemRef::Template { slug, .. }) => Some(slug.clone()),
            _ => None,
        };
        let rows: Vec<AnyElement> = templates
            .iter()
            .map(|t| self.render_row(t, open.as_ref() == Some(&t.slug), cx))
            .collect();
        v_flex()
            .w_64()
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(h_flex().p_2().child(self.new_button(cx).ghost()))
            .child(
                div()
                    .id("templates-scroll")
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .pb_2()
                    .child(
                        div()
                            .id("templates-list")
                            .child(v_flex().gap_px().children(rows)),
                    )
                    .overflow_y_scrollbar(),
            )
            .into_any_element()
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .id("templates-empty")
                    .test_support()
                    .aria_label(EMPTY_TEXT)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(EMPTY_TEXT),
            )
            .child(self.new_button(cx))
            .into_any_element()
    }
}

fn show_error(what: &str, err: io::Error, window: &mut Window, cx: &mut App) {
    window.push_notification(Notification::error(format!("{what}. {err}.")), cx);
}

impl Render for TemplatesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let templates: Vec<Template> = self.model.read(cx).templates().to_vec();
        if templates.is_empty() && self.editor.read(cx).item().is_none() {
            return self.render_empty(cx);
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
                .child("Select a Template or create one.")
                .into_any_element()
        };
        h_flex()
            .flex_1()
            .min_h_0()
            .size_full()
            .items_stretch()
            .child(self.render_list(&templates, cx))
            .child(div().flex_1().min_w_0().h_full().child(editor))
            .into_any_element()
    }
}
