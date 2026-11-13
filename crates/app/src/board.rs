use std::cell::Cell;
use std::collections::HashMap;
use std::io;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable as _, InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render,
    Role, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, actions, div, prelude::FluentBuilder as _, px,
};
use markdown_editor::CalloutKind;
use sett_store::{Board, BoardColumn, CalloutKind as TaskCalloutKind, Date, Task, TaskCallout};

use crate::app_model::{AppModel, Selection};
use crate::confirm_delete::{ITEM_CONSEQUENCE, confirm_delete};
use crate::dates::{to_chrono, today};

actions!(sett, [OpenCard]);

const CARD_CONTEXT: &str = "Card";

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("enter", OpenCard, Some(CARD_CONTEXT))]);
}

/// What the Board tells its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardEvent {
    OpenTask { project: String, slug: String },
}

/// The selected Project's Board: its Columns side by side, each with a
/// quick-add box and its Cards. Cards drag within and between Columns.
pub(crate) struct BoardView {
    model: Entity<AppModel>,
    /// The Project shown, by slug. Changing it resets the view state below.
    project: Option<String>,
    show_all_done: bool,
    selected_task: Option<String>,
    /// Quick-add boxes by Column name.
    quick_adds: HashMap<String, QuickAdd>,
    /// Lets Tab reach each Card and Enter open it, by Task slug.
    card_focus: HashMap<String, FocusHandle>,
    column_dialog: Option<Subscription>,
    _observe: Subscription,
}

struct QuickAdd {
    input: Entity<InputState>,
    _subscription: Subscription,
}

#[derive(Clone)]
struct DraggedCard {
    slug: String,
    title: SharedString,
}

struct CardPreview {
    title: SharedString,
}

impl Render for CardPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_72()
            .px_3()
            .py_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .shadow_md()
            .text_sm()
            .child(self.title.clone())
    }
}

impl EventEmitter<BoardEvent> for BoardView {}

impl BoardView {
    pub(crate) fn new(
        model: Entity<AppModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe_in(&model, window, |this, _, window, cx| {
            this.sync(window, cx);
        });
        let mut view = BoardView {
            model,
            project: None,
            show_all_done: false,
            selected_task: None,
            quick_adds: HashMap::new(),
            card_focus: HashMap::new(),
            column_dialog: None,
            _observe: observe,
        };
        view.sync(window, cx);
        view
    }

