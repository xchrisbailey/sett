use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, ElementId, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _, Window, div,
};
use sett_store::{ItemId, LinkTarget, SpaceIndex};

use crate::app_model::AppModel;
use crate::item_editor::ItemRef;

/// What sits under an Item's body: the Items linking to it ("Linked from")
/// and its own Links, each opening its Item. Broken Links show but can't be
/// followed; a Link whose slug several Items share is marked ambiguous.
#[derive(IntoElement)]
pub(crate) struct ItemLinks {
    model: Entity<AppModel>,
    item: ItemId,
}

impl ItemLinks {
    pub(crate) fn new(model: Entity<AppModel>, item: ItemId) -> Self {
        ItemLinks { model, item }
    }
}

/// One row: an Item to open, or a Link that leads nowhere.
enum Row {
    Item {
        item: ItemId,
        title: SharedString,
        kind: SharedString,
        ambiguous: bool,
    },
    Broken(SharedString),
}

impl RenderOnce for ItemLinks {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let index = self.model.read(cx).index();
        let inbound_links: Vec<Row> = index
            .inbound_links(&self.item)
            .into_iter()
            .map(|b| item_row(index, b.source, b.ambiguous))
            .collect();
        let links: Vec<Row> = index
            .links(&self.item)
            .into_iter()
            .filter_map(|link| match link.target {
                LinkTarget::Found { item, .. } if item == self.item => None,
                LinkTarget::Found { item, ambiguous } => Some(item_row(index, item, ambiguous)),
                LinkTarget::Broken => Some(Row::Broken(link.slug.into())),
            })
            .collect();
        if inbound_links.is_empty() && links.is_empty() {
            return div().into_any_element();
        }
        let theme = cx.theme();
        v_flex()
            .id("item-links")
            .flex_shrink_0()
            .max_h_48()
            .overflow_y_scroll()
            .px_6()
            .py_3()
            .gap_3()
            .border_t_1()
            .border_color(theme.border)
            .children(section(
                "inbound-links",
                "Linked from",
                inbound_links,
                &self.model,
                cx,
            ))
            .children(section("outgoing-links", "Links", links, &self.model, cx))
            .into_any_element()
    }
}

/// What an Item is, for a muted label: "Task · Kitchen reno", "Note",
/// "Journal".
pub(crate) fn kind_label(index: &SpaceIndex, item: &ItemId) -> String {
    match item {
        ItemId::Note { project: None, .. } => "Note".to_string(),
        ItemId::Note {
            project: Some(p), ..
        } => format!("Note \u{b7} {}", project_name(index, p)),
        ItemId::Task { project, .. } => format!("Task \u{b7} {}", project_name(index, project)),
        ItemId::Journal(_) => "Journal".to_string(),
    }
}

fn item_row(index: &SpaceIndex, item: ItemId, ambiguous: bool) -> Row {
    let kind = kind_label(index, &item);
    Row::Item {
        title: index.title_or_slug(&item).into(),
        kind: kind.into(),
        item,
        ambiguous,
    }
}

fn project_name(index: &SpaceIndex, slug: &str) -> String {
    index
        .project(slug)
        .map_or_else(|| slug.to_string(), |p| p.name.clone())
}

fn section(
    id: &'static str,
    heading: &'static str,
    rows: Vec<Row>,
    model: &Entity<AppModel>,
    cx: &App,
) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    let muted = cx.theme().muted_foreground;
    let rows: Vec<AnyElement> = rows
        .into_iter()
        .map(|row| render_row(id, row, model, cx))
        .collect();
    Some(
        v_flex()
            .id(id)
            .role(Role::List)
            .test_support()
            .aria_label(heading)
            .gap_1()
            .items_start()
            .child(
                div()
                    .px_2()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(heading),
            )
            .children(rows)
            .into_any_element(),
    )
}

fn render_row(section: &str, row: Row, model: &Entity<AppModel>, cx: &App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    match row {
        Row::Item {
            item,
            title,
            kind,
            ambiguous,
        } => {
            let id = ElementId::Name(format!("{section}-{}", key(&item)).into());
            let model = model.clone();
            let detail = if ambiguous {
                format!("{kind} \u{b7} ambiguous")
            } else {
                kind.to_string()
            };
            let button = Button::new(id)
                .ghost()
                .small()
                .label(title)
                .child(div().text_color(muted).child(detail))
                .on_click(move |_, _, cx| {
                    model.update(cx, |w, cx| w.open_item(item.clone(), cx));
                });
            if ambiguous {
                button
                    .tooltip("Several Items share this name. This one is first by path.")
                    .into_any_element()
            } else {
                button.into_any_element()
            }
        }
        Row::Broken(slug) => h_flex()
            .id(ElementId::Name(format!("{section}-broken-{slug}").into()))
            .role(Role::ListItem)
            .test_support()
            .aria_label(SharedString::from(format!("{slug} (not found)")))
            .px_2()
            .gap_1()
            .text_sm()
            .text_color(muted)
            .child(div().line_through().child(slug))
            .child("not found")
            .into_any_element(),
    }
}

/// A stable id for an Item's row, unique within the Space.
fn key(item: &ItemId) -> String {
    match item {
        ItemId::Note { project, slug } => {
            format!("note-{}/{slug}", project.as_deref().unwrap_or(""))
        }
        ItemId::Task { project, slug } => format!("task-{project}/{slug}"),
        ItemId::Journal(date) => format!("journal-{date}"),
    }
}

impl ItemRef {
    pub(crate) fn id(&self) -> ItemId {
        match self {
            ItemRef::Note { project, slug, .. } => ItemId::Note {
                project: project.clone(),
                slug: slug.clone(),
            },
            ItemRef::Journal { date, .. } => ItemId::Journal(*date),
            ItemRef::Task { project, slug, .. } => ItemId::Task {
                project: project.clone(),
                slug: slug.clone(),
            },
        }
    }
}
