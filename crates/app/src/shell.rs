use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, MouseButton, ParentElement as _, Render, Styled as _, Subscription, Window,
    actions, div, prelude::FluentBuilder as _,
};
use sett_store::Space;

use crate::center::CenterPanel;
use crate::nav::Nav;
use crate::project_dialog;
use crate::workspace::{Selection, Workspace};

actions!(sett, [ShowPersonal, ShowWork]);

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
    ]);
}

/// The main window: a title bar with the Space toggle above the left nav and
/// the center panel.
pub struct Shell {
    workspace: Entity<Workspace>,
    center: Entity<CenterPanel>,
    focus_handle: FocusHandle,
    _observe: Subscription,
}

impl Shell {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let center = cx.new(|cx| CenterPanel::new(workspace.clone(), window, cx));
        let observe = cx.observe(&workspace, |_, _, cx| cx.notify());
        let focus_handle = cx.focus_handle();
        // The Space shortcuts live on the shell's key context, so it needs focus
        // for them to work before anything inside it has been clicked.
        focus_handle.focus(window, cx);
        Shell {
            workspace,
            center,
            focus_handle,
            _observe: observe,
        }
    }

    pub fn workspace(&self) -> &Entity<Workspace> {
        &self.workspace
    }

    fn show_space(&mut self, space: Space, cx: &mut Context<Self>) {
        self.workspace
            .update(cx, |workspace, cx| workspace.switch_space(space, cx));
    }

    /// The selected Project's slug and whether it's archived, unless nothing
    /// or the Inbox (which can't be edited, archived or deleted) is selected.
    fn selected_project(&self, cx: &App) -> Option<(String, bool)> {
        let workspace = self.workspace.read(cx);
        let Selection::Project(slug) = workspace.selection() else {
            return None;
        };
        let project = workspace.index().project(&slug)?;
        (!project.is_inbox()).then_some((slug, project.archived))
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = match self.workspace.read(cx).space() {
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
                            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                let space = if *ix == 0 {
                                    Space::Personal
                                } else {
                                    Space::Work
                                };
                                this.show_space(space, cx);
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
            .on_action(
                cx.listener(|this, _: &ShowPersonal, _, cx| this.show_space(Space::Personal, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowWork, _, cx| this.show_space(Space::Work, cx)))
            .on_action(cx.listener(|this, _: &NewProject, window, cx| {
                project_dialog::open_new_project(this.workspace.clone(), window, cx)
            }))
            .when_some(selected, |this, (slug, archived)| {
                let workspace = self.workspace.clone();
                let (edit_ws, edit_slug) = (workspace.clone(), slug.clone());
                let (delete_ws, delete_slug) = (workspace.clone(), slug.clone());
                this.on_action(move |_: &EditProject, window, cx| {
                    project_dialog::open_edit_project(
                        edit_ws.clone(),
                        edit_slug.clone(),
                        window,
                        cx,
                    )
                })
                .on_action(move |_: &DeleteProject, window, cx| {
                    project_dialog::confirm_delete_project(
                        delete_ws.clone(),
                        delete_slug.clone(),
                        window,
                        cx,
                    )
                })
                .map(|this| {
                    if archived {
                        this.on_action(move |_: &UnarchiveProject, window, cx| {
                            project_dialog::set_archived(&workspace, &slug, false, window, cx)
                        })
                    } else {
                        this.on_action(move |_: &ArchiveProject, window, cx| {
                            project_dialog::set_archived(&workspace, &slug, true, window, cx)
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
                    .child(Nav::new(self.workspace.clone()))
                    .child(self.center.clone()),
            )
    }
}
