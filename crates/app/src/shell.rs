use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, MouseButton, ParentElement as _, Render, Styled as _, Subscription, Window,
    actions, div, prelude::FluentBuilder as _,
};
use sett_store::Space;

use crate::app_model::{AppModel, Selection};
use crate::center::CenterPanel;
use crate::nav::Nav;
use crate::project_dialog;

actions!(sett, [ShowPersonal, ShowWork, ToggleEditorMode]);

// The menu-bar route to the commands the nav's context menu offers. Each acts
// on the selected Project and is only handled when it applies, which is what
// enables or disables its menu item.
actions!(
    sett,
    [
        NewProject,
        EditProject,
        ArchiveProject,
        UnarchiveProject,
        DeleteProject
    ]
);

const CONTEXT: &str = "Shell";

/// Binds the shell's shortcuts. Call before `set_menus` so the menu shows them.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-1", ShowPersonal, Some(CONTEXT)),
        KeyBinding::new("cmd-2", ShowWork, Some(CONTEXT)),
        KeyBinding::new("cmd-/", ToggleEditorMode, Some(CONTEXT)),
    ]);
}

/// The main window: a title bar with the Space toggle above the left nav and
/// the center panel.
pub struct Shell {
    model: Entity<AppModel>,
    center: Entity<CenterPanel>,
    focus_handle: FocusHandle,
    _observe: Subscription,
}

impl Shell {
    pub fn new(model: Entity<AppModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let center = cx.new(|cx| CenterPanel::new(model.clone(), window, cx));
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        let focus_handle = cx.focus_handle();
        // The Space shortcuts live on the shell's key context, so it needs focus
        // for them to work before anything inside it has been clicked.
        focus_handle.focus(window, cx);
        Shell {
            model,
            center,
            focus_handle,
            _observe: observe,
        }
    }

    pub fn model(&self) -> &Entity<AppModel> {
        &self.model
    }

    /// Switches Space and takes focus back, since whatever had it (an open
    /// Task, say) may belong to the Space being left and stop rendering, which
    /// would leave the Space shortcuts without a key context.
    fn show_space(&mut self, space: Space, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        self.model
            .update(cx, |model, cx| model.switch_space(space, cx));
    }

    /// The selected Project's slug and whether it's archived, unless nothing
    /// or the Inbox (which can't be edited, archived or deleted) is selected.
    fn selected_project(&self, cx: &App) -> Option<(String, bool)> {
        let model = self.model.read(cx);
        let Selection::Project(slug) = model.selection() else {
            return None;
        };
        let project = model.index().project(&slug)?;
        (!project.is_inbox()).then_some((slug, project.archived))
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = match self.model.read(cx).space() {
            Space::Personal => 0,
            Space::Work => 1,
        };
        TitleBar::new().child(
            h_flex().flex_1().justify_center().child(
                // Keeps a press on the toggle from starting a window drag.
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        TabBar::new("space-toggle")
                            .segmented()
                            .small()
                            .selected_index(selected)
                            .on_click(cx.listener(|this, ix: &usize, window, cx| {
                                let space = if *ix == 0 {
                                    Space::Personal
                                } else {
                                    Space::Work
                                };
                                this.show_space(space, window, cx);
                            }))
                            .child(Tab::new().label("Personal"))
                            .child(Tab::new().label("Work")),
                    ),
            ),
        )
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_project(cx);
        v_flex()
            .size_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ShowPersonal, window, cx| {
                this.show_space(Space::Personal, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowWork, window, cx| {
                this.show_space(Space::Work, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleEditorMode, _, cx| {
                this.model
                    .update(cx, |model, cx| model.toggle_editor_mode(cx))
            }))
            .on_action(cx.listener(|this, _: &NewProject, window, cx| {
                project_dialog::open_new_project(this.model.clone(), window, cx)
            }))
            .when_some(selected, |this, (slug, archived)| {
                let model = self.model.clone();
                let (edit_model, edit_slug) = (model.clone(), slug.clone());
                let (delete_model, delete_slug) = (model.clone(), slug.clone());
                this.on_action(move |_: &EditProject, window, cx| {
                    project_dialog::open_edit_project(
                        edit_model.clone(),
                        edit_slug.clone(),
                        window,
                        cx,
                    )
                })
                .on_action(move |_: &DeleteProject, window, cx| {
                    project_dialog::confirm_delete_project(
                        delete_model.clone(),
                        delete_slug.clone(),
                        window,
                        cx,
                    )
                })
                .map(|this| {
                    if archived {
                        this.on_action(move |_: &UnarchiveProject, window, cx| {
                            project_dialog::set_archived(&model, &slug, false, window, cx)
                        })
                    } else {
                        this.on_action(move |_: &ArchiveProject, window, cx| {
                            project_dialog::set_archived(&model, &slug, true, window, cx)
                        })
                    }
                })
            })
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(Nav::new(self.model.clone()))
                    .child(self.center.clone()),
            )
    }
}
