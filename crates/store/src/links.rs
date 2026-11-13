//! Links between Items of one Space: `[[slug]]`, `[[slug|label]]` and
//! `[[slug#heading]]`, resolved by the slug alone.

use std::collections::{BTreeMap, BTreeSet};

use crate::Date;

/// Identifies one Item within a Space.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemId {
    Note {
        /// `None` for a Space-level Note.
        project: Option<String>,
        slug: String,
    },
    Task {
        project: String,
        slug: String,
    },
    Journal(Date),
}

impl ItemId {
    /// What a Link names to reach this Item.
    pub fn slug(&self) -> String {
        match self {
            ItemId::Note { slug, .. } | ItemId::Task { slug, .. } => slug.clone(),
            ItemId::Journal(date) => date.to_string(),
        }
    }

    /// Which candidate a Link to a duplicated slug resolves to: Space-level
    /// Notes first, then Journal Entries, then Project Items by path
    /// (`projects/<project>/notes` before `projects/<project>/tasks`).
    fn rank(&self) -> (u8, &str, u8) {
        match self {
            ItemId::Note { project: None, .. } => (0, "", 0),
            ItemId::Journal(_) => (1, "", 0),
            ItemId::Note {
                project: Some(p), ..
            } => (2, p, 0),
            ItemId::Task { project, .. } => (2, project, 1),
        }
    }
}

/// Where a Link leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// `ambiguous` when several Items share the slug; `item` is the one
    /// picked by path order.
    Found { item: ItemId, ambiguous: bool },
    /// No Item in the Space has the slug.
    Broken,
}

/// One outgoing Link of an Item, by the slug it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub slug: String,
    pub target: LinkTarget,
}

/// An Item whose body links to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundLink {
    pub source: ItemId,
    /// The Link's slug is shared by several Items, so the source may have
    /// meant another one.
    pub ambiguous: bool,
}

/// The Link graph of one Space. Each Item's outgoing slugs are parsed when
/// that Item changes; resolution and inbound Links are derived from them.
#[derive(Debug, Clone, Default)]
pub(crate) struct LinkGraph {
    /// Every Item by the slug a Link names it with.
    items: BTreeMap<String, BTreeSet<ItemId>>,
    /// Each Item's distinct outgoing slugs, in body order.
    outgoing: BTreeMap<ItemId, Vec<String>>,
    /// Which Items link to each slug.
    incoming: BTreeMap<String, BTreeSet<ItemId>>,
}

impl LinkGraph {
    /// Records the Item with this body, or forgets it for `None`.
    pub(crate) fn set(&mut self, item: ItemId, body: Option<&str>) {
        self.remove(&item);
        let Some(body) = body else {
            return;
        };
        let slugs = parse(body);
        for slug in &slugs {
            self.incoming
                .entry(slug.clone())
                .or_default()
                .insert(item.clone());
        }
        self.items
            .entry(item.slug())
            .or_default()
            .insert(item.clone());
        self.outgoing.insert(item, slugs);
    }

    fn remove(&mut self, item: &ItemId) {
        if let Some(slugs) = self.outgoing.remove(item) {
            for slug in slugs {
                remove_from(&mut self.incoming, &slug, item);
            }
        }
        remove_from(&mut self.items, &item.slug(), item);
    }

    pub(crate) fn resolve(&self, slug: &str) -> LinkTarget {
        let Some(candidates) = self.items.get(slug) else {
            return LinkTarget::Broken;
        };
        match candidates.iter().min_by_key(|c| c.rank()) {
            Some(item) => LinkTarget::Found {
                item: item.clone(),
                ambiguous: candidates.len() > 1,
            },
            None => LinkTarget::Broken,
        }
    }

    pub(crate) fn links(&self, item: &ItemId) -> Vec<Link> {
        self.outgoing
            .get(item)
            .into_iter()
            .flatten()
            .map(|slug| Link {
                slug: slug.clone(),
                target: self.resolve(slug),
            })
            .collect()
    }

    /// The Links `body` makes, resolved here, for text that isn't an Item
    /// (a Template).
    pub(crate) fn links_in(&self, body: &str) -> Vec<Link> {
        parse(body)
            .into_iter()
            .map(|slug| Link {
                target: self.resolve(&slug),
                slug,
            })
            .collect()
    }

    /// Sources sorted the same way a duplicated slug is resolved. An Item's
    /// Links to itself aren't listed.
    pub(crate) fn inbound_links(&self, item: &ItemId) -> Vec<InboundLink> {
        let slug = item.slug();
        let LinkTarget::Found {
            item: resolved,
            ambiguous,
        } = self.resolve(&slug)
        else {
            return Vec::new();
        };
        if &resolved != item {
            return Vec::new();
        }
        let mut sources: Vec<InboundLink> = self
            .incoming
            .get(&slug)
            .into_iter()
            .flatten()
            .filter(|source| *source != item)
            .map(|source| InboundLink {
                source: source.clone(),
                ambiguous,
            })
            .collect();
        sources.sort_by(|a, b| (a.source.rank(), &a.source).cmp(&(b.source.rank(), &b.source)));
        sources
    }
}

fn remove_from(map: &mut BTreeMap<String, BTreeSet<ItemId>>, key: &str, item: &ItemId) {
    if let Some(set) = map.get_mut(key) {
        set.remove(item);
        if set.is_empty() {
            map.remove(key);
        }
    }
}

/// The distinct slugs `body` links to, in order. Links inside fenced code
/// blocks and inline code spans are examples, not Links.
pub(crate) fn parse(body: &str) -> Vec<String> {
    let mut slugs: Vec<String> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(marker) = fence_marker(trimmed) {
            match fence {
                None => fence = Some(marker),
                Some((c, n)) if marker.0 == c && marker.1 >= n => fence = None,
                Some(_) => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        for slug in parse_line(line) {
            if !slugs.contains(&slug) {
                slugs.push(slug);
            }
        }
    }
    slugs
}

/// A line opening or closing a fenced code block: three or more backticks or
/// tildes.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let c = line.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = line.chars().take_while(|x| *x == c).count();
    (n >= 3).then_some((c, n))
}

fn parse_line(line: &str) -> Vec<String> {
    let mut slugs = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find(['`', '[']) {
        let tail = &rest[i..];
        if tail.starts_with('`') {
            let ticks = tail.len() - tail.trim_start_matches('`').len();
            let after = &tail[ticks..];
            match find_closing_ticks(after, ticks) {
                Some(end) => rest = &after[end + ticks..],
                // An unclosed run is literal text.
                None => rest = after,
            }
        } else if let Some(inner) = tail.strip_prefix("[[") {
            match inner.find("]]") {
                Some(end) => {
                    if let Some(slug) = link_slug(&inner[..end]) {
                        slugs.push(slug);
                    }
                    rest = &inner[end + 2..];
                }
                None => break,
            }
        } else {
            rest = &tail[1..];
        }
    }
    slugs
}

/// Where a run of exactly `ticks` backticks starts in `text`.
fn find_closing_ticks(text: &str, ticks: usize) -> Option<usize> {
    let mut offset = 0;
    while let Some(i) = text[offset..].find('`') {
        let start = offset + i;
        let run = text[start..].len() - text[start..].trim_start_matches('`').len();
        if run == ticks {
            return Some(start);
        }
        offset = start + run;
    }
    None
}

/// The slug part of a Link's inner text, before any `|label` or `#heading`.
fn link_slug(inner: &str) -> Option<String> {
    let slug = inner.split(['|', '#']).next()?.trim();
    (!slug.is_empty() && !slug.contains('[')).then(|| slug.to_string())
}
