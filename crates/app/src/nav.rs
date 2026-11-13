use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _};
use gpui_kit::{App, Entity, IntoElement, RenderOnce, SharedString, Window};
use sett_store::Project;

use crate::app_model::{AppModel, Selection};
use crate::icons::{TEMPLATES_ICON, project_icon};
use crate::project_dialog;

/// The left nav for the active Space: Journal, Notes and Templates, then the
/// Projects (Inbox first), then archived Projects in a folded group at the
/// bottom.
#[derive(IntoElement)]
pub(crate) struct Nav {
    model: Entity<AppModel>,
}

impl Nav {
    pub(crate) fn new(model: Entity<AppModel>) -> Self {
        Nav { model }
    }

    fn item(
        &self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        target: Selection,
        current: &Selection,
    ) -> SidebarMenuItem {
        let model = self.model.clone();
        SidebarMenuItem::new(label)
            .icon(icon)
            .active(*current == target)
            .on_click(move |_, _, cx| {
                model.update(cx, |model, cx| model.select(target.clone(), cx));
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
        let model = self.model.clone();
        let slug = project.slug.clone();
        let archived = project.archived;
        item.context_menu(move |menu, _, _| {
            let (edit_ws, edit_slug) = (model.clone(), slug.clone());
            let (archive_ws, archive_slug) = (model.clone(), slug.clone());
            let (delete_ws, delete_slug) = (model.clone(), slug.clone());
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
        let model = self.model.read(cx);
        let current = model.selection();
        let (archived, active): (Vec<&Project>, Vec<&Project>) = model
            .index()
            .projects()
            .into_iter()
            .partition(|p| p.archived);

        let model = self.model.clone();
        let mut sidebar = Sidebar::new("nav")
            .collapsible(false)
            .footer(
                Button::new("new-project")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .label("New project\u{2026}")
                    .on_click(move |_, window, cx| {
                        project_dialog::open_new_project(model.clone(), window, cx)
                    }),
            )
            .child(
                SidebarMenu::new()
                    .child(self.item("Journal", IconName::BookOpen, Selection::Journal, &current))
                    .child(self.item("Notes", IconName::FileText, Selection::Notes, &current))
                    .child(self.item("Templates", TEMPLATES_ICON, Selection::Templates, &current)),
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
