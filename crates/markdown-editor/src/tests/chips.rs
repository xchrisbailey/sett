//! FORK: Link chips in the projection (`wiki_chip.rs`, `direct.rs`,
//! `projection.rs`).

#![allow(clippy::single_range_in_vec_init)] // Test data: one-element lists of spans.

use super::*;
use crate::{
    position::BlockPosition,
    projection::{ProjectionCache, Snapshot},
    selection::BlockSelection,
    wiki_chip::{CHIP_LEN, OpenSpans},
};
use gpui_kit::base::text::editor_host::BlockNode;

const SOURCE: &str = "see [[budget]] now\n\n| h |\n| --- |\n| [[a\\|b]] |";

/// The first paragraph's children as `(text, is chip, is code)`.
fn first_paragraph(snapshot: &Snapshot) -> Vec<(String, bool, bool)> {
    match &snapshot.blocks[0].nodes[0] {
        BlockNode::Paragraph(p) => p
            .children()
            .iter()
            .map(|node| {
                (
                    node.text().to_string(),
                    node.as_custom().is_some(),
                    node.mark_runs().iter().any(|(_, mark)| mark.code),
                )
            })
            .collect(),
        other => panic!("not a paragraph: {other:?}"),
    }
}

#[test]
fn a_closed_link_is_one_chip_and_an_open_one_is_code() {
    let m = model(SOURCE);
    let leaf = m.doc.leaves()[0];
    let mut cache = ProjectionCache::default();

    let closed = cache.snapshot(&m.doc, OpenSpans::default());
    let chips = closed.leaves[0].chips.clone().expect("a chip map");
    assert_eq!(chips.to_rendered(14), 4 + CHIP_LEN);
    assert_eq!(
        first_paragraph(&closed),
        [
            ("see ".to_string(), false, false),
            ("[[budget]]".to_string(), true, false),
            (" now".to_string(), false, false),
        ]
    );

    // A caret inside the Link opens it, without touching the document.
    let revision = m.doc.revision;
    let inside = BlockSelection::caret(BlockPosition::new(leaf, 7));
    let open = cache.snapshot(&m.doc, OpenSpans::new(&m.doc, &inside, None));
    assert!(open.leaves[0].chips.is_none());
    assert_eq!(
        first_paragraph(&open),
        [
            ("see ".to_string(), false, false),
            ("[[budget]]".to_string(), false, true),
            (" now".to_string(), false, false),
        ]
    );
    assert_eq!(m.doc.revision, revision);

    // A caret at either edge leaves it closed.
    for at in [4, 14] {
        let edge = BlockSelection::caret(BlockPosition::new(leaf, at));
        let again = cache.snapshot(&m.doc, OpenSpans::new(&m.doc, &edge, None));
        assert!(again.leaves[0].chips.is_some(), "caret at {at}");
        assert!(first_paragraph(&again)[1].1);
    }
}

#[test]
fn a_selection_end_inside_opens_the_link() {
    let m = model(SOURCE);
    let leaf = m.doc.leaves()[0];
    let selection = BlockSelection::Text {
        anchor: BlockPosition::new(leaf, 0),
        head: BlockPosition::new(leaf, 9),
    };
    assert_eq!(OpenSpans::new(&m.doc, &selection, None).of(leaf), [4..14]);
}

#[test]
fn the_ime_range_opens_the_link_it_overlaps() {
    let m = model(SOURCE);
    let leaf = m.doc.leaves()[0];
    let caret = BlockSelection::caret(BlockPosition::new(leaf, 0));
    let open = OpenSpans::new(&m.doc, &caret, Some((leaf, 10..12)));
    assert_eq!(open.of(leaf), [4..14]);
    let beside = OpenSpans::new(&m.doc, &caret, Some((leaf, 14..16)));
    assert!(beside.of(leaf).is_empty());
}

#[test]
fn table_cell_links_are_chips_and_keep_their_bar() {
    let m = model(SOURCE);
    let cell = *m.doc.leaves().last().unwrap();
    assert_eq!(m.doc.leaf_text(cell), Some("[[a|b]]"));
    let mut cache = ProjectionCache::default();
    let snapshot = cache.snapshot(&m.doc, OpenSpans::default());
    let info = snapshot.leaves.iter().find(|l| l.block == cell).unwrap();
    assert_eq!(info.chips.as_ref().unwrap().to_rendered(7), CHIP_LEN);
    // The chip never reaches the serializer.
    assert!(m.source().contains("| [[a\\|b]] |"), "{}", m.source());
}

#[test]
fn opening_a_cell_link_rebuilds_the_table() {
    let m = model(SOURCE);
    let cell = *m.doc.leaves().last().unwrap();
    let mut cache = ProjectionCache::default();
    let closed = cache.snapshot(&m.doc, OpenSpans::default());
    let inside = BlockSelection::caret(BlockPosition::new(cell, 3));
    let open = cache.snapshot(&m.doc, OpenSpans::new(&m.doc, &inside, None));
    let chips = |s: &Snapshot| {
        s.leaves
            .iter()
            .find(|l| l.block == cell)
            .unwrap()
            .chips
            .is_some()
    };
    assert!(chips(&closed));
    assert!(!chips(&open));
    let BlockNode::Table(table) = &open.blocks[1].nodes[0] else {
        panic!("not a table");
    };
    let cell_node = table.rows().nth(1).unwrap().next().unwrap();
    assert!(
        cell_node.children()[0]
            .mark_runs()
            .iter()
            .any(|(_, m)| m.code)
    );
}
