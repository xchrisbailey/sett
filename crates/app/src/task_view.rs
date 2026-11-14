use std::io;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::calendar::Date as PickerDate;
use gpui_kit::component::combobox::{Combobox, ComboboxEvent, ComboboxState};
use gpui_kit::component::date_picker::{DatePicker, DatePickerEvent, DatePickerState};
use gpui_kit::component::input::Input;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex,
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, TestSupportExt as _, Window, actions, div,
};
use sett_store::{Board, Date, ItemId, Space, same_label};

use crate::app_model::AppModel;
use crate::confirm_delete::{ITEM_CONSEQUENCE, confirm_delete};
use crate::dates::{from_chrono, to_chrono};
use crate::item_editor::{ItemEditor, ItemRef};
use crate::label_field::{LabelOptions, render_trigger, spelled_like_the_space};

actions!(sett, [CloseTask]);

const CONTEXT: &str = "TaskView";

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseTask, Some(CONTEXT))]);
}

/// The Task view asks its owner to show the Board again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskViewEvent {
    Close { slug: String },
}

type ColumnSelect = SelectState<SearchableVec<SharedString>>;
type LabelsCombobox = ComboboxState<LabelOptions>;

/// What the controls last showed, so a AppModel change that doesn't touch it
/// leaves them alone.
#[derive(PartialEq)]
struct Shown {
    columns: Vec<String>,
    column: String,
    due: Option<Date>,
    labels: Vec<String>,
    /// The Space's Labels, which a Task elsewhere can add to.
    space_labels: Vec<String>,
}

/// One Task in full: its title, its Column and due date as controls, and its
/// body in the same editor Notes use.
///
/// The Column, due date and Labels are written as soon as they change, each touching
/// only this Task's file. The title and body follow the editor's rule of
/// saving only after a real edit (ADR 0003).
pub(crate) struct TaskView {
    model: Entity<AppModel>,
    /// The open Task: its Project and slug.
    task: Option<(String, String)>,
    /// The Space the Task is in. Both Spaces have an Inbox, so a Space switch
    /// can leave the same Project selected.
    space: Space,
    editor: Entity<ItemEditor>,
    column: Entity<ColumnSelect>,
    due: Entity<DatePickerState>,
    labels: Entity<LabelsCombobox>,
    shown: Option<Shown>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TaskViewEvent> for TaskView {}

impl Focusable for TaskView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TaskView {
    pub(crate) fn new(
        model: Entity<AppModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| ItemEditor::new(model.clone(), window, cx));
        let column =
            cx.new(|cx| SelectState::new(SearchableVec::new(Vec::new()), None, window, cx));
        let due = cx.new(|cx| DatePickerState::new(window, cx).date_format("%b %-d, %Y"));
        let view = cx.entity().downgrade();
        let labels = cx.new(|cx| {
            ComboboxState::new(LabelOptions::new(Vec::new(), view), Vec::new(), window, cx)
                .multiple(true)
                .searchable(true)
        });
        let subscriptions = vec![
            cx.subscribe_in(
                &column,
                window,
                |this, _, event: &SelectEvent<SearchableVec<SharedString>>, window, cx| {
                    let SelectEvent::Confirm(Some(column)) = event else {
                        return;
                    };
                    this.move_to(&column.clone(), window, cx);
                },
            ),
            cx.subscribe_in(&due, window, |this, _, event, window, cx| {
                let DatePickerEvent::Change(value) = event;
                let due = value.start().and_then(from_chrono);
                this.set_due(due, window, cx);
            }),
            cx.subscribe_in(&labels, window, |this, labels, event, window, cx| {
                let ComboboxEvent::Change(values) = event else {
                    return;
                };
                let picked: Vec<String> = values.iter().map(ToString::to_string).collect();
                this.set_labels(picked, window, cx);
                // A pick finishes what was typed; `sync` keeps the query
                // only for changes that aren't the user's pick.
                labels.update(cx, |combobox, cx| combobox.set_query("", window, cx));
            }),
            cx.observe_in(&model, window, |this, _, window, cx| this.sync(window, cx)),
        ];
        let space = model.read(cx).space();
        TaskView {
            model,
            task: None,
            space,
            editor,
            column,
            due,
            labels,
            shown: None,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.task.is_some()
    }

    /// The open Task's title as saved, for the breadcrumb.
    pub(crate) fn title(&self, cx: &App) -> SharedString {
        self.task
            .as_ref()
            .and_then(|(project, slug)| self.model.read(cx).index().task(project, slug))
            .map(|t| SharedString::from(t.title.clone()))
            .unwrap_or_default()
    }

