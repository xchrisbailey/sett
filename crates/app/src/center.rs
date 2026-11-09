use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, div,
};

use crate::board::BoardView;
use crate::item_editor::{ItemEditor, ItemRef};
use crate::notes::{NotesScope, NotesView};
use crate::workspace::{Selection, Workspace, today};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ProjectTab {
    #[default]
    Board,
    Notes,
}

/// Shows whatever the nav has selected: today's Journal Entry, the Notes
/// list, or a Project's Board or Notes.
pub(crate) struct CenterPanel {
    workspace: Entity<Workspace>,
    board: Entity<BoardView>,
    notes: Entity<NotesView>,
    project_notes: Entity<NotesView>,
    journal: Entity<ItemEditor>,
    project_tab: ProjectTab,
    /// What was showing at the last sync, to notice leaving it.
    shown: Option<(Selection, ProjectTab)>,
    _observe: Subscription,
}

impl CenterPanel {
    pub(crate) fn new(
        workspace: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe_in(&workspace, window, |this, _, window, cx| {
            this.sync(window, cx);
            cx.notify();
        });
        let board = cx.new(|cx| BoardView::new(workspace.clone(), window, cx));
        let notes = cx.new(|cx| NotesView::new(workspace.clone(), NotesScope::Space, window, cx));
        let project_notes =
            cx.new(|cx| NotesView::new(workspace.clone(), NotesScope::SelectedProject, window, cx));
        let journal = cx.new(|cx| ItemEditor::new(workspace.clone(), window, cx));
        let mut panel = CenterPanel {
            workspace,
            board,
            notes,
            project_notes,
            journal,
            project_tab: ProjectTab::default(),
            shown: None,
            _observe: observe,
        };
        panel.sync(window, cx);
        panel
    }

    /// Saves pending edits in whatever was just hidden, and opens today's
    /// Journal Entry when the Journal is shown.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.workspace.read(cx);
        let (space, selection) = (workspace.space(), workspace.selection());
        let shown = Some((selection.clone(), self.project_tab));
        if shown == self.shown {
            return;
        }
        self.shown = shown;
        self.notes.update(cx, |notes, cx| notes.flush(window, cx));
        self.project_notes
            .update(cx, |notes, cx| notes.flush(window, cx));
        self.journal
            .update(cx, |journal, cx| journal.flush(window, cx));
        if selection == Selection::Journal {
            let today = ItemRef::Journal {
                space,
                date: today(),
            };
            self.journal
                .update(cx, |journal, cx| journal.open(today, window, cx));
        }
    }

    fn render_header(&self, title: SharedString, cx: &Context<Self>) -> Div {
        h_flex()
            .px_4()
            .h_12()
            .flex_shrink_0()
            .gap_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .id("center-title")
                    .role(Role::Heading)
                    .test_support()
                    .aria_label(title.clone())
                    .text_lg()
                    .font_semibold()
                    .child(title),
            )
    }

    fn render_placeholder(&self, text: &'static str, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text)
    }

    fn render_project(&self, name: SharedString, cx: &mut Context<Self>) -> AnyElement {
        let tabs = TabBar::new("project-tabs")
            .underline()
            .selected_index(match self.project_tab {
                ProjectTab::Board => 0,
                ProjectTab::Notes => 1,
            })
            .on_click(cx.listener(|this, ix: &usize, window, cx| {
                this.project_tab = if *ix == 0 {
                    ProjectTab::Board
                } else {
                    ProjectTab::Notes
                };
                this.sync(window, cx);
                cx.notify();
            }))
            .child(Tab::new().label("Board"))
            .child(Tab::new().label("Notes"));
        let body = match self.project_tab {
            ProjectTab::Board => self.board.clone().into_any_element(),
            ProjectTab::Notes => self.project_notes.clone().into_any_element(),
        };
        v_flex()
            .size_full()
            .child(self.render_header(name, cx).child(tabs))
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn render_page(&self, title: &'static str, body: AnyElement, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .child(self.render_header(title.into(), cx))
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

impl Render for CenterPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.workspace.read(cx);
        let content = match workspace.selection() {
            Selection::Journal => {
                self.render_page("Journal", self.journal.clone().into_any_element(), cx)
            }
            Selection::Notes => {
                self.render_page("Notes", self.notes.clone().into_any_element(), cx)
            }
            Selection::Project(slug) => match workspace.index().project(&slug) {
                Some(project) => {
                    let name = SharedString::from(project.name.clone());
                    self.render_project(name, cx)
                }
                None => self
                    .render_placeholder("This Project no longer exists.", cx)
                    .into_any_element(),
            },
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(cx.theme().background)
            .child(content)
    }
}
