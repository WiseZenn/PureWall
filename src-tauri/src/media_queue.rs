use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

const MAX_QUEUED_THUMBNAILS: usize = 256;
const MAX_QUEUED_SPECULATIVE_PREVIEWS: usize = 2;

#[cfg(feature = "performance-harness")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MediaQueueSnapshot {
    pub pending_paths: usize,
    pub running_paths: usize,
    pub pending_thumbnails: usize,
    pub pending_previews: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MediaJobKind {
    Thumbnail,
    Preview,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MediaNeeds {
    pub thumbnail: bool,
    pub preview: bool,
}

impl MediaNeeds {
    fn for_kind(kind: MediaJobKind) -> Self {
        match kind {
            MediaJobKind::Thumbnail => Self {
                thumbnail: true,
                preview: false,
            },
            MediaJobKind::Preview => Self {
                thumbnail: false,
                preview: true,
            },
        }
    }

    fn merge(&mut self, other: Self) {
        self.thumbnail |= other.thumbnail;
        self.preview |= other.preview;
    }

    fn missing_from(self, queued: Self, running: Self) -> Self {
        Self {
            thumbnail: self.thumbnail && !queued.thumbnail && !running.thumbnail,
            preview: self.preview && !queued.preview && !running.preview,
        }
    }

    fn is_empty(self) -> bool {
        !self.thumbnail && !self.preview
    }

    fn primary_kind(self) -> MediaJobKind {
        if self.preview {
            MediaJobKind::Preview
        } else {
            MediaJobKind::Thumbnail
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MediaPriority {
    Thumbnail,
    SpeculativePreview,
    Preview,
    ActivePreview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaJob {
    pub kind: MediaJobKind,
    pub path: String,
    pub needs: MediaNeeds,
    pub priority: MediaPriority,
}

#[derive(Clone)]
pub struct MediaJobQueue {
    inner: Arc<QueueInner>,
}

struct QueueInner {
    state: Mutex<QueueState>,
    condvar: Condvar,
}

struct PathJobState {
    queued: MediaNeeds,
    running: MediaNeeds,
    priority: MediaPriority,
    is_running: bool,
}

#[derive(Default)]
struct QueueState {
    previews: VecDeque<String>,
    speculative_previews: VecDeque<String>,
    thumbnails: VecDeque<String>,
    jobs: HashMap<String, PathJobState>,
    stopped: bool,
}

impl MediaJobQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(QueueInner {
                state: Mutex::new(QueueState::default()),
                condvar: Condvar::new(),
            }),
        }
    }

    pub fn enqueue(&self, kind: MediaJobKind, path: String) -> bool {
        let priority = match kind {
            MediaJobKind::Thumbnail => MediaPriority::Thumbnail,
            MediaJobKind::Preview => MediaPriority::Preview,
        };
        self.enqueue_request(path, MediaNeeds::for_kind(kind), priority)
    }

    pub fn enqueue_speculative_preview(&self, path: String) -> bool {
        self.enqueue_request(
            path,
            MediaNeeds {
                thumbnail: false,
                preview: true,
            },
            MediaPriority::SpeculativePreview,
        )
    }

    pub fn enqueue_active_preview(&self, path: String) -> bool {
        self.enqueue_request(
            path,
            MediaNeeds {
                thumbnail: true,
                preview: true,
            },
            MediaPriority::ActivePreview,
        )
    }

    pub fn enqueue_request(
        &self,
        path: String,
        needs: MediaNeeds,
        priority: MediaPriority,
    ) -> bool {
        if path.is_empty() || needs.is_empty() {
            return false;
        }

        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.stopped {
            return false;
        }
        if priority == MediaPriority::ActivePreview {
            let stale_paths: Vec<String> = state.speculative_previews.drain(..).collect();
            for stale_path in stale_paths {
                state.jobs.remove(&stale_path);
            }
        }

        if state.jobs.contains_key(&path) {
            let (accepted, is_running, queued_priority) = {
                let job = state.jobs.get_mut(&path).expect("path job should exist");
                let missing = needs.missing_from(job.queued, job.running);
                let needs_accepted = !missing.is_empty();
                if needs_accepted {
                    job.queued.merge(missing);
                }
                let priority_upgraded = priority > job.priority;
                if priority_upgraded {
                    job.priority = priority;
                }
                (
                    needs_accepted || priority_upgraded,
                    job.is_running,
                    job.priority,
                )
            };

            if accepted && !is_running {
                state.previews.retain(|queued| queued != &path);
                state.speculative_previews.retain(|queued| queued != &path);
                state.thumbnails.retain(|queued| queued != &path);
                if queued_priority >= MediaPriority::Preview {
                    state.previews.push_front(path);
                } else if queued_priority == MediaPriority::SpeculativePreview {
                    state.speculative_previews.push_back(path);
                } else {
                    state.thumbnails.push_back(path);
                }
            }
            if accepted {
                if queued_priority >= MediaPriority::SpeculativePreview {
                    self.inner.condvar.notify_all();
                } else {
                    self.inner.condvar.notify_one();
                }
            }
            return accepted;
        }

        if priority == MediaPriority::SpeculativePreview
            && state.speculative_previews.len() >= MAX_QUEUED_SPECULATIVE_PREVIEWS
        {
            return false;
        }

        if !needs.preview && state.thumbnails.len() >= MAX_QUEUED_THUMBNAILS {
            if let Some(dropped) = state.thumbnails.pop_front() {
                state.jobs.remove(&dropped);
            }
        }

        state.jobs.insert(
            path.clone(),
            PathJobState {
                queued: needs,
                running: MediaNeeds::default(),
                priority,
                is_running: false,
            },
        );
        if priority >= MediaPriority::Preview {
            state.previews.push_front(path);
        } else if priority == MediaPriority::SpeculativePreview {
            state.speculative_previews.push_back(path);
        } else {
            state.thumbnails.push_back(path);
        }
        if priority >= MediaPriority::SpeculativePreview {
            self.inner.condvar.notify_all();
        } else {
            self.inner.condvar.notify_one();
        }
        true
    }

    pub fn pop(&self) -> Option<MediaJob> {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(job) = Self::pop_ready_job(&mut state) {
                return Some(job);
            }
            if state.stopped {
                return None;
            }
            state = self
                .inner
                .condvar
                .wait(state)
                .unwrap_or_else(|p| p.into_inner());
        }
    }

    pub fn try_pop(&self) -> Option<MediaJob> {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        Self::pop_ready_job(&mut state)
    }

    pub fn pop_preview(&self) -> Option<MediaJob> {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(job) = Self::pop_preview_job(&mut state) {
                return Some(job);
            }
            if state.stopped {
                return None;
            }
            state = self
                .inner
                .condvar
                .wait(state)
                .unwrap_or_else(|p| p.into_inner());
        }
    }

    pub fn try_pop_preview(&self) -> Option<MediaJob> {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        Self::pop_preview_job(&mut state)
    }

    pub fn finish(&self, job: &MediaJob) -> bool {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        let Some(path_state) = state.jobs.get_mut(&job.path) else {
            return false;
        };
        path_state.running = MediaNeeds::default();
        path_state.is_running = false;
        let has_pending = !path_state.queued.is_empty();
        let priority = path_state.priority;

        if !has_pending {
            state.jobs.remove(&job.path);
            return false;
        }

        if priority >= MediaPriority::Preview {
            state.previews.push_front(job.path.clone());
        } else if priority == MediaPriority::SpeculativePreview {
            state.speculative_previews.push_back(job.path.clone());
        } else {
            state.thumbnails.push_back(job.path.clone());
        }
        if priority >= MediaPriority::SpeculativePreview {
            self.inner.condvar.notify_all();
        } else {
            self.inner.condvar.notify_one();
        }
        true
    }

    pub fn shutdown(&self) {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        state.stopped = true;
        self.inner.condvar.notify_all();
    }

    #[cfg(feature = "performance-harness")]
    pub(crate) fn performance_snapshot(&self) -> MediaQueueSnapshot {
        let state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        MediaQueueSnapshot {
            pending_paths: state
                .jobs
                .values()
                .filter(|job| !job.queued.is_empty())
                .count(),
            running_paths: state
                .jobs
                .values()
                .filter(|job| job.is_running && !job.running.is_empty())
                .count(),
            pending_thumbnails: state.thumbnails.len(),
            pending_previews: state.previews.len() + state.speculative_previews.len(),
        }
    }

    #[cfg(feature = "performance-harness")]
    pub(crate) fn performance_config_descriptor() -> String {
        format!(
            "media-queue:max-thumbnails={MAX_QUEUED_THUMBNAILS};max-speculative-previews={MAX_QUEUED_SPECULATIVE_PREVIEWS}"
        )
    }

    fn pop_preview_job(state: &mut QueueState) -> Option<MediaJob> {
        while let Some(path) = state.previews.pop_front() {
            if let Some(job) = Self::claim_path(state, path) {
                return Some(job);
            }
        }
        None
    }

    fn pop_ready_job(state: &mut QueueState) -> Option<MediaJob> {
        loop {
            let path = state
                .previews
                .pop_front()
                .or_else(|| state.speculative_previews.pop_front())
                .or_else(|| state.thumbnails.pop_front())?;
            if let Some(job) = Self::claim_path(state, path) {
                return Some(job);
            }
        }
    }

    fn claim_path(state: &mut QueueState, path: String) -> Option<MediaJob> {
        let path_state = state.jobs.get_mut(&path)?;
        if path_state.is_running || path_state.queued.is_empty() {
            return None;
        }

        let needs = path_state.queued;
        path_state.queued = MediaNeeds::default();
        path_state.running = needs;
        path_state.is_running = true;
        Some(MediaJob {
            kind: needs.primary_kind(),
            path,
            needs,
            priority: path_state.priority,
        })
    }
}

