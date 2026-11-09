# Markdown files are the source of truth

Every Note, Task and Journal Entry is its own Markdown file with YAML frontmatter, stored in a plain folder tree (`<root>/<space>/...`). SQLite, if used, is only a cache that can be rebuilt. We chose this over SQLite or JSON as the source of truth because outside AI agents (Claude Code, OpenCode, etc.) need to read and edit the data directly with ordinary file tools. LLMs write Markdown reliably and spend fewer tokens on it, while long text inside JSON strings is easy for them to corrupt.

## Consequences

- sett must watch the data folder and reconcile changes made outside the app.
- Structured fields live only in frontmatter, so sett has to tolerate frontmatter that is hand-edited or missing.
- One file per Task means moving a card changes one line in one file, which keeps git diffs clean and avoids conflicting writes to a shared board file.
