//! The Items the Link Picker offers: every Note, Task and Journal Entry of the
//! Space except the one being edited and those in archived Projects.

use std::sync::Arc;

use markdown_editor::LinkCandidate;
use sett_store::{ItemId, LinkTarget, SpaceIndex};

use crate::item_links::kind_label;

/// The picker's candidates for someone editing `current`.
///
/// An Item is `reachable` only if a Link to its slug resolves to it; when
/// another Item shares the slug (after edits made outside sett) the Link
/// would open the wrong one, so the picker won't insert it.
pub(crate) fn link_candidates(index: &SpaceIndex, current: &ItemId) -> Arc<[LinkCandidate]> {
    let mut items: Vec<(ItemId, String, Option<std::time::SystemTime>)> = Vec::new();
    for note in index.notes(None) {
        items.push((
            ItemId::Note {
                project: None,
                slug: note.slug.clone(),
            },
            note.title.clone(),
            note.modified,
        ));
    }
    for project in index.projects().into_iter().filter(|p| !p.archived) {
        for note in index.notes(Some(&project.slug)) {
            items.push((
                ItemId::Note {
                    project: Some(project.slug.clone()),
                    slug: note.slug.clone(),
                },
                note.title.clone(),
                note.modified,
            ));
        }
        for task in index.tasks(&project.slug) {
            items.push((
                ItemId::Task {
                    project: project.slug.clone(),
                    slug: task.slug.clone(),
                },
                task.title.clone(),
                task.modified,
            ));
        }
    }
    for entry in index.journal() {
        items.push((
            ItemId::Journal(entry.date),
            entry.date.to_string(),
            entry.modified,
        ));
    }
    items
        .into_iter()
        .filter(|(id, _, _)| id != current)
        .map(|(id, title, modified)| {
            let reachable = matches!(
                index.resolve(&id.slug()),
                LinkTarget::Found { item, .. } if item == id
            );
            LinkCandidate::new(id.slug(), title, kind_label(index, &id))
                .modified(modified)
                .reachable(reachable)
        })
        .collect()
}
