//! What every Label surface shares: a Label's colour and its chip.

use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ColorName, Sizable as _};
use gpui_kit::{ParentElement as _, Styled as _, div, rems};

/// Red is left out: the Card keeps it for an overdue date.
const PALETTE: [ColorName; 8] = [
    ColorName::Orange,
    ColorName::Amber,
    ColorName::Lime,
    ColorName::Emerald,
    ColorName::Cyan,
    ColorName::Blue,
    ColorName::Violet,
    ColorName::Pink,
];

/// The most a chip grows before its text truncates.
const CHIP_MAX_WIDTH_REMS: f32 = 8.75;

/// The colour of a Label, the same on every surface and for every spelling of
/// its case. It's a hash of the lowercased text rather than a stored choice,
/// so hand-edited files colour themselves. FNV-1a is written out here because
/// `DefaultHasher` may change between Rust releases and recolour every Label.
pub fn label_color(label: &str) -> ColorName {
    let hash = label.to_lowercase().bytes().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });
    PALETTE[(hash % PALETTE.len() as u64) as usize]
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A small coloured chip showing `label`, at most 140 px wide with the text
/// truncating inside. Other surfaces add a remove button or a click handler by
/// chaining on the returned `Tag` or wrapping it.
pub fn label_chip(label: &str) -> Tag {
    Tag::color(label_color(label))
        .small()
        .max_w(rems(CHIP_MAX_WIDTH_REMS))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(label.to_string()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_keeps_its_colour_whatever_its_case() {
        // Literals worked out independently from the FNV-1a spec, so a change
        // to the hash or the palette recolours every Label and fails here.
        assert_eq!(label_color("docs"), ColorName::Lime);
        assert_eq!(label_color("DOCS"), ColorName::Lime);
        assert_eq!(label_color("Docs"), ColorName::Lime);
        assert_eq!(label_color("bug"), ColorName::Blue);
    }
}
