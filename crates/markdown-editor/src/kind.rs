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
    /// FORK: `callout` is set when the quote is a Callout (`> [!NOTE]`).
    Blockquote {
        callout: Option<CalloutKind>,
    },
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
    /// FORK: a quote that opens with `[!NOTE]` and the like is a Callout.
    Blockquote {
        callout: Option<Callout>,
    },
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

/// FORK: the type of a Callout, from its `[!TYPE]` marker.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    pub const ALL: [Self; 5] = [
        Self::Note,
        Self::Tip,
        Self::Important,
        Self::Warning,
        Self::Caution,
    ];

    /// The marker a new or retyped callout is written with (`NOTE`).
    pub fn marker(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }

    /// The type's name as the editor shows it (`Note`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }

    /// The type a marker names, in any case (`note`, `Note`, `NOTE`).
    pub fn from_marker(marker: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.marker().eq_ignore_ascii_case(marker))
    }
}

/// FORK: a Callout's type and its marker exactly as written, so a
/// lowercase `[!note]` saves as it was loaded.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Callout {
    pub kind: CalloutKind,
    pub marker: SharedString,
}

impl Callout {
    /// A callout as the editor writes it: an uppercase marker.
    pub fn new(kind: CalloutKind) -> Self {
        Self {
            kind,
            marker: kind.marker().into(),
        }
    }
}

/// FORK: the marker a list gets when the source doesn't say otherwise.
pub(crate) fn default_list_marker(ordered: bool) -> char {
    if ordered { '.' } else { '-' }
}

impl BlockKindData {
    pub(crate) fn public(&self) -> BlockKind {
        match self {
            Self::Root => BlockKind::Root,
            Self::Blockquote { callout } => BlockKind::Blockquote {
                callout: callout.as_ref().map(|callout| callout.kind),
            },
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

    /// FORK: the Callout type, when this is a quote that is a Callout.
    pub(crate) fn callout_kind(&self) -> Option<CalloutKind> {
        match self {
            Self::Blockquote {
                callout: Some(callout),
            } => Some(callout.kind),
            _ => None,
        }
    }

    /// Whether this kind holds child blocks.
    pub(crate) fn is_container(&self) -> bool {
        matches!(
            self,
            Self::Root
                | Self::Blockquote { .. }
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