impl Default for MediaJobQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_jobs_are_claimed_before_queued_thumbnails() {
        let queue = MediaJobQueue::new();
        assert!(queue.enqueue(MediaJobKind::Thumbnail, "thumb-a".to_string()));
        assert!(queue.enqueue(MediaJobKind::Thumbnail, "thumb-b".to_string()));
        assert!(queue.enqueue(MediaJobKind::Preview, "preview-a".to_string()));

        let job = queue.try_pop().expect("a job should be ready");

        assert_eq!(job.kind, MediaJobKind::Preview);
        assert_eq!(job.path, "preview-a");
    }

    #[test]
    fn newest_preview_is_claimed_before_older_preview_work() {
        let queue = MediaJobQueue::new();
        assert!(queue.enqueue(MediaJobKind::Preview, "older".to_string()));
        assert!(queue.enqueue(MediaJobKind::Preview, "newer".to_string()));

        let job = queue.try_pop().expect("a preview job should be ready");

        assert_eq!(job.kind, MediaJobKind::Preview);
        assert_eq!(job.path, "newer");
    }

    #[test]
    fn thumbnail_queue_drops_oldest_jobs_when_over_capacity() {
        let queue = MediaJobQueue::new();
        for index in 0..(MAX_QUEUED_THUMBNAILS + 3) {
            assert!(queue.enqueue(MediaJobKind::Thumbnail, format!("thumb-{index}")));
        }

        let job = queue.try_pop().expect("a thumbnail job should be ready");

        assert_eq!(job.kind, MediaJobKind::Thumbnail);
        assert_eq!(job.path, "thumb-3");
        assert!(queue.enqueue(MediaJobKind::Thumbnail, "thumb-0".to_string()));
    }

    #[test]
    fn duplicate_jobs_are_ignored_until_finished() {
        let queue = MediaJobQueue::new();
        assert!(queue.enqueue(MediaJobKind::Thumbnail, "same".to_string()));
        assert!(!queue.enqueue(MediaJobKind::Thumbnail, "same".to_string()));

        let job = queue.try_pop().expect("the original job should be ready");
        assert!(!queue.enqueue(MediaJobKind::Thumbnail, "same".to_string()));

        queue.finish(&job);
        assert!(queue.enqueue(MediaJobKind::Thumbnail, "same".to_string()));
    }

    #[test]
    fn shutdown_unblocks_waiting_workers() {
        let queue = MediaJobQueue::new();
        let worker_queue = queue.clone();
        let worker = std::thread::spawn(move || worker_queue.pop());

        queue.shutdown();

        assert_eq!(worker.join().expect("worker should exit cleanly"), None);
    }

    #[test]
    fn speculative_preview_backlog_is_capped_at_two_and_skips_reserved_lane() {
        let queue = MediaJobQueue::new();

        assert!(queue.enqueue_speculative_preview("likely-first".to_string()));
        assert!(queue.enqueue_speculative_preview("likely-second".to_string()));
        assert!(!queue.enqueue_speculative_preview("unlikely-third".to_string()));
        assert!(queue.try_pop_preview().is_none());

        let first = queue
            .try_pop()
            .expect("first speculative preview should be ready");
        let second = queue
            .try_pop()
            .expect("second speculative preview should be ready");
        assert_eq!(first.path, "likely-first");
        assert_eq!(first.priority, MediaPriority::SpeculativePreview);
        assert_eq!(second.path, "likely-second");
        assert!(queue.try_pop().is_none());
    }

    #[test]
    fn active_preview_evicts_stale_queued_speculative_work() {
        let queue = MediaJobQueue::new();
        assert!(queue.enqueue_speculative_preview("stale-first".to_string()));
        assert!(queue.enqueue_speculative_preview("stale-second".to_string()));

        assert!(queue.enqueue_active_preview("active".to_string()));

        let active = queue
            .try_pop_preview()
            .expect("active preview should be available to the reserved lane");
        assert_eq!(active.path, "active");
        assert_eq!(active.priority, MediaPriority::ActivePreview);
        assert!(queue.try_pop().is_none());
    }

    #[cfg(feature = "performance-harness")]
    #[test]
    fn performance_snapshot_counts_pending_and_running_needs() {
        let queue = MediaJobQueue::new();
        assert!(queue.enqueue(MediaJobKind::Thumbnail, "thumb-a".to_string()));
        assert!(queue.enqueue(MediaJobKind::Thumbnail, "thumb-b".to_string()));
        assert!(queue.enqueue_active_preview("active".to_string()));

        let preview = queue
            .try_pop_preview()
            .expect("active preview should be claimable");
        let snapshot = queue.performance_snapshot();

        assert_eq!(snapshot.pending_paths, 2);
        assert_eq!(snapshot.running_paths, 1);
        assert_eq!(snapshot.pending_thumbnails, 2);
        assert_eq!(snapshot.pending_previews, 0);
        queue.finish(&preview);
    }
}
