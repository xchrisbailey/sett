use gpui_kit::component::IconName;
use gpui_kit::component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};
use gpui_kit::{App, Entity, IntoElement, RenderOnce, SharedString, Window};
use sett_store::Project;

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
        icon: IconName,
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
        self.item(
            project.name.clone(),
            project_icon(project.icon.as_deref()),
            Selection::Project(project.slug.clone()),
            current,
        )
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

        let mut sidebar = Sidebar::new("nav")
            .collapsible(false)
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

/// Maps a Project's Lucide icon name onto the bundled default icons, falling
/// back to a folder. The curated icon set replaces this (issue #6).
fn project_icon(name: Option<&str>) -> IconName {
    match name {
        Some("inbox") => IconName::Inbox,
        Some("book-open") => IconName::BookOpen,
        Some("bot") => IconName::Bot,
        Some("building-2") => IconName::Building2,
        Some("calendar") => IconName::Calendar,
        Some("cpu") => IconName::Cpu,
        Some("file-text") => IconName::FileText,
        Some("folder-open") => IconName::FolderOpen,
        Some("github") => IconName::Github,
        Some("globe") => IconName::Globe,
        Some("heart") => IconName::Heart,
        Some("layout-dashboard") => IconName::LayoutDashboard,
        Some("map") => IconName::Map,
        Some("palette") => IconName::Palette,
        Some("star") => IconName::Star,
        Some("user") => IconName::User,
        _ => IconName::Folder,
    }
}
