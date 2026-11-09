use gpui_kit::component::breadcrumb::{Breadcrumb, BreadcrumbItem};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, div, prelude::FluentBuilder as _,
};

use sett_store::{ItemId, Space};

use crate::app_model::{AppModel, AppModelEvent, Selection, today};
use crate::board::{BoardEvent, BoardView};
use crate::item_editor::{ItemEditor, ItemRef};
use crate::notes::{NotesScope, NotesView};
use crate::notice::frontmatter_notice;
use crate::task_view::{TaskView, TaskViewEvent};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ProjectTab {
    #[default]
    Board,
    Notes,
}

/// Shows whatever the nav has selected: today's Journal Entry, the Notes
/// list, or a Project's Board (or one of its Tasks) or Notes.
pub(crate) struct CenterPanel {
    model: Entity<AppModel>,
    board: Entity<BoardView>,
    /// Shown in place of the Board while a Task is open.
    task: Entity<TaskView>,
    notes: Entity<NotesView>,
    project_notes: Entity<NotesView>,
    journal: Entity<ItemEditor>,
    project_tab: ProjectTab,
    /// What was showing at the last sync, to notice leaving it.
    shown: Option<(Selection, ProjectTab)>,
    _subscriptions: Vec<Subscription>,
}

impl CenterPanel {
    pub(crate) fn new(
        model: Entity<AppModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe_in(&model, window, |this, _, window, cx| {
            this.sync(None, window, cx);
            cx.notify();
        });
        let open_item = cx.subscribe_in(&model, window, |this, _, event, window, cx| {
            let AppModelEvent::OpenItem(item) = event;
            this.open_item(item.clone(), window, cx);
            cx.notify();
        });
        let board = cx.new(|cx| BoardView::new(model.clone(), window, cx));
        let task = cx.new(|cx| TaskView::new(model.clone(), window, cx));
        let subscriptions = vec![
            observe,
            open_item,
            cx.subscribe_in(&board, window, |this, _, event, window, cx| {
                let BoardEvent::OpenTask { project, slug } = event;
                this.task
                    .update(cx, |task, cx| task.open(project, slug, window, cx));
                cx.notify();
            }),
            cx.subscribe_in(&task, window, |this, _, event, window, cx| {
                let TaskViewEvent::Close { slug } = event;
                this.board
                    .update(cx, |board, cx| board.focus_card(slug, window, cx));
                cx.notify();
            }),
        ];
        let notes = cx.new(|cx| NotesView::new(model.clone(), NotesScope::Space, window, cx));
        let project_notes =
            cx.new(|cx| NotesView::new(model.clone(), NotesScope::SelectedProject, window, cx));
        let journal = cx.new(|cx| ItemEditor::new(model.clone(), window, cx));
        let mut panel = CenterPanel {
            model,
            board,
            task,
            notes,
            project_notes,
            journal,
            project_tab: ProjectTab::default(),
            shown: None,
            _subscriptions: subscriptions,
        };
        panel.sync(None, window, cx);
        panel
    }

    /// Shows an Item the AppModel was asked to open, in the view for it.
    fn open_item(&mut self, item: ItemId, window: &mut Window, cx: &mut Context<Self>) {
        match &item {
            ItemId::Task { .. } => self.project_tab = ProjectTab::Board,
            ItemId::Note {
                project: Some(_), ..
            } => self.project_tab = ProjectTab::Notes,
            _ => {}
        }
        self.sync(Some(&item), window, cx);
        let space = self.model.read(cx).space();
        self.reveal(item, space, window, cx);
    }

