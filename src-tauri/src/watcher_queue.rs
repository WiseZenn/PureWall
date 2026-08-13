use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const MAX_ROOTS: usize = 256;
pub const MAX_PATHS_PER_ROOT: usize = 4096;
pub const QUIET_WINDOW: Duration = Duration::from_millis(250);
pub const MAX_BATCH_LATENCY: Duration = Duration::from_secs(2);
pub const MAX_WORKERS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Paths(Vec<PathSignal>),
    NeedRescan,
    NotifyError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSignal {
    pub path: PathBuf,
    pub scan_existing_directory: bool,
    pub include_missing: bool,
}

impl PathSignal {
    pub fn new(path: PathBuf, scan_existing_directory: bool, include_missing: bool) -> Self {
        Self {
            path,
            scan_existing_directory,
            include_missing,
        }
    }

    fn merge(&mut self, other: Self) {
        self.scan_existing_directory |= other.scan_existing_directory;
        self.include_missing |= other.include_missing;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItem {
    pub root: String,
    pub source: String,
    pub paths: Vec<PathSignal>,
    pub full_snapshot: bool,
    pub reason: Option<String>,
    admission_token: u64,
}

struct RootState {
    root: String,
    source: String,
    admission_token: u64,
    paths: BTreeMap<PathBuf, PathSignal>,
    full_snapshot: bool,
    reason: Option<String>,
    first_pending: Option<Instant>,
    last_signal: Instant,
    dirty: bool,
}

struct State {
    roots: BTreeMap<String, RootState>,
    active_work: BTreeMap<String, u64>,
    next_admission_token: u64,
    closed: bool,
}

pub struct WatcherQueue {
    shared: Arc<(Mutex<State>, Condvar)>,
    workers: Mutex<Option<Vec<JoinHandle<()>>>>,
}

impl WatcherQueue {
    pub fn new<F>(callback: F) -> Arc<Self>
    where
        F: Fn(WorkItem) + Send + Sync + 'static,
    {
        let shared = Arc::new((
            Mutex::new(State {
                roots: BTreeMap::new(),
                active_work: BTreeMap::new(),
                next_admission_token: 1,
                closed: false,
            }),
            Condvar::new(),
        ));
        let callback: Arc<dyn Fn(WorkItem) + Send + Sync> = Arc::new(callback);
        let mut workers = Vec::with_capacity(MAX_WORKERS);
        for index in 0..MAX_WORKERS {
            let shared = Arc::clone(&shared);
            let callback = Arc::clone(&callback);
            workers.push(
                thread::Builder::new()
                    .name(format!("purewall-watcher-{index}"))
                    .spawn(move || worker_loop(shared, callback))
                    .expect("watcher worker should spawn"),
            );
        }
        Arc::new(Self {
            shared,
            workers: Mutex::new(Some(workers)),
        })
    }

    pub fn admit_root(&self, root: &str, source: &str) -> Result<(), &'static str> {
        let (lock, _) = &*self.shared;
        let mut state = lock.lock().map_err(|_| "watcher queue unavailable")?;
        if state.closed {
            return Err("watcher queue is shut down");
        }
        let root_key = crate::paths::path_identity_key(std::path::Path::new(root));
        if !state.roots.contains_key(&root_key) && state.roots.len() >= MAX_ROOTS {
            return Err("PureWall cannot watch more than 256 library roots at once");
        }
        let needs_new_admission = state
            .roots
            .get(&root_key)
            .is_none_or(|root_state| root_state.source != source);
        if needs_new_admission {
            let admission_token = state.next_admission_token;
            state.next_admission_token = admission_token
                .checked_add(1)
                .ok_or("watcher admission generation exhausted")?;
            state.roots.insert(
                root_key,
                RootState {
                    root: root.to_string(),
                    source: source.to_string(),
                    admission_token,
                    paths: BTreeMap::new(),
                    full_snapshot: false,
                    reason: None,
                    first_pending: None,
                    last_signal: Instant::now(),
                    dirty: false,
                },
            );
        }
        Ok(())
    }

    pub fn remove_root(&self, root: &str, source: &str) -> bool {
        if let Ok(mut state) = self.shared.0.lock() {
            let root_key = crate::paths::path_identity_key(std::path::Path::new(root));
            if state
                .roots
                .get(&root_key)
                .is_some_and(|root_state| root_state.source == source)
            {
                return state.roots.remove(&root_key).is_some();
            }
        }
        false
    }

    pub fn submit(&self, root: &str, source: &str, signal: Signal) -> Result<(), &'static str> {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().map_err(|_| "watcher queue unavailable")?;
        if state.closed {
            return Err("watcher queue is shut down");
        }
        let root_key = crate::paths::path_identity_key(std::path::Path::new(root));
        let identity_in_flight = state.active_work.contains_key(&root_key);
        let root_state = state
            .roots
            .get_mut(&root_key)
            .ok_or("watcher root is not admitted")?;
        if root_state.source != source {
            return Err("watcher source is not admitted for this root");
        }
        root_state.last_signal = Instant::now();
        if identity_in_flight {
            root_state.dirty = true;
        }
        match signal {
            Signal::Paths(paths) => {
                if !root_state.full_snapshot {
                    let mut overflowed = false;
                    for path_signal in paths {
                        let path = path_signal.path.clone();
                        if let Some(existing) = root_state.paths.get_mut(&path) {
                            existing.merge(path_signal);
                            continue;
                        }
                        if root_state.paths.len() >= MAX_PATHS_PER_ROOT {
                            overflowed = true;
                            break;
                        }
                        root_state.paths.insert(path, path_signal);
                    }
                    if overflowed {
                        root_state.paths.clear();
                        root_state.full_snapshot = true;
                        root_state.reason = Some("path_capacity".into());
                    }
                }
            }
            Signal::NeedRescan => {
                root_state.paths.clear();
                root_state.full_snapshot = true;
                if root_state.reason.is_none() {
                    root_state.reason = Some("notify_rescan".into());
                }
            }
            Signal::NotifyError(message) => {
                root_state.paths.clear();
                root_state.full_snapshot = true;
                if !root_state
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.starts_with("notify_error:"))
                {
                    root_state.reason = Some(format!("notify_error: {message}"));
                }
            }
        }
        if root_state.first_pending.is_none() {
            root_state.first_pending = Some(Instant::now());
        }
        wake.notify_all();
        Ok(())
    }

    pub fn close(&self) {
        let (_, wake) = &*self.shared;
        if let Ok(mut state) = self.shared.0.lock() {
            state.closed = true;
        }
        wake.notify_all();
    }

    pub fn close_and_join(&self) {
        self.close();
        if let Ok(mut workers) = self.workers.lock() {
            for worker in workers.take().unwrap_or_default() {
                let _ = worker.join();
            }
        }
    }

    #[cfg(test)]
    fn root_count(&self) -> usize {
        self.shared.0.lock().unwrap().roots.len()
    }
}

