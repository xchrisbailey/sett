---
status: accepted
---

# Block editing uses a fork of gpui-kit-markdown-editor

Items are edited in a Notion-style block view from `gpui-kit-markdown-editor`, with gpui-kit's `Editor` as a raw Markdown mode that one global toggle switches to. We chose this over writing our own rich-text engine (months of work, and the GPUI ecosystem already has several) and over a homemade click-to-edit block view (less like Notion, and no selection across blocks).

The spike (#2) found that the published 0.2.0 doesn't build against gpui-kit 0.9.1 and changed files lossily on save: wiki-links were escaped, lines were joined and an extra newline was added. We keep a patched copy in `crates/markdown-editor/`. `FORK.md` lists every patch to upstream and the normalisations that remain. Hands-on macOS checks passed: typing, IME, undo, selection across blocks and long Notes.

## Consequences

- sett writes an Item's file only after the user has actually edited it. Opening or viewing an Item never saves it, because the editor still normalises some Markdown on save.
- `[[wiki-links]]` show as plain text in the block view. Making them clickable is a later upstream contribution.
- We maintain the fork until our patches are merged upstream.
