use std::path::Path;

pub(crate) fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug.to_string()
    }
}

/// The first of `base`, `base-2`, `base-3`, ... not already taken in `dir`,
/// where `taken` maps a candidate slug to the path it would occupy.
pub(crate) fn unique_slug(dir: &Path, base: &str, taken: impl Fn(&Path, &str) -> bool) -> String {
    if !taken(dir, base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken(dir, candidate))
        .expect("an unbounded range always yields a free slug")
}

/// Slugs come from file names that callers can hand us, so reject anything
/// that could escape its folder.
pub(crate) fn is_safe(slug: &str) -> bool {
    !slug.is_empty() && !slug.starts_with('.') && !slug.contains(['/', '\\', '\0'])
}
