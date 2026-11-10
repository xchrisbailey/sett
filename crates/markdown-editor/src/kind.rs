use gpui::SharedString;

use gpui_kit::base::text::editor_host::ColumnAlign;

/// Public, payload-light projection of a block's kind, for matching in
/// toolbar code. Payloads (code language, table alignments, definition
/// targets) stay in the crate-internal `BlockKindData`.
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockKind {
    // containers
    Root,
    Blockquote,
    List {
        ordered: bool,
    },
    ListItem {
        checked: Option<bool>,
    },
    Table,
    TableRow,
    TableCell,
    // text-bearing leaves
    Paragraph,
    Heading {
        level: u8,
    },
    CodeBlock,
    // atomic leaves
    /// Reserved: on main, images are inline nodes inside a paragraph, so the
    /// spike stores them as inline atoms and never creates this kind.
    Image,
    HorizontalRule,
    Break,
    Custom,
    Definition,
    Unknown,
}

/// Internal kind with payload.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BlockKindData {
    Root,
    Blockquote,
    List {
        ordered: bool,
        start: Option<u32>,
        /// FORK: the bullet (`-`, `*`, `+`) or ordered delimiter (`.`, `)`)
        /// the list was written with. Lists with different markers are
        /// different lists in Markdown.
        marker: char,
    },
    ListItem {
        checked: Option<bool>,
        spread: bool,
    },
    Table {
        aligns: Vec<ColumnAlign>,
        /// FORK: the delimiter row as written (`|---|:--:|`), reused on save
        /// while it still matches the columns and their alignment.
        delimiter_row: Option<SharedString>,
    },
    TableRow,
    TableCell,
    Paragraph,
    Heading {
        level: u8,
    },
    CodeBlock {
        lang: Option<SharedString>,
        meta: Option<SharedString>,
    },
    HorizontalRule,
    Break {
        html: bool,
    },
    Custom,
    Definition {
        identifier: SharedString,
        url: SharedString,
        title: Option<SharedString>,
    },
    Unknown,
}

/// FORK: the marker a list gets when the source doesn't say otherwise.
pub(crate) fn default_list_marker(ordered: bool) -> char {
    if ordered { '.' } else { '-' }
}

impl BlockKindData {
    pub(crate) fn public(&self) -> BlockKind {
        match self {
            Self::Root => BlockKind::Root,
            Self::Blockquote => BlockKind::Blockquote,
            Self::List { ordered, .. } => BlockKind::List { ordered: *ordered },
            Self::ListItem { checked, .. } => BlockKind::ListItem { checked: *checked },
            Self::Table { .. } => BlockKind::Table,
            Self::TableRow => BlockKind::TableRow,
            Self::TableCell => BlockKind::TableCell,
            Self::Paragraph => BlockKind::Paragraph,
            Self::Heading { level } => BlockKind::Heading { level: *level },
            Self::CodeBlock { .. } => BlockKind::CodeBlock,
            Self::HorizontalRule => BlockKind::HorizontalRule,
            Self::Break { .. } => BlockKind::Break,
            Self::Custom => BlockKind::Custom,
            Self::Definition { .. } => BlockKind::Definition,
            Self::Unknown => BlockKind::Unknown,
        }
    }

    /// Whether this kind holds child blocks.
    pub(crate) fn is_container(&self) -> bool {
        matches!(
            self,
            Self::Root
                | Self::Blockquote
                | Self::List { .. }
                | Self::ListItem { .. }
                | Self::Table { .. }
                | Self::TableRow
        )
    }

    /// Whether this kind holds editable inline text.
    pub(crate) fn is_textblock(&self) -> bool {
        matches!(
            self,
            Self::Paragraph | Self::Heading { .. } | Self::TableCell
        )
    }
}