    /// Saves pending edits in whatever was just hidden, and opens today's
    /// Journal Entry when the Journal is shown, unless `opening` is another
    /// Journal Entry.
    fn sync(&mut self, opening: Option<&ItemId>, window: &mut Window, cx: &mut Context<Self>) {
        let model = self.model.read(cx);
        let (space, selection) = (model.space(), model.selection());
        let shown = Some((selection.clone(), self.project_tab));
        if shown != self.shown {
            self.shown = shown;
            self.notes.update(cx, |notes, cx| notes.flush(window, cx));
            self.project_notes
                .update(cx, |notes, cx| notes.flush(window, cx));
            self.task.update(cx, |task, cx| task.close(window, cx));
            self.journal
                .update(cx, |journal, cx| journal.flush(window, cx));
            if selection == Selection::Journal && !matches!(opening, Some(ItemId::Journal(_))) {
                let today = ItemRef::Journal {
                    space,
                    date: today(),
                };
                self.journal
                    .update(cx, |journal, cx| journal.open(today, window, cx));
            }
        }
    }

    /// Opens an Item in its view, once that view is the one selected.
    fn reveal(&mut self, item: ItemId, space: Space, window: &mut Window, cx: &mut Context<Self>) {
        match item {
            ItemId::Note {
                project: None,
                slug,
            } => self
                .notes
                .update(cx, |notes, cx| notes.reveal(&slug, window, cx)),
            ItemId::Note {
                project: Some(_),
                slug,
            } => self
                .project_notes
                .update(cx, |notes, cx| notes.reveal(&slug, window, cx)),
            ItemId::Task { slug, .. } => self
                .board
                .update(cx, |board, cx| board.reveal_task(&slug, window, cx)),
            ItemId::Journal(date) => self.journal.update(cx, |journal, cx| {
                journal.open(ItemRef::Journal { space, date }, window, cx)
            }),
        }
    }

    fn render_header(&self, title: SharedString, cx: &Context<Self>) -> Div {
        self.render_header_bar(cx).child(
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

    fn render_header_bar(&self, cx: &Context<Self>) -> Div {
        h_flex()
            .px_4()
            .h_12()
            .flex_shrink_0()
            .gap_4()
            .border_b_1()
            .border_color(cx.theme().border)
    }

    /// `<project> › Board › <task>`, where the first two go back to the Board.
    fn render_breadcrumb(&self, project: SharedString, cx: &mut Context<Self>) -> Div {
        let task = self.task.read(cx).title(cx);
        let task_view = self.task.downgrade();
        let back = move || {
            let task_view = task_view.clone();
            move |_: &_, window: &mut Window, cx: &mut gpui_kit::App| {
                task_view.update(cx, |task, cx| task.back(window, cx)).ok();
            }
        };
        self.render_header_bar(cx).child(
            div().id("task-breadcrumb").test_support().child(
                Breadcrumb::new()
                    .child(BreadcrumbItem::new(project).on_click(back()))
                    .child(BreadcrumbItem::new("Board").on_click(back()))
                    .child(BreadcrumbItem::new(task)),
            ),
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

    fn render_project(
        &self,
        name: SharedString,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
                this.model.update(cx, |model, _| model.set_open_item(None));
                this.sync(None, window, cx);
                cx.notify();
            }))
            .child(Tab::new().label("Board"))
            .child(Tab::new().label("Notes"));
        let task_open = self.project_tab == ProjectTab::Board && self.task.read(cx).is_open();
        let body = match self.project_tab {
            ProjectTab::Board if task_open => self.task.clone().into_any_element(),
            ProjectTab::Board => self.board.clone().into_any_element(),
            ProjectTab::Notes => self.project_notes.clone().into_any_element(),
        };
        let header = if task_open {
            self.render_breadcrumb(name, cx)
        } else {
            self.render_header(name, cx).child(tabs)
        };
        v_flex()
            .size_full()
            .child(header)
            .when(locked, |page| {
                page.child(div().px_4().pt_3().child(frontmatter_notice()))
            })
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
        let model = self.model.read(cx);
        let content = match model.selection() {
            Selection::Journal => {
                self.render_page("Journal", self.journal.clone().into_any_element(), cx)
            }
            Selection::Notes => {
                self.render_page("Notes", self.notes.clone().into_any_element(), cx)
            }
            Selection::Project(slug) => match model.index().project(&slug) {
                Some(project) => {
                    let name = SharedString::from(project.name.clone());
                    let locked = project.frontmatter_error();
                    self.render_project(name, locked, cx)
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
