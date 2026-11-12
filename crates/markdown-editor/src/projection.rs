//! Projection of the editing model into the shared renderer's `BlockNode`s
//! (design §7.3, D3).
//!
//! Each top-level block is projected directly into `BlockNode`s
//! (`direct.rs`), so the editor renders through the same renderer as
//! `TextView`. Every text leaf's `InlineState` is tagged with its `BlockId` as the
//! `editor_key`, which is how the renderer reports layouts back (the seam
//! in `text::editor_seam`). A top-level block is re-projected only when its
//! per-block revision changes, so one keystroke re-projects one block.

use std::{collections::HashMap, rc::Rc};

use super::{
    direct::NodeCache,
    document::{BlockContent, EditorDocument},
    id::BlockId,
    inline::InlineText,
    kind::BlockKindData,
    wiki_chip::{ChipMap, OpenSpans, chip_spans},
};
use gpui_kit::base::text::editor_host::BlockNode;

/// One projected top-level block.
pub(crate) struct ProjectedBlock {
    pub(crate) id: BlockId,
    revision: u64,
    pub(crate) nodes: Vec<BlockNode>,
}

/// Geometry facts the frame needs about a caret leaf.
// FORK: no longer `Copy`, as it holds the chip map.
#[derive(Clone, Debug)]
pub(crate) struct LeafInfo {
    pub(crate) block: BlockId,
    /// Caret offsets are 0/1 (code blocks in the spike).
    pub(crate) atomic: bool,
    /// Model text length (bytes); for atomic leaves the rendered text length.
    pub(crate) len: usize,
    /// FORK: the Link chips drawn in this leaf, if any.
    pub(crate) chips: Option<Rc<ChipMap>>,
}

#[derive(Default)]
pub(crate) struct Snapshot {
    pub(crate) blocks: Vec<Rc<ProjectedBlock>>,
    /// Leaves that have a rendered text layout, in document order.
    pub(crate) leaves: Rc<Vec<LeafInfo>>,
    /// Tables and their keyed cells, for node selection (no caret in cells).
    pub(crate) tables: Rc<Vec<(BlockId, Vec<BlockId>)>>,
    /// FORK: the chip map of each leaf that has chips.
    pub(crate) chips: Rc<HashMap<BlockId, Rc<ChipMap>>>,
}

#[derive(Default)]
pub(crate) struct ProjectionCache {
    revision: Option<u64>,
    snapshot: Rc<Snapshot>,
    by_id: HashMap<BlockId, Rc<ProjectedBlock>>,
    pub(crate) nodes: NodeCache,
    /// `cfg(test)`-checked perf counter (D9): snapshot rebuilds.
    pub(crate) rebuilds: usize,
}

impl ProjectionCache {
    /// FORK: `open` holds the Link spans shown as source. A change of it
    /// rebuilds the old and new leaves although the document did not change.
    pub(crate) fn snapshot(&mut self, doc: &EditorDocument, open: OpenSpans) -> Rc<Snapshot> {
        // FORK: leaves whose open spans changed are rebuilt.
        let dirty = self.nodes.set_open(doc, open);
        if !dirty.is_empty() {
            for id in dirty {
                self.by_id.remove(&id);
            }
            self.revision = None;
        }
        if self.revision == Some(doc.revision) {
            return self.snapshot.clone();
        }
        self.rebuilds += 1;
        let top = doc.children(doc.root);
        let mut blocks = Vec::with_capacity(top.len());
        let mut by_id = HashMap::with_capacity(top.len());
        for id in top {
            let revision = doc.block_revision(*id);
            let block = match self.by_id.get(id) {
                Some(prev) if prev.revision == revision => prev.clone(),
                _ => Rc::new(ProjectedBlock {
                    id: *id,
                    revision,
                    nodes: self.nodes.node(doc, *id).into_iter().collect(),
                }),
            };
            by_id.insert(*id, block.clone());
            blocks.push(block);
        }
        self.nodes.collect_garbage(doc);
        let leaves: Vec<LeafInfo> = doc
            .leaves()
            .iter()
            // FORK: the open spans decide which Links are chips.
            .filter_map(|id| leaf_info(doc, *id, &self.nodes.open_of(*id)))
            .collect();
        let mut tables: Vec<(BlockId, Vec<BlockId>)> = Vec::new();
        for id in doc.leaves() {
            if doc.kind(*id) != Some(&BlockKindData::TableCell) {
                continue;
            }
            let Some(table) = doc
                .path(*id)
                .into_iter()
                .rev()
                .find(|b| matches!(doc.kind(*b), Some(BlockKindData::Table { .. })))
            else {
                continue;
            };
            match tables.last_mut() {
                Some((last, cells)) if *last == table => cells.push(*id),
                _ => tables.push((table, vec![*id])),
            }
        }
        self.by_id = by_id;
        let chips = leaves
            .iter()
            .filter_map(|leaf: &LeafInfo| Some((leaf.block, leaf.chips.clone()?)))
            .collect();
        self.snapshot = Rc::new(Snapshot {
            chips: Rc::new(chips),
            blocks,
            leaves: Rc::new(leaves),
            tables: Rc::new(tables),
        });
        self.revision = Some(doc.revision);
        self.snapshot.clone()
    }

    /// Whether images are projected as placeholders; a change drops the cache.
    pub(crate) fn set_placeholders(&mut self, placeholders: bool) {
        if self.nodes.placeholders != placeholders {
            self.invalidate();
            self.nodes.placeholders = placeholders;
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.revision = None;
        self.by_id.clear();
        self.nodes.clear();
    }

    /// Nodes and table rows built rather than reused (D9).
    #[cfg_attr(not(all(test, feature = "view-tests")), allow(dead_code))]
    pub(crate) fn built(&self) -> usize {
        self.nodes.built
    }
}

fn leaf_info(
    doc: &EditorDocument,
    id: BlockId,
    // FORK: the leaf's open Link spans.
    open: &[std::ops::Range<usize>],
) -> Option<LeafInfo> {
    let entry = doc.get(id)?;
    match &entry.content {
        BlockContent::Inline(inline) => Some(LeafInfo {
            block: id,
            atomic: false,
            len: inline.len(),
            chips: chip_map(inline, open),
        }),
        BlockContent::Plain(code) => Some(LeafInfo {
            block: id,
            atomic: false,
            len: code.len(),
            chips: None,
        }),
        _ => None,
    }
}

/// FORK: the chips drawn in a leaf: its Link spans except the open ones.
fn chip_map(inline: &InlineText, open: &[std::ops::Range<usize>]) -> Option<Rc<ChipMap>> {
    if !inline.text().contains("[[") {
        return None;
    }
    let mut spans = chip_spans(inline);
    spans.retain(|span| !open.contains(span));
    ChipMap::new(spans).map(Rc::new)
}
