//! The Task view's Labels field: a multi-select Combobox whose search can also
//! create a Label, and whose trigger shows the Task's Labels as removable chips.

use gpui_kit::component::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::combobox::ComboboxTriggerContext;
use gpui_kit::component::searchable_list::{SearchableListDelegate, SearchableListItem};
use gpui_kit::component::{ActiveTheme as _, IndexPath, h_flex};
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::{
    AnyElement, App, ElementId, IntoElement, ParentElement as _, SharedString, Styled as _, Task,
    WeakEntity, Window, div,
};

use crate::labels::label_chip;
use crate::task_view::TaskView;

/// A row of the popup: a Label of the Space, or the row that creates the
/// Label being typed. Both are valued by the Label's text, so adding through
/// the row selects what the Task then lists.
#[derive(Debug, Clone)]
pub(crate) enum LabelOption {
    Existing(SharedString),
    Add(SharedString),
}

impl SearchableListItem for LabelOption {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        match self {
            LabelOption::Existing(label) => label.clone(),
            LabelOption::Add(label) => format!("Add \u{201c}{label}\u{201d}").into(),
        }
    }

    fn value(&self) -> &SharedString {
        match self {
            LabelOption::Existing(label) | LabelOption::Add(label) => label,
        }
    }
}

/// The Space's Labels, filtered by what's typed.
pub(crate) struct LabelOptions {
    labels: Vec<SharedString>,
    shown: Vec<LabelOption>,
    view: WeakEntity<TaskView>,
}

impl LabelOptions {
    pub(crate) fn new(labels: Vec<String>, view: WeakEntity<TaskView>) -> Self {
        let labels: Vec<SharedString> = labels.into_iter().map(SharedString::from).collect();
        let shown = labels.iter().cloned().map(LabelOption::Existing).collect();
        LabelOptions {
            labels,
            shown,
            view,
        }
    }
}

/// The spelling a typed Label takes: the Space's own when one matches
/// ignoring case, so a Label never splits into two spellings.
pub(crate) fn spelled_like_the_space(typed: &str, space: &[String]) -> String {
    let typed = typed.trim();
    space
        .iter()
        .find(|l| l.to_lowercase() == typed.to_lowercase())
        .map_or_else(|| typed.to_string(), Clone::clone)
}

impl SearchableListDelegate for LabelOptions {
    type Item = LabelOption;

    fn items_count(&self, _: usize) -> usize {
        self.shown.len()
    }

    fn item(&self, ix: IndexPath) -> Option<&LabelOption> {
        self.shown.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        LabelOption: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.shown
            .iter()
            .position(|o| o.value() == value)
            .map(|row| IndexPath::default().row(row))
    }

    fn perform_search(&mut self, query: &str, window: &mut Window, cx: &mut App) -> Task<()> {
        // A comma commits what came before it. The popup's search box can't
        // be told to do that itself, and the commit rewrites the selection,
        // which can't happen while this list is being updated.
        let query = if query.contains(',') {
            let typed: Vec<String> = query
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect();
            let view = self.view.clone();
            window.defer(cx, move |window, cx| {
                view.update(cx, |view, cx| view.add_labels(&typed, window, cx))
                    .ok();
            });
            ""
        } else {
            query
        };
        let needle = query.trim().to_lowercase();
        self.shown = self
            .labels
            .iter()
            .filter(|l| l.to_lowercase().contains(&needle))
            .cloned()
            .map(LabelOption::Existing)
            .collect();
        let known = self.labels.iter().any(|l| l.to_lowercase() == needle);
        if !needle.is_empty() && !known {
            // After partial matches, so Enter takes the best one; "doc" must
            // still be creatable while "docs" exists.
            self.shown
                .push(LabelOption::Add(query.trim().to_string().into()));
        }
        Task::ready(())
    }
}

/// The trigger: the Task's Labels as chips with an x each, wrapping onto more
/// lines as they grow.
pub(crate) fn render_trigger(
    trigger: &ComboboxTriggerContext<LabelOptions>,
    view: &WeakEntity<TaskView>,
    cx: &App,
) -> AnyElement {
    let labels: Vec<SharedString> = trigger
        .selection()
        .iter()
        .map(|(_, option)| option.value().clone())
        .collect();
    if labels.is_empty() {
        return div()
            .text_color(cx.theme().muted_foreground)
            .child(trigger.placeholder().cloned().unwrap_or_default())
            .into_any_element();
    }
    let disabled = trigger.is_disabled();
    h_flex()
        .flex_wrap()
        .gap_1()
        .children(labels.into_iter().enumerate().map(|(ix, label)| {
            let remove = Button::new(ElementId::Name(format!("remove-label-{ix}").into()))
                .ghost()
                .xsmall()
                .icon(IconName::Close)
                .accessibility_label(format!("Remove {label}"))
                .disabled(disabled)
                .on_click({
                    let (view, label) = (view.clone(), label.clone());
                    move |_, window, cx| {
                        cx.stop_propagation();
                        view.update(cx, |view, cx| view.remove_label(&label, window, cx))
                            .ok();
                    }
                });
            label_chip(&label).child(remove)
        }))
        .into_any_element()
}
