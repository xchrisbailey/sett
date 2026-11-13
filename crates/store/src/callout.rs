//! Finding a Task's first Callout in its body, as plain text for a Card.
//!
//! The editor draws a Callout from a quote whose first line is exactly
//! `[!TYPE]` (see `crates/markdown-editor/FORK.md`, "Callouts"). This is a
//! line scan that follows the same rule without parsing the whole body.

/// The store's own copy of the editor's `CalloutKind`, since the store can't
/// depend on the editor. A test in `crates/app` keeps the two in step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    pub fn marker(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }

    pub fn from_marker(marker: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.marker().eq_ignore_ascii_case(marker))
    }
}

/// A Callout's kind and its text, one entry per non-blank source line, with
/// Markdown markup turned into plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCallout {
    pub kind: CalloutKind,
    pub lines: Vec<String>,
}

impl TaskCallout {
    /// The first `limit` lines, and whether more follow them.
    pub fn shown(&self, limit: usize) -> (&[String], bool) {
        let shown = limit.min(self.lines.len());
        (&self.lines[..shown], self.lines.len() > shown)
    }
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
    let indent = indent(line);
    let rest = &line[indent..];
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
    Some(indent + marker + spaces.max(1))
}

/// Whether a line could start a block of its own, so it can't be a lazy
/// continuation of a paragraph.
fn starts_block(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_fence(line)
        || trimmed.starts_with('#')
        || list_marker_width(line).is_some()
        || ["---", "***", "___"]
            .iter()
            .any(|rule| trimmed.starts_with(rule))
}

/// The line after the first `>` and one optional space.
fn quote_content(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches(' ').strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// The first Callout in the body that has text, or `None`. Top level only
/// (not in a list item, another quote, or code), marker alone on its
/// line. An empty Callout is skipped.
///
/// Mirrors the editor's CommonMark import; `crates/app/tests/first_callout.rs`
/// checks that the two agree.
pub(crate) fn first_callout(body: &str) -> Option<TaskCallout> {
    let lines: Vec<&str> = body.lines().collect();
    let mut in_fence = false;
    let mut list_width: Option<usize> = None;
    // Whether the last line was paragraph text in a list item, which a
    // following unindented line continues lazily.
    let mut item_paragraph = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            item_paragraph = false;
            i += 1;
            continue;
        }
        let line_indent = indent(line);
        if in_fence {
            if is_fence(line) && line_indent < 4 {
                in_fence = false;
            }
            i += 1;
            continue;
        }
        if list_width.is_some_and(|width| line_indent >= width) {
            // Inside a list item. A fence there still hides its lines.
            item_paragraph = !is_fence(line);
            if is_fence(line) {
                in_fence = true;
            }
            i += 1;
            continue;
        }
        // A quote interrupts a paragraph, so it is never a lazy line.
        let lazy = list_width.is_some()
            && item_paragraph
            && !starts_block(line)
            && quote_content(line).is_none();
        if lazy {
            i += 1;
            continue;
        }
        list_width = list_marker_width(line);
        if let Some(width) = list_width {
            item_paragraph = line.len() > width && !line[width..].trim().is_empty();
            i += 1;
            continue;
        }
        if line_indent >= 4 {
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
            .and_then(CalloutKind::from_marker);
        let mut quote_lines = Vec::new();
        let mut last_text = !first.trim().is_empty();
        i += 1;
        while i < lines.len() {
            let next = lines[i];
            if let Some(content) = quote_content(next).filter(|_| indent(next) < 4) {
                last_text = !content.trim().is_empty();
                quote_lines.push(content);
            } else if last_text && !next.trim().is_empty() && !starts_block(next) {
                quote_lines.push(next);
            } else {
                break;
            }
            i += 1;
        }
        if let Some(kind) = marker {
            let lines = plain_lines(&quote_lines);
            if !lines.is_empty() {
                return Some(TaskCallout { kind, lines });
            }
        }
    }
    None
}