    /// Opens a Task of the active Space and focuses the view, so `Esc` goes
    /// back to the Board straight away.
    pub(crate) fn open(
        &mut self,
        project: &str,
        slug: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let space = self.model.read(cx).space();
        let item = ItemRef::Task {
            space,
            project: project.to_string(),
            slug: slug.to_string(),
        };
        self.editor
            .update(cx, |editor, cx| editor.open(item, window, cx));
        self.task = Some((project.to_string(), slug.to_string()));
        self.space = space;
        let id = ItemId::Task {
            project: project.to_string(),
            slug: slug.to_string(),
        };
        self.model
            .update(cx, |model, _| model.set_open_item(Some(id)));
        self.shown = None;
        self.sync(window, cx);
        self.focus_handle.focus(window, cx);
    }

    /// Goes back to the Board at the user's request, which is then what the
    /// Space reopens on.
    pub(crate) fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model.update(cx, |model, _| model.set_open_item(None));
        self.close(window, cx);
    }

    /// Saves pending edits and asks the owner to show the Board.
    pub(crate) fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| {
            editor.flush(window, cx);
            editor.discard(cx);
        });
        if let Some((_, slug)) = self.task.take() {
            cx.emit(TaskViewEvent::Close { slug });
        }
        cx.notify();
    }

    /// Keeps the controls in step with the file, and closes when the Task
    /// is gone (deleted, or its Space or Project left).
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, slug)) = self.task.clone() else {
            return;
        };
        let model = self.model.read(cx);
        let index = model.index();
        let task = index.task(&project, &slug);
        let (true, Some(project), Some(_)) =
            (model.space() == self.space, index.project(&project), task)
        else {
            self.close(window, cx);
            return;
        };
        let board = Board::new(&project.columns, index.tasks(&project.slug));
        let column = board
            .columns()
            .iter()
            .find(|c| c.tasks().iter().any(|t| t.slug == slug))
            .map(|c| c.name().to_string())
            .unwrap_or_default();
        let task = board.task(&slug);
        let due = task.and_then(|t| t.due);
        let labels = task.map(|t| t.labels.clone()).unwrap_or_default();
        let space_labels = model.space_labels();
        let shown = Some(Shown {
            columns: project.columns.clone(),
            column: column.clone(),
            due,
            labels: labels.clone(),
            space_labels: space_labels.clone(),
        });
        if shown == self.shown {
            return;
        }
        let items: Vec<SharedString> = project
            .columns
            .iter()
            .map(|c| SharedString::from(c.clone()))
            .collect();
        self.shown = shown;
        self.column.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(items), window, cx);
            select.set_selected_value(&SharedString::from(column), window, cx);
        });
        let date = PickerDate::Single(due.and_then(to_chrono));
        self.due
            .update(cx, |picker, cx| picker.set_date(date, window, cx));
        // Rebuilt every time so a Label added here is listed and stays selected.
        let view = cx.entity().downgrade();
        let selected: Vec<SharedString> = labels.into_iter().map(SharedString::from).collect();
        self.labels.update(cx, |combobox, cx| {
            // Setting values and items both discard the search text, which
            // would wipe a half-typed Label whenever another Task changes.
            let query = combobox.query(cx);
            combobox.set_items(LabelOptions::new(space_labels, view), window, cx);
            if combobox.selected_values() != selected {
                combobox.set_selected_values(&selected, window, cx);
            }
            if !query.is_empty() {
                combobox.set_query(query, window, cx);
            }
        });
        cx.notify();
    }

    /// Moves the Task to the end of `column`, as dropping its Card on that
    /// Column would.
    fn move_to(&mut self, column: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, slug)) = self.task.clone() else {
            return;
        };
        if self.shown.as_ref().is_some_and(|s| s.column == column) {
            return;
        }
        let result = self
            .model
            .update(cx, |w, cx| w.move_task(&project, &slug, column, None, cx));
        if let Err(err) = result {
            show_error("Couldn\u{2019}t move the Task", err, window, cx);
        }
    }

    fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.title(cx);
        let this = cx.entity().downgrade();
        confirm_delete(&title, ITEM_CONSEQUENCE, window, cx, move |window, cx| {
            this.update(cx, |this, cx| this.delete(window, cx)).ok();
        });
    }

    /// Trashes the open Task and goes back to the Board. Pending edits are
    /// dropped first, so saving them can't bring the file back.
    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, slug)) = self.task.clone() else {
            return;
        };
        self.editor.update(cx, |editor, cx| editor.discard(cx));
        let result = self
            .model
            .update(cx, |m, cx| m.trash_task(&project, &slug, cx));
        match result {
            Ok(()) => self.back(window, cx),
            Err(err) => show_error("Couldn\u{2019}t delete the Task", err, window, cx),
        }
    }

    fn set_due(&mut self, due: Option<Date>, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, slug)) = self.task.clone() else {
            return;
        };
        let result = self
            .model
            .update(cx, |w, cx| w.set_task_due(&project, &slug, due, cx));
        if let Err(err) = result {
            show_error("Couldn\u{2019}t change the due date", err, window, cx);
        }
    }

    /// The open Task's Labels as last shown.
    fn current_labels(&self) -> Vec<String> {
        self.shown
            .as_ref()
            .map(|s| s.labels.clone())
            .unwrap_or_default()
    }

    /// Adds typed Labels, in the Space's spelling when it has one.
    pub(crate) fn add_labels(
        &mut self,
        typed: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let space = self.model.read(cx).space_labels();
        let mut labels = self.current_labels();
        labels.extend(typed.iter().map(|t| spelled_like_the_space(t, &space)));
        self.set_labels(labels, window, cx);
        self.labels
            .update(cx, |combobox, cx| combobox.set_query("", window, cx));
    }

    pub(crate) fn remove_label(
        &mut self,
        label: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut labels = self.current_labels();
        labels.retain(|l| !same_label(l, label));
        self.set_labels(labels, window, cx);
    }

    fn set_labels(&mut self, labels: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, slug)) = self.task.clone() else {
            return;
        };
        let result = self
            .model
            .update(cx, |w, cx| w.set_task_labels(&project, &slug, &labels, cx));
        if let Err(err) = result {
            show_error("Couldn\u{2019}t change the Labels", err, window, cx);
        }
        // A rejected or ignored change must not leave the field showing it.
        self.shown = None;
        self.sync(window, cx);
    }

    fn render_field(label: &'static str, control: impl IntoElement, cx: &App) -> impl IntoElement {
        h_flex()
            .gap_3()
            .child(
                div()
                    .w_20()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(div().w_56().child(control))
    }
}

