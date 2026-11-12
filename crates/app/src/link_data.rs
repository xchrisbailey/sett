//! What the block editor is told about Links: what each `[[slug]]` in the
//! body points at (for its chips) and the Items the Link Picker offers.

use std::sync::Arc;
use std::time::SystemTime;

use markdown_editor::{LinkCandidate, WikiLinkDirectory, WikiLinkInfo, WikiLinkState};
use sett_store::{ItemId, LinkTarget, SpaceIndex};

use crate::item_links::kind_label;

/// What each of `slugs` points at. A slug that names no Item is left out, so
/// its chip draws as broken.
pub(crate) fn wiki_link_directory(
    index: &SpaceIndex,
    slugs: impl IntoIterator<Item = String>,
) -> WikiLinkDirectory {
    let mut directory = WikiLinkDirectory::new();
    for slug in slugs {
        if let LinkTarget::Found { item, ambiguous } = index.resolve(&slug) {
            let state = if ambiguous {
                WikiLinkState::Ambiguous
            } else {
                WikiLinkState::Working
            };
            directory.insert(
                slug,
                WikiLinkInfo {
                    title: index.title_or_slug(&item).into(),
                    state,
                },
            );
        }
    }
    directory
}

/// The picker's candidates for someone editing `current`: every Note, Task and
/// Journal Entry of the Space except that one and those in archived Projects.
///
/// An Item is `reachable` only if a Link to its slug resolves to it; when
/// another Item shares the slug (after edits made outside sett) the Link
/// would open the wrong one, so the picker won't insert it.
pub(crate) fn link_candidates(index: &SpaceIndex, current: &ItemId) -> Arc<[LinkCandidate]> {
    let mut candidates = Vec::new();
    let mut add = |id: ItemId, title: &str, modified: Option<SystemTime>| {
        if &id == current {
            return;
        }
        let reachable = matches!(
            index.resolve(&id.slug()),
            LinkTarget::Found { item, .. } if item == id
        );
        candidates.push(
            LinkCandidate::new(id.slug(), title, kind_label(index, &id))
                .modified(modified)
                .reachable(reachable),
        );
    };
    for note in index.notes(None) {
        let id = ItemId::Note {
            project: None,
            slug: note.slug.clone(),
        };
        add(id, &note.title, note.modified);
    }
    for project in index.projects().into_iter().filter(|p| !p.archived) {
        for note in index.notes(Some(&project.slug)) {
            let id = ItemId::Note {
                project: Some(project.slug.clone()),
                slug: note.slug.clone(),
            };
            add(id, &note.title, note.modified);
        }
        for task in index.tasks(&project.slug) {
            let id = ItemId::Task {
                project: project.slug.clone(),
                slug: task.slug.clone(),
            };
            add(id, &task.title, task.modified);
        }
    }
    for entry in index.journal() {
        add(
            ItemId::Journal(entry.date),
            &entry.date.to_string(),
            entry.modified,
        );
    }
    candidates.into()
}
