use std::io;

use crate::{Date, Project, Store, Task, invalid, not_found};

/// The fewest Columns a Board can have: a start and a done Column.
pub const MIN_COLUMNS: usize = 2;

/// A Project's Tasks laid out in its Columns, in Board order. A Task whose
/// `column` is missing or names no Column (a hand edit, or a Column renamed
/// outside sett) shows at the end of the first Column so it can't vanish.
#[derive(Debug)]
pub struct Board<'a> {
    names: &'a [String],
    columns: Vec<BoardColumn<'a>>,
}

#[derive(Debug)]
pub struct BoardColumn<'a> {
    name: &'a str,
    tasks: Vec<&'a Task>,
}

impl<'a> Board<'a> {
    pub fn new(columns: &'a [String], tasks: impl IntoIterator<Item = &'a Task>) -> Board<'a> {
        let mut tasks: Vec<&Task> = tasks.into_iter().collect();
        tasks.sort_by(|a, b| crate::task_order(a, b));
        let mut board = Board {
            names: columns,
            columns: columns
                .iter()
                .map(|name| BoardColumn {
                    name,
                    tasks: Vec::new(),
                })
                .collect(),
        };
        let mut strays = Vec::new();
        for task in tasks {
            let column = task
                .column
                .as_deref()
                .and_then(|c| board.columns.iter_mut().find(|col| col.name == c));
            match column {
                Some(column) => column.tasks.push(task),
                None => strays.push(task),
            }
        }
        if let Some(first) = board.columns.first_mut() {
            first.tasks.extend(strays);
        }
        board
    }

    pub fn columns(&self) -> &[BoardColumn<'a>] {
        &self.columns
    }

    pub fn column(&self, name: &str) -> Option<&BoardColumn<'a>> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// A Column can be deleted only when it shows no Tasks and the Board
    /// keeps at least [`MIN_COLUMNS`]. Blocking a non-empty Column means a
    /// delete never has to decide where its Tasks go.
    pub fn is_column_deletable(&self, name: &str) -> bool {
        self.columns.len() > MIN_COLUMNS && self.column(name).is_some_and(|c| c.tasks.is_empty())
    }

    pub fn column_names(&self) -> &'a [String] {
        self.names
    }

    pub fn task(&self, slug: &str) -> Option<&'a Task> {
        self.columns
            .iter()
            .flat_map(|c| c.tasks.iter().copied())
            .find(|t| t.slug == slug)
    }

    /// The Tasks to write so `slug` sits in Column `to`, before the Task
    /// `before` or at the end, each with its new `order`. Normally that is
    /// only the moved Task, ordered halfway between its new neighbours. When
    /// there is no room left between them (or a neighbour has no `order`),
    /// the whole Column is renumbered `1, 2, 3, …`, skipping Tasks whose order
    /// already matches. Empty when the Task wouldn't move. `None` when the
    /// Task or Column doesn't exist.
    pub(crate) fn plan_move(
        &self,
        slug: &str,
        to: &str,
        before: Option<&str>,
    ) -> Option<Vec<(&'a Task, f64)>> {
        let task = self.task(slug)?;
        let target = self.column(to)?;
        if before == Some(slug) {
            return Some(Vec::new());
        }
        let current_ix = target.tasks.iter().position(|t| t.slug == slug);
        let others: Vec<&Task> = target
            .tasks
            .iter()
            .copied()
            .filter(|t| t.slug != slug)
            .collect();
        let ix = before
            .and_then(|b| others.iter().position(|t| t.slug == b))
            .unwrap_or(others.len());
        if current_ix == Some(ix) && task.column.as_deref() == Some(to) {
            return Some(Vec::new());
        }

        let prev = ix.checked_sub(1).map(|i| others[i]);
        if let Some(order) = order_between(prev, others.get(ix).copied()) {
            return Some(vec![(task, order)]);
        }
        let mut sequence = others;
        sequence.insert(ix, task);
        Some(
            sequence
                .into_iter()
                .zip(1..)
                .map(|(t, n)| (t, f64::from(n)))
                .filter(|(t, order)| {
                    t.slug == slug || t.order != Some(*order) || t.column.as_deref() != Some(to)
                })
                .collect(),
        )
    }
}

impl<'a> BoardColumn<'a> {
    pub fn name(&self) -> &'a str {
        self.name
    }

    pub fn tasks(&self) -> &[&'a Task] {
        &self.tasks
    }
}

