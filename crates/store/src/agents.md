# sett data

This folder is managed by sett, a local-first app for Notes, Tasks and Journal
Entries. Every Item is a plain Markdown file with YAML frontmatter, so you can
read and edit it with ordinary file tools.

## Layout

```
AGENTS.md
personal/ | work/            # Spaces: fully isolated, never link across them
  notes/<slug>.md
  journal/YYYY-MM-DD.md
  projects/<slug>/
    project.md               # name, icon (Lucide name), columns (ordered), archived
    tasks/<slug>.md
    notes/<slug>.md
  templates/<slug>.md        # Templates: starter bodies, not Items
```

`personal/` and `work/` are separate Spaces. Nothing in one may reference the
other.

## Frontmatter

Task:

```yaml
title: Fix login bug
column: Doing        # must match one of the Project's columns
order: 1.5           # position within the column; lower comes first
created: 2026-01-31
due: 2026-02-14      # optional
done: 2026-02-10     # set when the Task enters the last column
labels: [bug, Urgent] # optional
```

`labels` is a YAML list of short free-text Labels. Labels are compared ignoring
case, so `Bug` and `bug` are the same Label. A Label exists while some Task has
it: there is no Label list to maintain, and removing a Label from every Task
removes it. Leave the key out when a Task has none.

Note:

```yaml
title: Meeting notes
created: 2026-01-31
```

Template: only a title is needed.

```yaml
title: Meeting notes
```

Journal Entry: no required frontmatter. The date comes from the filename.

Project (`project.md`):

```yaml
name: Inbox
icon: inbox
columns: [Todo, Doing, Done]
archived: false
```

Dates are always `YYYY-MM-DD`. The body after the frontmatter is any Markdown.

## Rules

- The last column means done. The first column means not started; every column
  between means in progress. To finish a Task, move it to the last column and
  set `done` to today's date.
- Link to another Item in the same Space with `[[slug]]`, `[[slug|label]]` or
  `[[slug#heading]]`. Link a Journal Entry by its date, `[[YYYY-MM-DD]]`.
- Slugs (file and folder names) are stable. Never rename a file when its title
  changes, because Links point at the slug. The one exception is a Template,
  which may be renamed when its title changes. New slugs come from the title,
  lowercased with dashes. A Note or Task slug must be unique across the whole
  Space (every Project's Notes and Tasks, archived ones too, and Space-level
  Notes); on a clash add `-2`, `-3`, and so on.
- A Template is not an Item. Don't link to it. Its slug only has to be unique
  within `templates/`. Keep any other frontmatter keys on a Template, but they
  are never copied into new Items. To start a Task or Note from a Template,
  copy its body, not its frontmatter, into the new Item.
- Keep frontmatter keys you don't recognise. sett preserves them.
- The `inbox` Project can't be deleted or archived.
