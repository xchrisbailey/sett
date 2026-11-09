use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, div,
};

use crate::board::BoardView;
use crate::workspace::{Selection, Workspace};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ProjectTab {
    #[default]
    Board,
    Notes,
}

/// Shows whatever the nav has selected. Journal, Notes and the Project's
/// Notes are placeholders until their own views land.
pub(crate) struct CenterPanel {
    workspace: Entity<Workspace>,
    board: Entity<BoardView>,
    project_tab: ProjectTab,
    _observe: Subscription,
}

impl CenterPanel {
    pub(crate) fn new(
        workspace: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&workspace, |_, _, cx| cx.notify());
        let board = cx.new(|cx| BoardView::new(workspace.clone(), window, cx));
        CenterPanel {
            workspace,
            board,
            project_tab: ProjectTab::default(),
            _observe: observe,
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
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                this.project_tab = if *ix == 0 {
                    ProjectTab::Board
                } else {
                    ProjectTab::Notes
                };
                cx.notify();
            }))
            .child(Tab::new().label("Board"))
            .child(Tab::new().label("Notes"));
        let body = match self.project_tab {
            ProjectTab::Board => self.board.clone().into_any_element(),
            ProjectTab::Notes => self
                .render_placeholder("This Project's Notes will show here.", cx)
                .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(self.render_header(name, cx).child(tabs))
            .child(body)
            .into_any_element()
    }

    fn render_page(
        &self,
        title: &'static str,
        placeholder: &'static str,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .size_full()
            .child(self.render_header(title.into(), cx))
            .child(self.render_placeholder(placeholder, cx))
            .into_any_element()
    }
}

impl Render for CenterPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.workspace.read(cx);
        let content = match workspace.selection() {
            Selection::Journal => {
                self.render_page("Journal", "Journal Entries will show here.", cx)
            }
            Selection::Notes => self.render_page("Notes", "Notes will show here.", cx),
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
