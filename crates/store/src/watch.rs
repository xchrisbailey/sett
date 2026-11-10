use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher as _};

use crate::{Change, Date, Space};

/// How long to gather raw events before reporting, so one save that fires
/// several events reaches the caller once. Well under the 1 s target.
const DEBOUNCE: Duration = Duration::from_millis(50);

/// What sett itself last did at a path, relative to the data root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    File(u64),
    Dir,
    /// Being moved to the Trash. Trashing can be slow, so until it's gone the
    /// path may still exist; `was` is the file's content hash at that moment
    /// (`None` for a folder), so a file an agent puts back is still reported.
    Removed {
        was: Option<u64>,
    },
}

/// sett's own writes, so the watcher can tell them from outside changes.
///
/// Matching on what is on disk now (rather than on timing) means duplicate or
/// late events for sett's write are dropped, while an agent editing the same
/// file afterwards is still reported because its content no longer matches.
#[derive(Debug, Default, Clone)]
pub(crate) struct WriteLog(Arc<Mutex<HashMap<PathBuf, Mark>>>);

impl WriteLog {
    fn lock(&self) -> MutexGuard<'_, HashMap<PathBuf, Mark>> {
        // A panic mid-insert can't leave the map inconsistent, so keep going.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn file(&self, rel: PathBuf, content: &[u8]) {
        self.lock().insert(rel, Mark::File(hash(content)));
    }

    pub(crate) fn dir(&self, rel: PathBuf) {
        self.lock().insert(rel, Mark::Dir);
    }

    pub(crate) fn removed(&self, rel: PathBuf, path: &Path) {
        let was = path
            .is_file()
            .then(|| fs::read(path).ok().map(|c| hash(&c)))
            .flatten();
        self.lock().insert(rel, Mark::Removed { was });
    }

    /// Whether disk at `rel` is exactly what sett last left there. Anything
    /// else is an outside change, and its mark is cleared so that a later
    /// outside change back to sett's state is reported too.
    fn is_own(&self, root: &Path, rel: &Path) -> bool {
        let mut log = self.lock();
        let path = root.join(rel);
        let own = match fs::symlink_metadata(&path) {
            Ok(m) if m.is_file() => match log.get(rel) {
                Some(Mark::File(h) | Mark::Removed { was: Some(h) }) => {
                    fs::read(&path).is_ok_and(|c| hash(&c) == *h)
                }
                _ => false,
            },
            Ok(m) if m.is_dir() => {
                matches!(log.get(rel), Some(Mark::Dir | Mark::Removed { was: None }))
            }
            Ok(_) => false,
            Err(_) => rel
                .ancestors()
                .any(|a| matches!(log.get(a), Some(Mark::Removed { .. }))),
        };
        if !own {
            log.remove(rel);
        }
        own
    }
}

fn hash(content: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    h.write(content);
    h.finish()
}

/// Watches one Space's folder and reports outside changes. Dropping it stops
/// watching and closes the channel.
pub struct Watcher {
    _inner: notify::RecommendedWatcher,
    events: async_channel::Receiver<Change>,
}

impl Watcher {
    pub(crate) fn start(root: &Path, space: Space, log: WriteLog) -> io::Result<Watcher> {
        // FSEvents reports resolved paths (`/private/var/...` for `/var/...`),
        // so compare against the resolved root.
        let root = fs::canonicalize(root)?;
        let space_dir = root.join(space.dir_name());
        let (raw_tx, raw_rx) = mpsc::channel();
        let mut inner = notify::recommended_watcher(move |res| {
            let _ = raw_tx.send(res);
        })
        .map_err(io::Error::other)?;
        inner
            .watch(&space_dir, RecursiveMode::Recursive)
            .map_err(io::Error::other)?;

        let (tx, events) = async_channel::unbounded();
        thread::Builder::new()
            .name(format!("sett-watch-{}", space.dir_name()))
            .spawn(move || forward(raw_rx, tx, &root, space, &log))?;
        Ok(Watcher {
            _inner: inner,
            events,
        })
    }

    /// Outside changes in this Space, in the order they happened. Feed each
    /// one to `SpaceIndex::apply`.
    pub fn events(&self) -> async_channel::Receiver<Change> {
        self.events.clone()
    }
}

type RawEvent = notify::Result<notify::Event>;

fn forward(
    raw: mpsc::Receiver<RawEvent>,
    tx: async_channel::Sender<Change>,
    root: &Path,
    space: Space,
    log: &WriteLog,
) {
    // Ends when the notify watcher is dropped, which drops its sender.
    while let Ok(first) = raw.recv() {
        let mut batch = vec![first];
        let deadline = Instant::now() + DEBOUNCE;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match raw.recv_timeout(left) {
                Ok(ev) => batch.push(ev),
                Err(_) => break,
            }
        }

        let mut changes: Vec<Change> = Vec::new();
        let mut push = |c: Change| {
            if !changes.contains(&c) {
                changes.push(c);
            }
        };
        for ev in batch {
            match ev {
                Ok(ev) if !ev.need_rescan() => {
                    for path in ev.paths {
                        let Ok(rel) = path.strip_prefix(root) else {
                            continue;
                        };
                        let Ok(in_space) = rel.strip_prefix(space.dir_name()) else {
                            continue;
                        };
                        if let Some(c) = classify(in_space)
                            && !log.is_own(root, rel)
                        {
                            push(c);
                        }
                    }
                }
                // Dropped or unreadable events: only a full rescan is safe.
                _ => push(Change::All),
            }
        }
        for c in changes {
            if tx.send_blocking(c).is_err() {
                return;
            }
        }
    }
}

/// Maps a path inside a Space folder to what it holds, using the layout from
/// ADR 0001. Hidden files (editor swap and temp files) are ignored.
fn classify(rel: &Path) -> Option<Change> {
    let parts: Vec<&str> = rel
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [] | ["projects"] | ["notes"] | ["journal"] => Some(Change::All),
        ["projects", p, rest @ ..] if visible(p) => {
            let project = p.to_string();
            match rest {
                [] | ["project.md"] | ["tasks"] | ["notes"] => Some(Change::Project(project)),
                ["tasks", f] => Some(Change::Task {
                    project,
                    slug: md_stem(f)?.to_string(),
                }),
                ["notes", f] => Some(Change::Note {
                    project: Some(project),
                    slug: md_stem(f)?.to_string(),
                }),
                _ => None,
            }
        }
        ["notes", f] => Some(Change::Note {
            project: None,
            slug: md_stem(f)?.to_string(),
        }),
        ["journal", f] => Some(Change::Journal(Date::parse(md_stem(f)?)?)),
        _ => None,
    }
}

fn md_stem(name: &str) -> Option<&str> {
    name.strip_suffix(".md").filter(|s| visible(s))
}

fn visible(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.')
}