fn plain_lines(quote_lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in quote_lines {
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
        let line_indent = indent(line);
        let rest = &line[line_indent..];
        let (prefix, text) = match ["- ", "* ", "+ "]
            .iter()
            .find_map(|bullet| rest.strip_prefix(bullet))
        {
            Some(item) => {
                let (glyph, item) = if let Some(todo) = item.strip_prefix("[ ]") {
                    ("☐ ", todo)
                } else if let Some(done) = item.strip_prefix("[x]").or(item.strip_prefix("[X]")) {
                    ("☑ ", done)
                } else {
                    ("• ", item)
                };
                (
                    format!("{}{glyph}", &line[..line_indent]),
                    item.trim_start(),
                )
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

/// A char of inline output, and whether it is a `*` that may turn out to be
/// emphasis. Which `*` are emphasis is only known once the whole line is read.
struct Out {
    ch: char,
    star: bool,
}

/// Inline Markdown to plain text.
fn inline_plain(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Out> = Vec::new();
    let literal = |out: &mut Vec<Out>, ch: char| out.push(Out { ch, star: false });
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        match c {
            // Only ASCII punctuation can be escaped; `\s` keeps its backslash.
            '\\' if chars.get(at + 1).is_some_and(char::is_ascii_punctuation) => {
                literal(&mut out, chars[at + 1]);
                at += 2;
            }
            '`' => {
                // A code span is literal: only its backticks go. One that
                // never closes is just backticks.
                let ticks = chars[at..].iter().take_while(|&&c| c == '`').count();
                match code_span_end(&chars, at + ticks, ticks) {
                    Some(close) => {
                        for &ch in &chars[at + ticks..close] {
                            literal(&mut out, ch);
                        }
                        at = close + ticks;
                    }
                    None => {
                        for _ in 0..ticks {
                            literal(&mut out, '`');
                        }
                        at += ticks;
                    }
                }
            }
            '[' if chars.get(at + 1) == Some(&'[') => {
                let close = (at + 2..chars.len().saturating_sub(1))
                    .find(|&j| chars[j] == ']' && chars[j + 1] == ']');
                match close {
                    Some(close) => {
                        let inner: String = chars[at + 2..close].iter().collect();
                        let shown = match inner.split_once('|') {
                            Some((_, label)) => label,
                            None => inner.split('#').next().unwrap_or(&inner),
                        };
                        shown.trim().chars().for_each(|ch| literal(&mut out, ch));
                        at = close + 2;
                    }
                    None => {
                        literal(&mut out, c);
                        at += 1;
                    }
                }
            }
            '[' => match link_end(&chars, at) {
                Some((text_end, after)) => {
                    let label: String = chars[at + 1..text_end].iter().collect();
                    inline_plain(&label)
                        .chars()
                        .for_each(|ch| literal(&mut out, ch));
                    at = after;
                }
                None => {
                    literal(&mut out, c);
                    at += 1;
                }
            },
            '*' => {
                out.push(Out { ch: c, star: true });
                at += 1;
            }
            '_' => {
                let word = |k: Option<&char>| k.is_some_and(|c| c.is_alphanumeric());
                let inside_word = at > 0 && word(chars.get(at - 1)) && word(chars.get(at + 1));
                if inside_word {
                    literal(&mut out, c);
                }
                at += 1;
            }
            _ => {
                literal(&mut out, c);
                at += 1;
            }
        }
    }
    drop_emphasis_stars(&mut out);
    out.into_iter().map(|out| out.ch).collect()
}

/// For a code span whose opening run of `ticks` backticks ends just before
/// `from`: the index of the closing run, which has to be as long as the
/// opening one.
fn code_span_end(chars: &[char], from: usize, ticks: usize) -> Option<usize> {
    let mut at = from;
    while at < chars.len() {
        if chars[at] != '`' {
            at += 1;
            continue;
        }
        let run = chars[at..].iter().take_while(|&&c| c == '`').count();
        if run == ticks {
            return Some(at);
        }
        at += run;
    }
    None
}

/// Removes the `*` that open and close emphasis, and keeps the rest (`2 * 3`,
/// `**kwargs`). A run of stars opens when text follows it directly and closes
/// when text precedes it directly (CommonMark's flanking, without the
/// punctuation cases); it closes the nearest open run before it.
fn drop_emphasis_stars(out: &mut Vec<Out>) {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut at = 0;
    while at < out.len() {
        if out[at].star {
            let start = at;
            while at < out.len() && out[at].star {
                at += 1;
            }
            runs.push((start, at));
        } else {
            at += 1;
        }
    }
    let spaced = |at: Option<usize>| {
        at.and_then(|at| out.get(at))
            .is_none_or(|o| o.ch.is_whitespace())
    };
    let mut open: Vec<(usize, usize)> = Vec::new();
    let mut dropped = Vec::new();
    for (start, end) in runs {
        let can_close = !spaced(start.checked_sub(1));
        let can_open = !spaced(Some(end));
        if can_close && let Some(opener) = open.pop() {
            dropped.push(opener);
            dropped.push((start, end));
        } else if can_open {
            open.push((start, end));
        }
    }
    let mut at = 0;
    out.retain(|_| {
        let drop = dropped
            .iter()
            .any(|&(start, end)| (start..end).contains(&at));
        at += 1;
        !drop
    });
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
