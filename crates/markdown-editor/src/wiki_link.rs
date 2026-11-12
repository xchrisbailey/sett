//! FORK: what the host knows about Links, and how a Link chip is drawn.
//!
//! The editor knows only the text of a Link. The host hands it a
//! [`WikiLinkDirectory`] saying which slug is which Item (its title) and
//! whether it resolves. The chip plugin reads the directory each frame
//! through a shared handle, so a new directory only needs a repaint.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ::markdown::mdast::Node;
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement as _, ParentElement as _, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, rems,
};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::base::text::{
    InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin,
};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex};

use gpui_kit::component::tooltip::Tooltip;

use super::wiki_chip::{CHIP_NODE, ChipData, ChipKey, LinkParts, link_parts};

/// How a Link resolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WikiLinkState {
    /// Exactly one Item has this slug.
    Working,
    /// No Item has this slug.
    Broken,
    /// Several Items share this slug.
    Ambiguous,
}

/// What the host knows about one slug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiLinkInfo {
    /// The target Item's title.
    pub title: SharedString,
    pub state: WikiLinkState,
}

/// Slug to [`WikiLinkInfo`]. A slug that is missing draws as broken.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WikiLinkDirectory {
    links: HashMap<String, WikiLinkInfo>,
}

impl WikiLinkDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, slug: impl Into<String>, info: WikiLinkInfo) {
        self.links.insert(slug.into(), info);
    }

    pub fn get(&self, slug: &str) -> Option<&WikiLinkInfo> {
        self.links.get(slug)
    }
}

/// The directory the plugin reads, shared with the editor state.
pub(crate) type DirectoryHandle = Arc<RwLock<Arc<WikiLinkDirectory>>>;

/// The chip Cmd/Ctrl-hover is over, if any, shared with the editor state.
/// It is set only while the modifier is held.
pub(crate) type HoverHandle = Arc<RwLock<Option<ChipKey>>>;

const ICON_LINK: &[u8] = include_bytes!("../assets/icons/file-text.svg");

/// Draws `sett-wiki-chip` nodes. It parses nothing: the projection makes the
/// chips.
pub(crate) struct WikiChipPlugin {
    pub(crate) directory: DirectoryHandle,
    pub(crate) hover: HoverHandle,
}

impl MarkdownPlugin for WikiChipPlugin {
    fn name(&self) -> &str {
        CHIP_NODE
    }

    fn parse(&self, _: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        None
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        _: &InlineRenderContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let data = node.data::<ChipData>()?;
        let directory = self
            .directory
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let parts = link_parts(&data.inner);
        let info = directory.get(parts.slug);
        let followable = *self
            .hover
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            == Some(data.key);
        Some(InlineElement::new(chip(
            &parts, info, followable, data.key, cx,
        )))
    }
}

/// The tooltip over a broken chip while Cmd/Ctrl is held.
fn missing_item_tip(slug: &str) -> String {
    format!("No Item has the slug `{slug}`")
}

/// The widest a chip grows; a longer title ends in an ellipsis.
const MAX_WIDTH_REMS: f32 = 20.;

/// How the chip words a Link that resolves.
enum Shape<'a> {
    Label(&'a str),
    Heading(&'a str),
    Plain,
}

/// `hovered`: Cmd/Ctrl is held with the pointer over this chip. A working
/// chip darkens and underlines; a broken one gets a tooltip. `key` makes the
/// element id unique to this chip, even when another has the same slug.
fn chip(
    parts: &LinkParts<'_>,
    info: Option<&WikiLinkInfo>,
    hovered: bool,
    key: ChipKey,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let working = info.filter(|info| info.state != WikiLinkState::Broken);
    let shape = match (parts.label, parts.heading) {
        (Some(label), _) => Shape::Label(label),
        (None, Some(heading)) => Shape::Heading(heading),
        (None, None) => Shape::Plain,
    };
    let shown = match (working, &shape) {
        // A broken Link shows its slug, whatever its label says.
        (None, _) => parts.slug.to_string(),
        (Some(_), Shape::Label(label)) => label.to_string(),
        (Some(info), Shape::Heading(heading)) => format!("{} \u{203A} {heading}", info.title),
        (Some(info), Shape::Plain) => info.title.to_string(),
    };
    let name = match working {
        None => format!("{shown} (not found)"),
        Some(info) if info.state == WikiLinkState::Ambiguous => format!("{shown} (ambiguous)"),
        Some(_) => shown,
    };
    let base = h_flex()
        .id(key.element_id())
        .role(Role::Link)
        .aria_label(name)
        .test_support()
        .items_center()
        .gap_1()
        .px_1()
        .max_w(rems(MAX_WIDTH_REMS))
        .overflow_hidden()
        .whitespace_nowrap()
        .rounded(theme.radius);
    let truncated = |text: SharedString| {
        div()
            .min_w_0()
            .overflow_hidden()
            .text_ellipsis()
            .child(text)
    };
    let Some(info) = working else {
        // A broken Link keeps its slug in a dashed outline.
        let text = parts.slug.to_string();
        let tip = missing_item_tip(parts.slug);
        return base
            .when(hovered, |chip| {
                chip.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
            })
            .border_1()
            .border_dashed()
            .border_color(theme.danger)
            .child(truncated(text.into()))
            .into_any_element();
    };
    let mut chip = base
        .bg(theme.link.opacity(if hovered { 0.28 } else { 0.12 }))
        .when(hovered, |chip| chip.underline())
        .text_color(theme.link)
        .child(Icon::default().data(ICON_LINK).xsmall());
    chip = match shape {
        Shape::Label(label) => chip.child(truncated(label.to_string().into())),
        Shape::Heading(heading) => chip
            .child(truncated(info.title.clone()))
            .child(div().text_color(theme.muted_foreground).child("\u{203A}"))
            .child(truncated(heading.to_string().into()).text_color(theme.muted_foreground)),
        Shape::Plain => chip.child(truncated(info.title.clone())),
    };
    if info.state == WikiLinkState::Ambiguous {
        chip = chip.child(
            div()
                .flex_none()
                .size(rems(0.375))
                .rounded_full()
                .bg(theme.warning),
        );
    }
    chip.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tip_over_a_broken_chip_names_the_slug() {
        assert_eq!(
            missing_item_tip("paint-colours"),
            "No Item has the slug `paint-colours`"
        );
    }
}
