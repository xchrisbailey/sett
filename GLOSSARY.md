# sett

A local-first personal management app for solo developers, built so AI agents can read and work on the same data the user does.

## Language

**Item**:
Any Note, Task or Journal Entry. Items are the things the user and agents create, read and link to.
_Avoid_: Entry, document, doc, file, record

**Space**:
One of two fully isolated areas of the user's life: Personal or Work. Nothing in one Space can see or reference the other.
_Avoid_: Context, workspace, mode, profile

**Project**:
A named body of work inside a Space, with its own board of Tasks and its own Notes.
_Avoid_: Side project, repo, folder

**Note**:
A freeform document written by the user or an agent.
_Avoid_: Page, doc, file

**Inbox**:
The built-in Project in each Space that holds Tasks not yet sorted into another Project. It can't be deleted or archived.
_Avoid_: Unsorted, backlog, triage

**Task**:
A unit of work belonging to one Project, sitting in exactly one Column of that Project's Board. Its body can hold any content a Note can.
_Avoid_: Todo, ticket, issue

**Card**:
A Task's compact summary on a Board. A Card is a view of a Task, never a separate thing.
_Avoid_: Tile, ticket

**Board**:
The kanban view of one Project's Tasks, arranged in Columns. Each Project has exactly one Board.
_Avoid_: Kanban, list

**Column**:
A user-named, user-ordered stage on a Board.
_Avoid_: Lane, row, status, stage

**Task State**:
Whether a Task is not started, in progress or done. It follows from the Task's Column's position: the first Column is not started, the last is done, and every Column between is in progress.
_Avoid_: Status

**Link**:
A reference from one item's body to another item in the same Space.
_Avoid_: Reference, mention, backlink

**Link Picker**:
The list of Items that opens when the user types `[[` in an Item's body. Choosing one inserts a Link to it.
_Avoid_: Mention, autocomplete, quick switcher

**Callout**:
A highlighted box in an Item's body that marks its content as a note, tip, important point, warning or caution.
_Avoid_: Alert, admonition, surface

**Code Block**:
A block in an Item's body that holds code exactly as written, optionally tagged with the language it is written in.
_Avoid_: Snippet, fence, pre

**Language Menu**:
The list of languages that opens from a Code Block's language label. Choosing one sets the language the block is tagged with.
_Avoid_: Language picker, language dropdown

**Insert Menu**:
The list of block types that opens when the user types `/` at the start of an empty block in an Item's body. Choosing one turns that block into the chosen type.
_Avoid_: Slash menu, command palette, block picker

**Journal Entry**:
A Note tied to a single calendar date within a Space. Each Space keeps its own Journal.
_Avoid_: Daily note, log, diary
