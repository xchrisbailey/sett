use std::cell::Cell;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::form::{field, v_form};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Selectable as _, Sizable as _, WindowExt as _};
use gpui_kit::{
    App, AppContext as _, Context, ElementId, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
    px,
};

use crate::app_model::AppModel;
use crate::confirm_delete::confirm_delete;
use crate::icons::{DEFAULT_ICON, PROJECT_ICONS, icon_name, project_icon};

/// Opens the dialog that creates a Project in the active Space.
pub(crate) fn open_new_project(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
    open_project_form(model, None, window, cx);
}

/// Opens the dialog that renames a Project or changes its icon.
pub(crate) fn open_edit_project(
    model: Entity<AppModel>,
    slug: String,
    window: &mut Window,
    cx: &mut App,
) {
    open_project_form(model, Some(slug), window, cx);
}

/// Asks before moving a Project's folder to the Trash.
pub(crate) fn confirm_delete_project(
    model: Entity<AppModel>,
    slug: String,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(name) = model
        .read(cx)
        .index()
        .project(&slug)
        .map(|p| p.name.clone())
    else {
        return;
    };
    confirm_delete(
        &name,
        "Its folder, with its Tasks and Notes, moves to the Trash. \
         Links to it from other Items will stop working.",
        window,
        cx,
        move |window, cx| {
            let result = model.update(cx, |w, cx| w.trash_project(&slug, cx));
            if let Err(err) = result {
                show_error("Couldn\u{2019}t delete the Project", err, window, cx);
            }
        },
    );
}

/// Archives or unarchives a Project, reporting a failed write.
pub(crate) fn set_archived(
    model: &Entity<AppModel>,
    slug: &str,
    archived: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if let Err(err) = model.update(cx, |w, cx| w.set_archived(slug, archived, cx)) {
        show_error("Couldn\u{2019}t update the Project", err, window, cx);
    }
}

fn show_error(what: &str, err: std::io::Error, window: &mut Window, cx: &mut App) {
    window.push_notification(Notification::error(format!("{what}. {err}.")), cx);
}

/// `editing` is the slug of the Project being edited, or `None` to create one.
fn open_project_form(
    model: Entity<AppModel>,
    editing: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let existing = editing
        .as_deref()
        .and_then(|slug| model.read(cx).index().project(slug));
    let (name, icon) = match existing {
        Some(p) => (p.name.clone(), project_icon(p.icon.as_deref())),
        None if editing.is_some() => return,
        None => (String::new(), DEFAULT_ICON),
    };
    let form = cx.new(|cx| ProjectForm::new(&name, icon, window, cx));
    let (title, commit) = if editing.is_some() {
        ("Edit project", "Save")
    } else {
        ("New project", "Create")
    };
    // The builder runs again on every rebuild; focus only on the first.
    let first = Cell::new(true);
    window.open_dialog(cx, move |dialog, window, cx| {
        if first.replace(false) {
            form.read(cx)
                .name
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        }
        let form_ok = form.clone();
        let model = model.clone();
        let editing = editing.clone();
        dialog
            .title(title)
            .w(px(480.))
            .child(form.clone())
            .on_ok(move |_, window, cx| {
                let Some((name, icon)) = form_ok.update(cx, |form, cx| form.submit(cx)) else {
                    return false;
                };
                let icon = icon_name(icon);
                let result = model.update(cx, |w, cx| match &editing {
                    Some(slug) => w.update_project(slug, &name, Some(&icon), cx),
                    None => w.create_project(&name, Some(&icon), cx).map(|_| ()),
                });
                match result {
                    Ok(()) => true,
                    Err(err) => {
                        show_error("Couldn\u{2019}t save the Project", err, window, cx);
                        false
                    }
                }
            })
            .footer(
                DialogFooter::new()
                    .child(DialogClose::new().trigger(|button| button.label("Cancel")))
                    .child(
                        DialogAction::new()
                            .child(Button::new("create-project").primary().label(commit)),
                    ),
            )
    });
}

/// The fields of the create and edit dialogs: a name and an icon picked from
/// a grid that the filter narrows by icon name.
struct ProjectForm {
    name: Entity<InputState>,
    filter: Entity<InputState>,
    icon: IconName,
    missing_name: bool,
    _subscriptions: [Subscription; 2],
}

impl ProjectForm {
    fn new(name: &str, icon: IconName, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Project name")
                .default_value(name.to_string())
        });
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter icons"));
        let subscriptions = [
            cx.subscribe_in(&name_input, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) && this.missing_name {
                    this.missing_name = false;
                    cx.notify();
                }
            }),
            cx.subscribe_in(&filter, window, |_, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        ProjectForm {
            name: name_input,
            filter,
            icon,
            missing_name: false,
            _subscriptions: subscriptions,
        }
    }

    /// The trimmed name and chosen icon, or `None` after flagging an empty name.
    fn submit(&mut self, cx: &mut Context<Self>) -> Option<(String, IconName)> {
        let name = self.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.missing_name = true;
            cx.notify();
            return None;
        }
        Some((name, self.icon))
    }

    fn render_icon_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Lucide names are kebab-case, so "credit card" finds `credit-card`.
        let query = self
            .filter
            .read(cx)
            .value()
            .trim()
            .to_lowercase()
            .replace(' ', "-");
        let icons: Vec<(IconName, SharedString)> = PROJECT_ICONS
            .iter()
            .map(|icon| (*icon, icon_name(*icon)))
            .filter(|(_, name)| name.contains(query.as_str()))
            .collect();

        let body = if icons.is_empty() {
            div()
                .py_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No icons match.")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(icons.into_iter().map(|(icon, name)| {
                    Button::new(ElementId::Name(format!("icon-{name}").into()))
                        .ghost()
                        .icon(Icon::new(icon))
                        .selected(icon == self.icon)
                        .tooltip(name.clone())
                        .accessibility_label(name)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.icon = icon;
                            cx.notify();
                        }))
                }))
                .into_any_element()
        };
        // A fixed height keeps the dialog from jumping as the filter narrows.
        div()
            .id("icon-grid")
            .h(px(184.))
            .child(body)
            .overflow_y_scrollbar()
    }
}

impl Render for ProjectForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut name_field = field()
            .label("Name")
            .child(Input::new(&self.name).id("project-name"));
        if self.missing_name {
            name_field = name_field.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child("Enter a name."),
            );
        }
        v_form().child(name_field).child(
            field().label("Icon").child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Input::new(&self.filter)
                            .id("icon-filter")
                            .small()
                            .cleanable(true)
                            .prefix(Icon::new(gpui_kit::component::IconName::Search).small()),
                    )
                    .child(self.render_icon_grid(cx)),
            ),
        )
    }
}