/// An `order` strictly between the neighbours, or `None` when there's no room.
fn order_between(prev: Option<&Task>, next: Option<&Task>) -> Option<f64> {
    let order = match (prev.map(|t| t.order), next.map(|t| t.order)) {
        (None, None) => 1.0,
        (Some(Some(p)), None) => p + 1.0,
        (None, Some(Some(n))) => n - 1.0,
        (Some(Some(p)), Some(Some(n))) => p + (n - p) / 2.0,
        _ => return None,
    };
    let above = prev.and_then(|t| t.order).is_none_or(|p| order > p);
    let below = next.and_then(|t| t.order).is_none_or(|n| order < n);
    (order.is_finite() && above && below).then_some(order)
}

/// Column edits. Tasks name their Column by its name, so renaming a Column
/// rewrites the Tasks in it; and Task State follows from position, so a
/// change to which Column is last updates `done` on the Tasks it affects.
impl Store {
    /// Adds a Column just before the last one, so the done Column stays last.
    pub fn add_column(&self, project: &mut Project, name: &str) -> io::Result<()> {
        let name = column_name(&project.columns, name, None)?;
        let mut updated = project.clone();
        let at = updated.columns.len().saturating_sub(1);
        updated.columns.insert(at, name);
        self.save_project(&mut updated)?;
        *project = updated;
        Ok(())
    }

    /// Renames Column `from` and rewrites each Task in it to the new name.
    pub fn rename_column<'t>(
        &self,
        project: &mut Project,
        tasks: impl IntoIterator<Item = &'t Task>,
        from: &str,
        to: &str,
    ) -> io::Result<()> {
        let to = column_name(&project.columns, to, Some(from))?;
        let mut updated = project.clone();
        let Some(column) = updated.columns.iter_mut().find(|c| *c == from) else {
            return Err(not_found(format!("column {from}")));
        };
        *column = to.clone();
        self.save_project(&mut updated)?;
        *project = updated;
        for task in tasks {
            if task.column.as_deref() == Some(from) {
                let mut task = task.clone();
                task.column = Some(to.clone());
                self.save_task(&mut task)?;
            }
        }
        Ok(())
    }

    /// Moves the Column at `from` to position `to` (both zero-based).
    pub fn move_column<'t>(
        &self,
        project: &mut Project,
        tasks: impl IntoIterator<Item = &'t Task>,
        from: usize,
        to: usize,
        today: Date,
    ) -> io::Result<()> {
        let len = project.columns.len();
        if from >= len || to >= len {
            return Err(invalid(format!("no Column at {from} or {to}")));
        }
        let mut columns = project.columns.clone();
        let column = columns.remove(from);
        columns.insert(to, column);
        self.save_columns(project, columns, tasks, today)
    }

    /// Deletes Column `name` if [`Board::is_column_deletable`] allows it.
    pub fn delete_column<'t>(
        &self,
        project: &mut Project,
        tasks: impl IntoIterator<Item = &'t Task>,
        name: &str,
        today: Date,
    ) -> io::Result<()> {
        let tasks: Vec<&Task> = tasks.into_iter().collect();
        if !Board::new(&project.columns, tasks.iter().copied()).is_column_deletable(name) {
            return Err(invalid(format!(
                "Column {name} still has Tasks, or is one of the last {MIN_COLUMNS}"
            )));
        }
        let columns = project
            .columns
            .iter()
            .filter(|c| *c != name)
            .cloned()
            .collect();
        self.save_columns(project, columns, tasks, today)
    }

    /// Saves the Project with `columns`, then updates `done` on the Tasks in
    /// the Columns that stopped or started being last.
    fn save_columns<'t>(
        &self,
        project: &mut Project,
        columns: Vec<String>,
        tasks: impl IntoIterator<Item = &'t Task>,
        today: Date,
    ) -> io::Result<()> {
        let old_last = project.columns.last().cloned();
        let new_last = columns.last().cloned();
        let mut updated = project.clone();
        updated.columns = columns;
        self.save_project(&mut updated)?;
        *project = updated;
        if old_last == new_last {
            return Ok(());
        }
        for task in tasks {
            if task.column != old_last && task.column != new_last {
                continue;
            }
            let mut task = task.clone();
            if task.sync_done(&project.columns, today) {
                self.save_task(&mut task)?;
            }
        }
        Ok(())
    }
}

/// A trimmed, non-empty name no other Column has (ignoring case). `current`
/// is the Column being renamed, which may keep its own name.
fn column_name(columns: &[String], name: &str, current: Option<&str>) -> io::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("a Column needs a name"));
    }
    let taken = columns
        .iter()
        .filter(|c| Some(c.as_str()) != current)
        .any(|c| c.to_lowercase() == name.to_lowercase());
    if taken {
        return Err(invalid(format!("there's already a Column named {name}")));
    }
    Ok(name.to_string())
}