impl Drop for WatcherQueue {
    fn drop(&mut self) {
        self.close_and_join()
    }
}

fn worker_loop(
    shared: Arc<(Mutex<State>, Condvar)>,
    callback: Arc<dyn Fn(WorkItem) + Send + Sync>,
) {
    loop {
        let item = {
            let (lock, wake) = &*shared;
            let mut state = match lock.lock() {
                Ok(state) => state,
                Err(_) => return,
            };
            loop {
                if state.closed {
                    return;
                }
                let now = Instant::now();
                let mut selected = None;
                let mut next_due = None;
                for (root_key, root_state) in &state.roots {
                    if state.active_work.contains_key(root_key)
                        || root_state.first_pending.is_none()
                    {
                        continue;
                    }
                    let first = root_state.first_pending.unwrap();
                    let due = std::cmp::min(
                        first + MAX_BATCH_LATENCY,
                        root_state.last_signal + QUIET_WINDOW,
                    );
                    if due <= now {
                        selected = Some(root_key.clone());
                        break;
                    }
                    next_due = Some(next_due.map_or(due, |current: Instant| current.min(due)));
                }
                if let Some(root_key) = selected {
                    let item = {
                        let root_state = state.roots.get_mut(&root_key).unwrap();
                        root_state.first_pending = None;
                        let paths = std::mem::take(&mut root_state.paths)
                            .into_values()
                            .collect();
                        let full_snapshot = root_state.full_snapshot;
                        let reason = root_state.reason.take();
                        root_state.full_snapshot = false;
                        WorkItem {
                            root: root_state.root.clone(),
                            source: root_state.source.clone(),
                            paths,
                            full_snapshot,
                            reason,
                            admission_token: root_state.admission_token,
                        }
                    };
                    state.active_work.insert(root_key, item.admission_token);
                    break item;
                }
                state = match next_due {
                    Some(due) => {
                        wake.wait_timeout(state, due.saturating_duration_since(now))
                            .unwrap()
                            .0
                    }
                    None => wake.wait(state).unwrap(),
                };
            }
        };
        callback(item.clone());
        if let Ok(mut state) = shared.0.lock() {
            let root_key = crate::paths::path_identity_key(std::path::Path::new(&item.root));
            let completed_active_work = state
                .active_work
                .get(&root_key)
                .is_some_and(|token| *token == item.admission_token);
            if completed_active_work {
                state.active_work.remove(&root_key);
            }
            if completed_active_work {
                if let Some(root_state) = state.roots.get_mut(&root_key) {
                    if root_state.dirty {
                        root_state.dirty = false;
                        if root_state.first_pending.is_none() {
                            root_state.first_pending = Some(Instant::now());
                        }
                    }
                }
            }
            shared.1.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    fn path_signal(path: impl Into<PathBuf>) -> PathSignal {
        PathSignal::new(path.into(), false, false)
    }

    #[test]
    fn path_capacity_promotes_once_to_snapshot() {
        let (tx, rx) = mpsc::channel();
        let queue = WatcherQueue::new(move |item| {
            tx.send(item).unwrap();
        });
        queue.admit_root("root", "source").unwrap();
        let paths = (0..MAX_PATHS_PER_ROOT)
            .map(|i| path_signal(format!("{i}.jpg")))
            .collect();
        queue
            .submit("root", "source", Signal::Paths(paths))
            .unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("overflow.jpg")]),
            )
            .unwrap();
        let item = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(item.full_snapshot);
        assert_eq!(item.reason.as_deref(), Some("path_capacity"));
        queue.close();
    }

    #[test]
    fn duplicate_paths_do_not_consume_remaining_capacity() {
        let (tx, rx) = mpsc::channel();
        let queue = WatcherQueue::new(move |item| {
            tx.send(item).unwrap();
        });
        queue.admit_root("root", "source").unwrap();
        let paths = (0..MAX_PATHS_PER_ROOT - 1)
            .map(|i| path_signal(format!("{i}.jpg")))
            .collect();
        queue
            .submit("root", "source", Signal::Paths(paths))
            .unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("0.jpg"); MAX_PATHS_PER_ROOT]),
            )
            .unwrap();

        let item = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(!item.full_snapshot);
        assert_eq!(item.paths.len(), MAX_PATHS_PER_ROOT - 1);
        queue.close_and_join();
    }

    #[test]
    fn duplicate_at_exact_unique_path_boundary_stays_incremental() {
        let (tx, rx) = mpsc::channel();
        let queue = WatcherQueue::new(move |item| {
            tx.send(item).unwrap();
        });
        queue.admit_root("root", "source").unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("0.jpg"); MAX_PATHS_PER_ROOT]),
            )
            .unwrap();
        let remaining = (1..MAX_PATHS_PER_ROOT)
            .map(|i| path_signal(format!("{i}.jpg")))
            .collect();
        queue
            .submit("root", "source", Signal::Paths(remaining))
            .unwrap();
        queue
            .submit("root", "source", Signal::Paths(vec![path_signal("0.jpg")]))
            .unwrap();

        let item = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(!item.full_snapshot);
        assert_eq!(item.paths.len(), MAX_PATHS_PER_ROOT);
        queue.close_and_join();
    }

    #[test]
    fn need_rescan_clears_paths_and_repeated_signals_coalesce() {
        let (tx, rx) = mpsc::channel();
        let queue = WatcherQueue::new(move |item| {
            tx.send(item).unwrap();
        });
        queue.admit_root("root", "source").unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("before-rescan.jpg")]),
            )
            .unwrap();
        queue.submit("root", "source", Signal::NeedRescan).unwrap();
        queue.submit("root", "source", Signal::NeedRescan).unwrap();

        let item = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(item.full_snapshot);
        assert!(item.paths.is_empty());
        assert_eq!(item.reason.as_deref(), Some("notify_rescan"));
        assert!(rx.recv_timeout(QUIET_WINDOW * 2).is_err());
        queue.close_and_join();
    }

    #[test]
    fn notify_error_reason_survives_a_coalesced_rescan() {
        let (tx, rx) = mpsc::channel();
        let queue = WatcherQueue::new(move |item| {
            tx.send(item).unwrap();
        });
        queue.admit_root("root", "source").unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("unreliable.jpg")]),
            )
            .unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::NotifyError("backend unavailable".into()),
            )
            .unwrap();
        queue.submit("root", "source", Signal::NeedRescan).unwrap();

        let item = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(item.full_snapshot);
        assert!(item.paths.is_empty());
        assert_eq!(
            item.reason.as_deref(),
            Some("notify_error: backend unavailable")
        );
        assert!(rx.recv_timeout(QUIET_WINDOW * 2).is_err());
        queue.close_and_join();
    }

    #[test]
    fn in_flight_signals_schedule_exactly_one_follow_up() {
        let (item_tx, item_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let invocation = Arc::new(AtomicUsize::new(0));
        let queue = WatcherQueue::new({
            let release_rx = Arc::clone(&release_rx);
            let invocation = Arc::clone(&invocation);
            move |item| {
                let current = invocation.fetch_add(1, Ordering::SeqCst);
                item_tx.send(item).unwrap();
                if current == 0 {
                    release_rx.lock().unwrap().recv().unwrap();
                }
            }
        });
        queue.admit_root("root", "source").unwrap();
        queue.submit("root", "source", Signal::NeedRescan).unwrap();
        let first = item_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(first.full_snapshot);

        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("follow-up-a.jpg")]),
            )
            .unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("follow-up-b.jpg")]),
            )
            .unwrap();
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal("follow-up-a.jpg")]),
            )
            .unwrap();
        release_tx.send(()).unwrap();

        let follow_up = item_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(!follow_up.full_snapshot);
        assert_eq!(
            follow_up.paths,
            vec![
                path_signal("follow-up-a.jpg"),
                path_signal("follow-up-b.jpg")
            ]
        );
        assert!(item_rx.recv_timeout(QUIET_WINDOW * 2).is_err());
        assert_eq!(invocation.load(Ordering::SeqCst), 2);
        queue.close_and_join();
    }

    #[test]
    fn continuous_signals_dispatch_by_maximum_batch_latency() {
        let (tx, rx) = mpsc::channel();
        let queue = WatcherQueue::new(move |item| {
            tx.send(item).unwrap();
        });
        queue.admit_root("root", "source").unwrap();
        let start = Instant::now();
        let repeated_path = PathBuf::from("continuous.jpg");
        queue
            .submit(
                "root",
                "source",
                Signal::Paths(vec![path_signal(repeated_path.clone())]),
            )
            .unwrap();

        let item = loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(item) => break item,
                Err(mpsc::RecvTimeoutError::Timeout) => queue
                    .submit(
                        "root",
                        "source",
                        Signal::Paths(vec![path_signal(repeated_path.clone())]),
                    )
                    .unwrap(),
                Err(error) => panic!("watcher callback channel disconnected: {error}"),
            }
            assert!(
                start.elapsed() <= MAX_BATCH_LATENCY + Duration::from_secs(1),
                "continuous signals exceeded the maximum batch latency"
            );
        };

        assert_eq!(item.paths, vec![path_signal(repeated_path)]);
        assert!(!item.full_snapshot);
        assert!(
            start.elapsed() <= MAX_BATCH_LATENCY + Duration::from_millis(500),
            "dispatch exceeded the maximum batch latency allowance"
        );
        queue.close_and_join();
    }

    #[test]
    fn root_aliases_share_one_admission_and_removal_identity() {
        let queue = WatcherQueue::new(|_| {});
        queue.admit_root(r"D:\Walls", "source-a").unwrap();
        queue.admit_root(r"d:/walls\", "source-a").unwrap();

        assert_eq!(queue.root_count(), 1);
        assert!(queue.remove_root(r"d:/WALLS", "source-a"));
        assert_eq!(queue.root_count(), 0);
        queue.close_and_join();
    }

    #[test]
    fn source_replacement_preserves_in_flight_owner_and_uses_new_owner_afterward() {
        let (item_tx, item_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let invocation = Arc::new(AtomicUsize::new(0));
        let queue = WatcherQueue::new({
            let release_rx = Arc::clone(&release_rx);
            let invocation = Arc::clone(&invocation);
            move |item| {
                let current = invocation.fetch_add(1, Ordering::SeqCst);
                item_tx.send(item).unwrap();
                if current == 0 {
                    release_rx.lock().unwrap().recv().unwrap();
                }
            }
        });
        queue.admit_root(r"D:\Walls", "source-a").unwrap();
        queue
            .submit(r"d:/walls", "source-a", Signal::NeedRescan)
            .unwrap();
        let stale = item_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(stale.root, r"D:\Walls");
        assert_eq!(stale.source, "source-a");

        queue.admit_root(r"d:/WALLS", "source-b").unwrap();
        queue
            .submit(
                r"D:\Walls",
                "source-b",
                Signal::Paths(vec![path_signal("new-owner.jpg")]),
            )
            .unwrap();
        release_tx.send(()).unwrap();

        let current = item_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(current.root, r"d:/WALLS");
        assert_eq!(current.source, "source-b");
        queue.close_and_join();
    }

    #[test]
    fn removed_and_readmitted_root_does_not_accept_stale_completion() {
        let (started_tx, started_rx) = mpsc::channel();
        let (old_release_tx, old_release_rx) = mpsc::channel();
        let old_release_rx = Arc::new(Mutex::new(old_release_rx));
        let (new_release_tx, new_release_rx) = mpsc::channel();
        let new_release_rx = Arc::new(Mutex::new(new_release_rx));
        let queue = WatcherQueue::new({
            let old_release_rx = Arc::clone(&old_release_rx);
            let new_release_rx = Arc::clone(&new_release_rx);
            move |item| {
                started_tx.send(item.clone()).unwrap();
                if item.root == r"D:\Walls" && item.full_snapshot {
                    old_release_rx.lock().unwrap().recv().unwrap();
                } else if item
                    .paths
                    .iter()
                    .any(|signal| signal.path == std::path::Path::new("new-owner.jpg"))
                {
                    new_release_rx.lock().unwrap().recv().unwrap();
                }
            }
        });

        queue.admit_root(r"D:\Walls", "source").unwrap();
        queue
            .submit(r"D:\Walls", "source", Signal::NeedRescan)
            .unwrap();
        let old = started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(old.full_snapshot);

        assert!(queue.remove_root(r"d:/walls", "source"));
        queue.admit_root(r"d:/WALLS", "source").unwrap();
        queue
            .submit(
                r"d:/walls",
                "source",
                Signal::Paths(vec![path_signal("new-owner.jpg")]),
            )
            .unwrap();
        let premature_new = started_rx.recv_timeout(QUIET_WINDOW * 2);
        if let Ok(unexpected) = premature_new {
            old_release_tx.send(()).unwrap();
            new_release_tx.send(()).unwrap();
            queue.close_and_join();
            panic!(
                "readmitted root started beside the stale callback for the same identity: {unexpected:?}"
            );
        }

        old_release_tx.send(()).unwrap();
        let new = started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(new.root, r"d:/WALLS");
        assert_eq!(new.paths, vec![path_signal("new-owner.jpg")]);

        queue
            .submit(
                r"D:\Walls",
                "source",
                Signal::Paths(vec![path_signal("follow-up.jpg")]),
            )
            .unwrap();
        let concurrent = started_rx.recv_timeout(QUIET_WINDOW * 2);
        new_release_tx.send(()).unwrap();
        if let Ok(unexpected) = concurrent {
            queue.close_and_join();
            panic!(
                "stale completion allowed a third callback beside the active new owner: {unexpected:?}"
            );
        }

        let follow_up = started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(follow_up.paths, vec![path_signal("follow-up.jpg")]);
        assert!(started_rx.recv_timeout(QUIET_WINDOW * 2).is_err());
        queue.close_and_join();
    }

    #[test]
    fn roots_are_bounded_and_rescan_coalesces() {
        let queue = WatcherQueue::new(|_| {});
        for i in 0..MAX_ROOTS {
            queue.admit_root(&format!("root-{i}"), "source").unwrap();
        }
        assert!(queue.admit_root("overflow", "source").is_err());
        assert_eq!(queue.root_count(), MAX_ROOTS);
        queue.close();
    }

    #[test]
    fn worker_count_is_capped_and_ready_roots_make_progress() {
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let queue = WatcherQueue::new({
            let release = Arc::clone(&release);
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            move |item| {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                started_tx.send(item.root).unwrap();
                let mut released = release.0.lock().unwrap();
                while !*released {
                    released = release.1.wait(released).unwrap();
                }
                active.fetch_sub(1, Ordering::SeqCst);
            }
        });
        for root in ["root-a", "root-b", "root-c"] {
            queue.admit_root(root, "source").unwrap();
            queue.submit(root, "source", Signal::NeedRescan).unwrap();
        }

        let mut first_wave = vec![
            started_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            started_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
        ];
        first_wave.sort();
        assert_eq!(first_wave, vec!["root-a", "root-b"]);
        assert_eq!(peak.load(Ordering::SeqCst), MAX_WORKERS);
        assert!(started_rx.recv_timeout(QUIET_WINDOW).is_err());

        *release.0.lock().unwrap() = true;
        release.1.notify_all();
        assert_eq!(
            started_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            "root-c"
        );
        queue.close_and_join();
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn close_and_join_waits_until_an_active_callback_finishes() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let queue = WatcherQueue::new(move |_| {
            started_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        });
        queue.admit_root("root", "source").unwrap();
        queue.submit("root", "source", Signal::NeedRescan).unwrap();
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap();

        let (joined_tx, joined_rx) = mpsc::channel();
        let closer = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || {
                queue.close_and_join();
                joined_tx.send(()).unwrap();
            })
        };
        assert!(joined_rx.recv_timeout(QUIET_WINDOW).is_err());
        release_tx.send(()).unwrap();
        joined_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        closer.join().unwrap();
    }

    #[test]
    fn close_and_join_wakes_idle_workers() {
        let queue = WatcherQueue::new(|_| panic!("idle worker must not invoke callback"));
        let (joined_tx, joined_rx) = mpsc::channel();
        let closer = {
            let queue = Arc::clone(&queue);
            std::thread::spawn(move || {
                queue.close_and_join();
                joined_tx.send(()).unwrap();
            })
        };

        joined_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        closer.join().unwrap();
    }
}