    /// Follows the AppModel's selection and keeps one quick-add box per
    /// Column. Boxes are created here rather than in `render` because an
    /// `InputState` needs the window.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model = self.model.read(cx);
        let project = match model.selection() {
            Selection::Project(slug) => Some(slug),
            _ => None,
        };
        let columns = project
            .as_deref()
            .and_then(|p| model.index().project(p))
            .map(|p| p.columns.clone())
            .unwrap_or_default();
        let slugs: Vec<String> = project
            .as_deref()
            .map(|p| model.index().tasks(p))
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.slug.clone())
            .collect();
        if project != self.project {
            self.project = project;
            self.show_all_done = false;
            self.selected_task = None;
            self.quick_adds.clear();
            self.card_focus.clear();
        }
        self.card_focus.retain(|slug, _| slugs.contains(slug));
        for slug in slugs {
            self.card_focus
                .entry(slug)
                .or_insert_with(|| cx.focus_handle().tab_stop(true));
        }
        self.quick_adds.retain(|name, _| columns.contains(name));
        for column in columns {
            if self.quick_adds.contains_key(&column) {
                continue;
            }
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Add a Task"));
            let name = column.clone();
            let subscription =
                cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        this.quick_add(&name, input, window, cx);
                    }
                });
            self.quick_adds.insert(
                column,
                QuickAdd {
                    input,
                    _subscription: subscription,
                },
            );
        }
    }

    fn quick_add(
        &mut self,
        column: &str,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = input.read(cx).value().trim().to_string();
        let Some(project) = self.project.clone() else {
            return;
        };
        if title.is_empty() {
            return;
        }
        let result = self
            .model
            .update(cx, |w, cx| w.create_task(&project, column, &title, cx));
        match result {
            Ok(()) => input.update(cx, |input, cx| input.set_value("", window, cx)),
            Err(err) => show_error("Couldn't add the Task", err, window, cx),
        }
    }

    fn move_card(
        &mut self,
        slug: &str,
        column: &str,
        before: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.project.clone() else {
            return;
        };
        let result = self
            .model
            .update(cx, |w, cx| w.move_task(&project, slug, column, before, cx));
        if let Err(err) = result {
            show_error("Couldn't move the Task", err, window, cx);
        }
    }

    fn confirm_delete_task(
        &self,
        slug: &str,
        title: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.project.clone() else {
            return;
        };
        let model = self.model.clone();
        let slug = slug.to_string();
        confirm_delete(title, ITEM_CONSEQUENCE, window, cx, move |window, cx| {
            let result = model.update(cx, |m, cx| m.trash_task(&project, &slug, cx));
            if let Err(err) = result {
                window.push_notification(
                    Notification::error(format!("Couldn't delete the Task: {err}")),
                    cx,
                );
            }
        });
    }

    /// Opens a Task of whichever Project is now selected, as following a
    /// Link to it does.
    pub(crate) fn reveal_task(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.sync(window, cx);
        self.open_task(slug, cx);
    }

    fn open_task(&mut self, slug: &str, cx: &mut Context<Self>) {
        let Some(project) = self.project.clone() else {
            return;
        };
        self.selected_task = Some(slug.to_string());
        cx.emit(BoardEvent::OpenTask {
            project,
            slug: slug.to_string(),
        });
        cx.notify();
    }

    /// Focuses a Card, for when its Task view closes.
    pub(crate) fn focus_card(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.card_focus.get(slug) {
            handle.focus(window, cx);
        }
        cx.notify();
    }

    fn edit_column(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut AppModel, &str, &mut Context<AppModel>) -> io::Result<()>,
    ) -> bool {
        let Some(project) = self.project.clone() else {
            return false;
        };
        let result = self.model.update(cx, |w, cx| edit(w, &project, cx));
        match result {
            Ok(()) => true,
            Err(err) => {
                show_error("Couldn't change the Columns", err, window, cx);
                false
            }
        }
    }

    /// Adds a Column, or renames `rename`. Returns whether it worked, so the
    /// dialog stays open to fix a clashing name.
    fn submit_column_name(
        &mut self,
        rename: Option<&str>,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_column(window, cx, |w, project, cx| match rename {
            Some(from) => w.rename_column(project, from, name, cx),
            None => w.add_column(project, name, cx),
        })
    }

    fn open_column_dialog(
        &mut self,
        rename: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Column name")
                .default_value(rename.clone().unwrap_or_default())
        });
        let submit = {
            let rename = rename.clone();
            move |this: &mut Self,
                  input: &Entity<InputState>,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                let name = input.read(cx).value();
                if this.submit_column_name(rename.as_deref(), &name, window, cx) {
                    window.close_dialog(cx);
                }
            }
        };
        self.column_dialog = Some(cx.subscribe_in(&input, window, {
            let submit = submit.clone();
            move |this, input, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    submit(this, input, window, cx);
                }
            }
        }));

        let view = cx.entity().downgrade();
        let first_build = Cell::new(true);
        let (title, ok) = match rename {
            Some(_) => ("Rename column", "Rename"),
            None => ("New column", "Add"),
        };
        window.open_dialog(cx, move |dialog, window, cx| {
            // The builder runs again on rebuilds; focus only the first time.
            if first_build.replace(false) {
                input.focus_handle(cx).focus(window, cx);
            }
            let (view, input, submit) = (view.clone(), input.clone(), submit.clone());
            dialog.title(title).child(Input::new(&input)).footer(
                DialogFooter::new()
                    .child(
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("ok")
                            .primary()
                            .label(ok)
                            .on_click(move |_, window, cx| {
                                view.update(cx, |this, cx| submit(this, &input, window, cx))
                                    .ok();
                            }),
                    ),
            )
        });
    }

    /// A `locked` Board belongs to a Project whose frontmatter can't be read:
    /// it shows its Tasks but takes no new Tasks, Column edits or drops.
    fn render_column(
        &self,
        board: &Board,
        ix: usize,
        column: &BoardColumn,
        locked: bool,
        today: Date,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let name = column.name().to_string();
        let is_last = ix + 1 == board.columns().len();
        let (shown, hidden): (Vec<&Task>, Vec<&Task>) = column
            .tasks()
            .iter()
            .partition(|t| !is_last || self.show_all_done || t.is_recently_done(today));
        let has_older = is_last && column.tasks().iter().any(|t| !t.is_recently_done(today));

        let cards = v_flex()
            .id("cards")
            .flex_1()
            .min_h_0()
            .gap_2()
            .px_2()
            .pb_2()
            .children(
                shown
                    .iter()
                    .map(|task| self.render_card(task, &name, locked, today, cx)),
            )
            .when(has_older, |cards| {
                let label = if self.show_all_done {
                    SharedString::from("Show recent")
                } else {
                    SharedString::from(format!("Show all ({} older)", hidden.len()))
                };
                cards.child(
                    Button::new("show-all")
                        .ghost()
                        .xsmall()
                        .label(label)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_all_done = !this.show_all_done;
                            cx.notify();
                        })),
                )
            })
            .overflow_y_scrollbar();

        let drop_name = name.clone();
        v_flex()
            .id(ElementId::Name(format!("col-{name}").into()))
            .w_72()
            .flex_shrink_0()
            .min_h_0()
            .gap_2()
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().muted)
            .when(!locked, |col| {
                col.drag_over::<DraggedCard>(|style, _, _, cx| style.bg(cx.theme().accent))
                    .on_drop(cx.listener(move |this, card: &DraggedCard, window, cx| {
                        this.move_card(&card.slug, &drop_name, None, window, cx);
                    }))
            })
            .child(self.render_column_header(board, ix, column, locked, cx))
            .when_some(
                self.quick_adds.get(&name).filter(|_| !locked),
                |col, quick_add| {
                    col.child(
                        div()
                            .px_2()
                            .child(Input::new(&quick_add.input).id("quick-add").small()),
                    )
                },
            )
            .child(cards)
    }

    fn render_column_header(
        &self,
        board: &Board,
        ix: usize,
        column: &BoardColumn,
        locked: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let name = column.name().to_string();
        let last_ix = board.columns().len() - 1;
        let deletable = board.is_column_deletable(&name);
        let view = cx.entity().downgrade();
        let menu = Button::new("column-menu")
            .ghost()
            .xsmall()
            .icon(IconName::Ellipsis)
            .tooltip("Column actions")
            .dropdown_menu(move |menu, _, _| {
                let item = |label: &'static str, disabled: bool, action: ColumnAction| {
                    let (view, name) = (view.clone(), name.clone());
                    PopupMenuItem::new(label)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            view.update(cx, |this, cx| {
                                this.column_action(action, &name, ix, window, cx)
                            })
                            .ok();
                        })
                };
                menu.item(item("Rename…", false, ColumnAction::Rename))
                    .item(item("Move left", ix == 0, ColumnAction::MoveLeft))
                    .item(item("Move right", ix == last_ix, ColumnAction::MoveRight))
                    .separator()
                    .item(item("Delete", !deletable, ColumnAction::Delete))
            });
        h_flex()
            .px_3()
            .pt_2()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .truncate()
                    .child(column.name().to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(column.tasks().len().to_string()),
            )
            .child(div().flex_1())
            .when(!locked, |header| header.child(menu))
    }

    fn column_action(
        &mut self,
        action: ColumnAction,
        name: &str,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            ColumnAction::Rename => self.open_column_dialog(Some(name.to_string()), window, cx),
            ColumnAction::MoveLeft | ColumnAction::MoveRight => {
                let to = if action == ColumnAction::MoveLeft {
                    ix.saturating_sub(1)
                } else {
                    ix + 1
                };
                self.edit_column(window, cx, |w, project, cx| {
                    w.move_column(project, ix, to, cx)
                });
            }
            ColumnAction::Delete => {
                self.edit_column(window, cx, |w, project, cx| {
                    w.delete_column(project, name, cx)
                });
            }
        }
    }

    fn render_card(
        &self,
        task: &Task,
        column: &str,
        locked: bool,
        today: Date,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        // A Task whose frontmatter can't be read can't be moved, since moving
        // writes its file.
        let draggable = !locked && !task.frontmatter_error();
        let slug = task.slug.clone();
        let title = SharedString::from(task.title.clone());
        let selected = self.selected_task.as_deref() == Some(slug.as_str());
        let overdue = task.done.is_none() && task.due.is_some_and(|due| due < today);
        let checklist = task.checklist();
        let theme = cx.theme();

        let due = task.due.map(|due| {
            let (icon, color) = if overdue {
                (IconName::TriangleAlert, theme.danger)
            } else {
                (IconName::Calendar, theme.muted_foreground)
            };
            h_flex()
                .gap_1()
                .text_color(color)
                .child(Icon::new(icon).xsmall())
                .child(format_date(due, today))
        });
        let checklist = checklist.map(|(checked, total)| {
            h_flex()
                .gap_1()
                .child(Icon::new(IconName::CircleCheck).xsmall())
                .child(format!("{checked}/{total}"))
        });
        let has_meta = due.is_some() || checklist.is_some();
        let callout = task.first_callout();
        let description = callout.as_ref().map(callout_description);
        let callout = callout.map(|callout| callout_line(&callout, cx));

        let (click_slug, key_slug, menu_slug) = (slug.clone(), slug.clone(), slug.clone());
        let menu_title = task.title.clone();
        let view = cx.entity().downgrade();
        let focus = self.card_focus.get(&slug).cloned();
        let ring = theme.ring;
        let (drop_column, drop_before) = (column.to_string(), slug.clone());
        v_flex()
            .id(ElementId::Name(format!("card-{slug}").into()))
            .role(Role::ListItem)
            .test_support()
            .aria_label(title.clone())
            .when_some(description, |card, description| {
                card.aria_description(description)
            })
            .gap_1()
            .px_3()
            .py_2()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(if selected {
                theme.accent
            } else {
                theme.background
            })
            .when_some(focus, |card, focus| card.track_focus(&focus))
            .key_context(CARD_CONTEXT)
            .focus_visible(move |style| style.border_color(ring))
            .on_action(cx.listener(move |this, _: &OpenCard, _, cx| this.open_task(&key_slug, cx)))
            .on_click(cx.listener(move |this, _, _, cx| this.open_task(&click_slug, cx)))
            .when(draggable, |card| {
                card.on_drag(
                    DraggedCard {
                        slug,
                        title: title.clone(),
                    },
                    |card, _, _, cx| {
                        let title = card.title.clone();
                        cx.new(|_| CardPreview { title })
                    },
                )
            })
            .when(!locked, |card| {
                card.drag_over::<DraggedCard>(|style, _, _, cx| {
                    style.border_t_2().border_color(cx.theme().drag_border)
                })
                .on_drop(cx.listener(
                    move |this, card: &DraggedCard, window, cx| {
                        // The Column under the Card handles drops too; this one wins.
                        cx.stop_propagation();
                        this.move_card(&card.slug, &drop_column, Some(&drop_before), window, cx);
                    },
                ))
            })
            .child(div().text_sm().child(title))
            .children(callout)
            .when(has_meta, |card| {
                card.child(
                    h_flex()
                        .gap_3()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .children(due)
                        .children(checklist),
                )
            })
            .context_menu(move |menu, _, _| {
                let (view, slug, title) = (view.clone(), menu_slug.clone(), menu_title.clone());
                menu.item(
                    PopupMenuItem::new("Delete\u{2026}").on_click(move |_, window, cx| {
                        view.update(cx, |this, cx| {
                            this.confirm_delete_task(&slug, &title, window, cx)
                        })
                        .ok();
                    }),
                )
            })
    }
}

