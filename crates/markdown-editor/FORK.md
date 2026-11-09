# Fork of gpui-kit-markdown-editor

- Upstream: https://github.com/wabzqem/gpui-kit-markdown-editor
- Base version: 0.2.0 (crates.io)
- License: Apache-2.0 (see `LICENSE-APACHE`)
- Package name `gpui-kit-markdown-editor` and lib name `markdown_editor` are unchanged.

Every patched spot is marked with a `FORK` comment. `tests/round_trip.rs` and `tests/fixtures/` cover what we rely on.

## Patches we carry

Each line is a candidate for upstreaming.

- **gpui-kit 0.9.1 build fix** (`direct.rs`): sets `task_marker: None` on `BlockNode::ListItem`. Without it the crate doesn't compile against gpui-kit ^0.9.1.
- **Final newline** (`import.rs`, `document.rs`, `serialize.rs`): an odd trailing line ending is kept as trivia (`EditorDocument::final_line_ending`) and no longer becomes an empty paragraph. Before, every save added a `\n`.
- **Wiki-links** (`markdown_source.rs`): `[[target]]`, `[[target|label]]` and `[[target#heading]]` are written unescaped, when their inner text needs no escaping. In table cells the `|` is still escaped. Before, they were saved as `\[\[…\]\]`, which breaks the link for other tools.
- **Soft line breaks** (`import.rs`, `serialize.rs`, `command.rs`, `state.rs`): gpui-kit's parser folds soft breaks into spaces. A parse-only inline plugin claims text nodes that contain line endings, and the importer turns each break into an `editor-soft-break` atom. The atom shows as a space and saves as the original line ending. The editor's own block copy and paste keeps the breaks, an inline copy writes them as spaces, and external pastes are unchanged. Before, hard-wrapped paragraphs, quotes and list items were joined into one line.
- **List markers** (`kind.rs`, `import.rs`, `serialize.rs`, `transaction.rs`, `command.rs`): `BlockKindData::List` records its bullet (`-` `*` `+`) or ordered delimiter (`.` `)`). Adjacent lists merge only when their markers match, because in Markdown they are separate lists. Before, every list was written with `-` or `1.`.
- **Callouts** (`markdown_source.rs`): a `[!TYPE]` marker at the start of a paragraph is written unescaped (`> [!NOTE]`). Before, it was saved as `\[!NOTE\]`.
- **Table delimiter row** (`kind.rs`, `import.rs`, `serialize.rs`, `table.rs`): the source's delimiter row (`|------|`) is kept for as long as it still matches the columns and their alignment. Before, it was always rewritten to `| --- |`.
- **Our build only, not for upstream** (`Cargo.toml`, `tests/mod.rs`, `component.rs`, `state.rs`, `projection.rs`): the GPUI view tests are gated behind a `view-tests` feature, and the `gpui-kit/test-support` dev-dependency is dropped because it pulls in `gpui-fast`, whose pins don't resolve in our workspace. The crate uses the workspace `gpui-kit`, and upstream's `Cargo.lock` is removed.

## Known normalisations

A file using any of these is rewritten on its first save; after that, saves are stable.

- `__bold__` / `_italic_` → `**bold**` / `*italic*`. The delimiter would have to live on gpui-kit's `TextMark`.
- Hard breaks (two trailing spaces, `\` + newline) → `<br>` with the lines joined.
- `$` in text → `\$`. Escaping is decided per text run, but a math span can cross marks.
- Setext headings → ATX, and closing `##` are dropped.
- `~~~` fences → backticks; indented code → fenced code.
- Table cell padding → `| a | b |`. The delimiter row is kept, as above.
- A URL inside `( )` → `<url>`.
- Trailing spaces before a soft break are dropped. CRLF line endings between blocks become LF.
- Unescaped `[[…]]` or `[!X]` text could become a link on reparse if a matching `[…]: url` definition exists. This is unlikely in practice.
