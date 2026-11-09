use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _};
use gpui_kit::{App, Entity, IntoElement, RenderOnce, SharedString, Window};
use sett_store::Project;

use crate::icons::project_icon;
use crate::project_dialog;
use crate::workspace::{Selection, Workspace};

/// The left nav for the active Space: Journal and Notes, then the Projects
/// (Inbox first), then archived Projects in a folded group at the bottom.
#[derive(IntoElement)]
pub(crate) struct Nav {
    workspace: Entity<Workspace>,
}

impl Nav {
    pub(crate) fn new(workspace: Entity<Workspace>) -> Self {
        Nav { workspace }
    }

    fn item(
        &self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        target: Selection,
        current: &Selection,
    ) -> SidebarMenuItem {
        let workspace = self.workspace.clone();
        SidebarMenuItem::new(label)
            .icon(icon)
            .active(*current == target)
            .on_click(move |_, _, cx| {
                workspace.update(cx, |workspace, cx| workspace.select(target.clone(), cx));
            })
    }

    fn project_item(&self, project: &Project, current: &Selection) -> SidebarMenuItem {
        let item = self.item(
            project.name.clone(),
            project_icon(project.icon.as_deref()),
            Selection::Project(project.slug.clone()),
            current,
        );
        // The Inbox can't be renamed, archived or deleted.
        if project.is_inbox() {
            return item;
        }
        let workspace = self.workspace.clone();
        let slug = project.slug.clone();
        let archived = project.archived;
        item.context_menu(move |menu, _, _| {
            let (edit_ws, edit_slug) = (workspace.clone(), slug.clone());
            let (archive_ws, archive_slug) = (workspace.clone(), slug.clone());
            let (delete_ws, delete_slug) = (workspace.clone(), slug.clone());
            menu.item(
                PopupMenuItem::new("Edit\u{2026}").on_click(move |_, window, cx| {
                    project_dialog::open_edit_project(
                        edit_ws.clone(),
                        edit_slug.clone(),
                        window,
                        cx,
                    )
                }),
            )
            .item(
                PopupMenuItem::new(if archived { "Unarchive" } else { "Archive" }).on_click(
                    move |_, window, cx| {
                        project_dialog::set_archived(
                            &archive_ws,
                            &archive_slug,
                            !archived,
                            window,
                            cx,
                        )
                    },
                ),
            )
            .separator()
            .item(
                PopupMenuItem::new("Delete\u{2026}").on_click(move |_, window, cx| {
                    project_dialog::confirm_delete_project(
                        delete_ws.clone(),
                        delete_slug.clone(),
                        window,
                        cx,
                    )
                }),
            )
        })
    }
}

impl RenderOnce for Nav {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let workspace = self.workspace.read(cx);
        let current = workspace.selection();
        let (archived, active): (Vec<&Project>, Vec<&Project>) = workspace
            .index()
            .projects()
            .into_iter()
            .partition(|p| p.archived);

        let workspace = self.workspace.clone();
        let mut sidebar = Sidebar::new("nav")
            .collapsible(false)
            .footer(
                Button::new("new-project")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .label("New project\u{2026}")
                    .on_click(move |_, window, cx| {
                        project_dialog::open_new_project(workspace.clone(), window, cx)
                    }),
            )
            .child(
                SidebarMenu::new()
                    .child(self.item("Journal", IconName::BookOpen, Selection::Journal, &current))
                    .child(self.item("Notes", IconName::FileText, Selection::Notes, &current)),
            )
            .child(
                SidebarMenu::new().children(active.iter().map(|p| self.project_item(p, &current))),
            );
        // Last, so adding a Project doesn't change the group's identity and
        // reset whether it's open.
        if !archived.is_empty() {
            sidebar = sidebar.child(
                SidebarMenu::new().child(
                    SidebarMenuItem::new("Archived")
                        .icon(IconName::FolderClosed)
                        .click_to_toggle(true)
                        .children(archived.iter().map(|p| self.project_item(p, &current))),
                ),
            );
        }
        sidebar
    }
}
