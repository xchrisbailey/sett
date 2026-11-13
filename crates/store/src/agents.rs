//! Keeps the data root's `AGENTS.md` current. sett owns only the text between
//! the markers; anything else in the file belongs to the user.

use std::fs;
use std::io;
use std::path::Path;

const BODY: &str = include_str!("agents.md");
// Earlier releases wrote the file without markers. Matching them exactly is
// how we tell an untouched file (safe to replace) from one the user edited.
const SHIPPED: [&str; 2] = [include_str!("agents_v1.md"), include_str!("agents_v2.md")];
const BEGIN: &str = "<!-- sett:begin -->";
const END: &str = "<!-- sett:end -->";

fn block() -> String {
    format!("{BEGIN}\n{}\n{END}\n", BODY.trim_end())
}

/// What `AGENTS.md` should contain, or `None` when it is already right.
fn updated(existing: &str) -> Option<String> {
    // A CRLF file stays CRLF, or the block would leave it with mixed endings.
    let eol = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let current = block().replace('\n', eol);
    let next = match (existing.find(BEGIN), existing.find(END)) {
        (Some(start), Some(end)) if start < end => {
            let end = end + END.len();
            // Swallow the line break after the end marker, which the block
            // itself carries, so repeated runs don't grow blank lines.
            let tail = &existing[end..];
            let tail = tail
                .strip_prefix("\r\n")
                .or_else(|| tail.strip_prefix('\n'))
                .unwrap_or(tail);
            format!("{}{current}{tail}", &existing[..start])
        }
        _ if is_shipped(existing) => current,
        _ => {
            let mut text = existing.trim_end().to_string();
            if !text.is_empty() {
                text.push_str(eol);
                text.push_str(eol);
            }
            text + &current
        }
    };
    (next != existing).then_some(next)
}

fn is_shipped(text: &str) -> bool {
    let text = text.replace("\r\n", "\n");
    SHIPPED.contains(&text.as_str())
}

pub(crate) fn sync(root: &Path) -> io::Result<()> {
    let path = root.join("AGENTS.md");
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    match updated(&existing) {
        Some(text) => fs::write(path, text),
        None => Ok(()),
    }
}