fn show_error(what: &str, err: io::Error, window: &mut Window, cx: &mut App) {
    window.push_notification(Notification::error(format!("{what}. {err}.")), cx);
}

impl Render for TaskView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.task.is_none() {
            return div().into_any_element();
        }
        let editor = self.editor.read(cx);
        let (title, locked) = (editor.title_input().clone(), editor.locked());
        let view = cx.entity().downgrade();
        v_flex()
            .id("task-view")
            .test_support()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &CloseTask, window, cx| this.back(window, cx)))
            .size_full()
            .min_h_0()
            .child(
                v_flex()
                    .px_6()
                    .pt_4()
                    .gap_2()
                    .child(
                        Input::new(&title)
                            .id("task-title")
                            .readonly(locked)
                            .appearance(false)
                            .large()
                            .text_xl()
                            .font_semibold(),
                    )
                    .child(
                        v_flex()
                            .px_3()
                            .gap_2()
                            .child(Self::render_field(
                                "Column",
                                Select::new(&self.column)
                                    .id("task-column")
                                    .small()
                                    .disabled(locked)
                                    .accessibility_label("Column"),
                                cx,
                            ))
                            .child(Self::render_field(
                                "Due",
                                div().id("task-due").test_support().child(
                                    DatePicker::new(&self.due)
                                        .small()
                                        .placeholder("No due date")
                                        .cleanable(true)
                                        .disabled(locked),
                                ),
                                cx,
                            ))
                            .child(Self::render_field(
                                "Labels",
                                div().id("task-labels").test_support().child(
                                    Combobox::new(&self.labels)
                                        .small()
                                        .placeholder("Add a label")
                                        .search_placeholder("Find or add a label")
                                        .disabled(locked)
                                        .render_trigger(move |trigger, _, cx| {
                                            render_trigger(trigger, &view, cx)
                                        }),
                                ),
                                cx,
                            ))
                            .child(
                                h_flex().child(
                                    Button::new("delete-task")
                                        .ghost()
                                        .small()
                                        .label("Delete\u{2026}")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.confirm_delete(window, cx)
                                        })),
                                ),
                            ),
                    ),
            )
            .child(self.editor.clone())
            .into_any_element()
    }
}
