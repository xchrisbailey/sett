//! Finding a Task's first Callout in its body, as plain text for a Card.
//!
//! The editor draws a Callout from a quote whose first line is exactly
//! `[!TYPE]` (see `crates/markdown-editor/FORK.md`, "Callouts"). This is a
//! line scan that follows the same rule without parsing the whole body.

/// The type of a Callout. The store's own copy of the editor's `CalloutKind`,
/// since the store can't depend on the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalloutType {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutType {
    pub const ALL: [Self; 5] = [
        Self::Note,
        Self::Tip,
        Self::Important,
        Self::Warning,
        Self::Caution,
    ];

    /// The marker the type is written with (`NOTE`).
    pub fn marker(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }

    /// The type a marker names, in any case.
    pub fn from_marker(marker: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.marker().eq_ignore_ascii_case(marker))
    }
}

/// A Callout's type and its text, one entry per non-blank source line, with
/// Markdown markup turned into plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCallout {
    pub kind: CalloutType,
    pub lines: Vec<String>,
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_fence(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("```") || line.starts_with("~~~")
}

/// The width of a list marker at the start of `line` (indent, marker and the
/// spaces after it), or `None` when the line isn't a list item.
fn list_marker_width(line: &str) -> Option<usize> {
    let ind = indent(line);
    let rest = &line[ind..];
    let marker = if rest.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !matches!(rest.as_bytes().get(digits), Some(b'.' | b')')) {
            return None;
        }
        digits + 1
    };
    let after = &rest[marker..];
    if !(after.is_empty() || after.starts_with(' ')) {
        return None;
    }
    let spaces = after.len() - after.trim_start_matches(' ').len();
    Some(ind + marker + spaces.max(1))
}

/// Whether a line could start a block of its own, so it can't be a lazy
/// continuation of a paragraph.
fn starts_block(line: &str) -> bool {
    let t = line.trim_start();
    is_fence(line)
        || t.starts_with('#')
        || list_marker_width(line).is_some()
        || ["---", "***", "___"].iter().any(|r| t.starts_with(r))
}

/// The line after the first `>` and one optional space.
fn quote_content(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches(' ').strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// The first Callout in the body that has text, or `None`. Top level only
/// (not in a list item, another quote, or code), marker alone on its
/// line. An empty Callout is skipped.
pub(crate) fn first_callout(body: &str) -> Option<TaskCallout> {
    let lines: Vec<&str> = body.lines().collect();
    let mut in_fence = false;
    let mut list_width: Option<usize> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let ind = indent(line);
        if in_fence {
            if is_fence(line) && ind < 4 {
                in_fence = false;
            }
            i += 1;
            continue;
        }
        if list_width.is_some_and(|w| ind >= w) {
            // Inside a list item. A fence there still hides its lines.
            if is_fence(line) {
                in_fence = true;
            }
            i += 1;
            continue;
        }
        list_width = list_marker_width(line);
        if list_width.is_some() {
            i += 1;
            continue;
        }
        if ind >= 4 {
            i += 1;
            continue;
        }
        if is_fence(line) {
            in_fence = true;
            i += 1;
            continue;
        }
        let Some(first) = quote_content(line) else {
            i += 1;
            continue;
        };
        // A quote: gather it, then see whether it is a Callout.
        let marker = first
            .trim_matches([' ', '\t'])
            .strip_prefix("[!")
            .and_then(|m| m.strip_suffix(']'))
            .and_then(CalloutType::from_marker);
        let mut body = Vec::new();
        let mut last_text = !first.trim().is_empty();
        i += 1;
        while i < lines.len() {
            let l = lines[i];
            if let Some(content) = quote_content(l).filter(|_| indent(l) < 4) {
                last_text = !content.trim().is_empty();
                body.push(content);
            } else if last_text && !l.trim().is_empty() && !starts_block(l) {
                body.push(l);
            } else {
                break;
            }
            i += 1;
        }
        if let Some(kind) = marker {
            let lines = plain_lines(&body);
            if !lines.is_empty() {
                return Some(TaskCallout { kind, lines });
            }
        }
    }
    None
}

/// A Callout's body as plain text lines.
fn plain_lines(body: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in body {
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if in_fence {
            out.push(line.to_string());
            continue;
        }
        let ind = indent(line);
        let rest = &line[ind..];
        let (prefix, text) = match ["- ", "* ", "+ "].iter().find_map(|b| rest.strip_prefix(b)) {
            Some(item) => {
                let (glyph, item) = if let Some(t) = item.strip_prefix("[ ]") {
                    ("☐ ", t)
                } else if let Some(t) = item.strip_prefix("[x]").or(item.strip_prefix("[X]")) {
                    ("☑ ", t)
                } else {
                    ("• ", item)
                };
                (format!("{}{glyph}", &line[..ind]), item.trim_start())
            }
            None => (String::new(), rest),
        };
        let plain = format!("{prefix}{}", inline_plain(text));
        if !plain.trim().is_empty() {
            out.push(plain);
        }
    }
    out
}

/// Inline Markdown to plain text.
fn inline_plain(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() => {
                out.push(chars[i + 1]);
                i += 2;
            }
            '[' if chars.get(i + 1) == Some(&'[') => {
                let close = (i + 2..chars.len().saturating_sub(1))
                    .find(|&j| chars[j] == ']' && chars[j + 1] == ']');
                match close {
                    Some(j) => {
                        let inner: String = chars[i + 2..j].iter().collect();
                        let shown = match inner.split_once('|') {
                            Some((_, label)) => label,
                            None => inner.split('#').next().unwrap_or(&inner),
                        };
                        out.push_str(shown.trim());
                        i = j + 2;
                    }
                    None => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            '[' => match link_end(&chars, i) {
                Some((text_end, after)) => {
                    let label: String = chars[i + 1..text_end].iter().collect();
                    out.push_str(&inline_plain(&label));
                    i = after;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            '*' | '`' => i += 1,
            '_' => {
                let word = |k: Option<&char>| k.is_some_and(|c| c.is_alphanumeric());
                let inside_word = i > 0 && word(chars.get(i - 1)) && word(chars.get(i + 1));
                if inside_word {
                    out.push(c);
                }
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// For `[text](url)` starting at `open`: the index of `]` and the index just
/// after `)`.
fn link_end(chars: &[char], open: usize) -> Option<(usize, usize)> {
    let mut depth = 0;
    let close = (open..chars.len()).find(|&j| match chars[j] {
        '\\' => false,
        '[' => {
            depth += 1;
            false
        }
        ']' => {
            depth -= 1;
            depth == 0
        }
        _ => false,
    })?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = (close + 2..chars.len()).find(|&j| chars[j] == ')')?;
    Some((close, end + 1))
}