/// How many lines of a Callout a Card shows, hard breaks included.
const CARD_CALLOUT_LINES: usize = 2;

/// The editor's kind for the store's. Spelled out so a new kind on either
/// side fails to compile here instead of panicking while drawing a Card.
fn callout_kind(kind: TaskCalloutKind) -> CalloutKind {
    match kind {
        TaskCalloutKind::Note => CalloutKind::Note,
        TaskCalloutKind::Tip => CalloutKind::Tip,
        TaskCalloutKind::Important => CalloutKind::Important,
        TaskCalloutKind::Warning => CalloutKind::Warning,
        TaskCalloutKind::Caution => CalloutKind::Caution,
    }
}

/// What a Card's accessible description says about its Callout:
/// `<Type>: <first two lines joined with a space>`, with "\u{2026}" when more follow.
fn callout_description(callout: &TaskCallout) -> String {
    let (shown, more) = callout.shown(CARD_CALLOUT_LINES);
    let ellipsis = if more { "\u{2026}" } else { "" };
    format!(
        "{}: {}{ellipsis}",
        callout_kind(callout.kind).label(),
        shown.join(" ")
    )
}

/// The quiet line under a Card's title: the type's icon in its colour, then
/// the Callout's text, muted, in at most two lines in all, ending in an
/// ellipsis when cut. `line_clamp` counts again after each hard break, so a
/// Callout of several lines is drawn one line at a time instead.
fn callout_line(callout: &TaskCallout, cx: &App) -> impl IntoElement + use<> {
    let kind = callout_kind(callout.kind);
    let color = markdown_editor::callout_color(kind, cx);
    let muted = cx.theme().muted_foreground;
    let (shown, more) = callout.shown(CARD_CALLOUT_LINES);
    let text = match shown {
        [only] if !more => div()
            .text_ellipsis()
            .line_clamp(CARD_CALLOUT_LINES)
            .child(only.clone())
            .into_any_element(),
        lines => v_flex()
            .children(lines.iter().enumerate().map(|(ix, line)| {
                // Cut after the last visible line: the ellipsis is the text's
                // own when more lines follow.
                let line = if more && ix + 1 == lines.len() {
                    format!("{line}\u{2026}")
                } else {
                    line.clone()
                };
                div().truncate().child(line)
            }))
            .into_any_element(),
    };
    h_flex()
        .items_start()
        .gap_1()
        .text_xs()
        .text_color(muted)
        .child(
            // Level with the first line of text, which is a touch taller.
            Icon::default()
                .data(markdown_editor::callout_icon(kind))
                .xsmall()
                .text_color(color)
                .mt(px(2.)),
        )
        .child(div().flex_1().min_w_0().child(text))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnAction {
    Rename,
    MoveLeft,
    MoveRight,
    Delete,
}

impl Render for BoardView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = today();
        let model = self.model.read(cx);
        let Some(project) = self
            .project
            .as_deref()
            .and_then(|p| model.index().project(p))
        else {
            return div().into_any_element();
        };
        let locked = project.frontmatter_error();
        let board = Board::new(&project.columns, model.index().tasks(&project.slug));
        let columns: Vec<AnyElement> = board
            .columns()
            .iter()
            .enumerate()
            .map(|(ix, column)| {
                self.render_column(&board, ix, column, locked, today, cx)
                    .into_any_element()
            })
            .collect();
        h_flex()
            .id("board")
            .flex_1()
            .min_h_0()
            .items_stretch()
            .gap_3()
            .p_4()
            .children(columns)
            .when(!locked, |board| {
                board.child(
                    div().flex_shrink_0().child(
                        Button::new("add-column")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .label("Add column…")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_column_dialog(None, window, cx);
                            })),
                    ),
                )
            })
            .overflow_x_scrollbar()
            .into_any_element()
    }
}

/// `Mar 4`, or `Mar 4, 2027` outside the current year.
fn format_date(date: Date, today: Date) -> String {
    let Some(naive) = to_chrono(date) else {
        return date.to_string();
    };
    if date.year() == today.year() {
        naive.format("%b %-d").to_string()
    } else {
        naive.format("%b %-d, %Y").to_string()
    }
}

fn show_error(what: &str, err: io::Error, window: &mut Window, cx: &mut Context<BoardView>) {
    window.push_notification(Notification::error(format!("{what}: {err}")), cx);
}
