use crate::{Error, ErrorDto, Result};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

pub type TaskId = u32;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEvent {
    Progress {
        id: TaskId,
        done: u32,
        total: u32,
        msg: String,
    },
    Finished {
        id: TaskId,
    },
    Cancelled {
        id: TaskId,
    },
    Failed {
        id: TaskId,
        error: ErrorDto,
    },
    /// Not a task: files changed outside the app (`what` = "mods" | "config"). Shares this channel.
    FsChanged {
        what: String,
    },
}

/// Where task events go; the Tauri layer implements this to emit UI events.
pub trait TaskSink: Send + Sync + 'static {
    fn emit(&self, event: TaskEvent);
}

/// Handle given to a running task for progress + cancellation.
pub struct TaskCtx {
    id: TaskId,
    cancel: Arc<AtomicBool>,
    sink: Arc<dyn TaskSink>,
}

impl TaskCtx {
    pub fn progress(&self, done: u32, total: u32, msg: impl Into<String>) {
        self.sink.emit(TaskEvent::Progress {
            id: self.id,
            done,
            total,
            msg: msg.into(),
        });
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// `Err(Cancelled)` if cancel was requested — use with `?` in loops.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

pub struct TaskManager {
    sink: Arc<dyn TaskSink>,
    next: AtomicU32,
    running: Arc<Mutex<HashMap<TaskId, Arc<AtomicBool>>>>,
}

impl TaskManager {
    pub fn new(sink: impl TaskSink) -> Self {
        Self {
            sink: Arc::new(sink),
            next: AtomicU32::new(1),
            running: Default::default(),
        }
    }

    /// Run blocking/CPU work on its own thread; returns immediately.
    // chisle: std::thread per task; switch to a bounded pool if task counts grow.
    pub fn spawn<F>(&self, work: F) -> TaskId
    where
        F: FnOnce(&TaskCtx) -> Result<()> + Send + 'static,
    {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap().insert(id, cancel.clone());
        let ctx = TaskCtx {
            id,
            cancel,
            sink: self.sink.clone(),
        };
        let running = self.running.clone();
        std::thread::spawn(move || {
            let event = match work(&ctx) {
                Ok(()) => TaskEvent::Finished { id },
                Err(Error::Cancelled) => TaskEvent::Cancelled { id },
                Err(e) => TaskEvent::Failed {
                    id,
                    error: e.into(),
                },
            };
            running.lock().unwrap().remove(&id);
            ctx.sink.emit(event);
        });
        id
    }

    /// The event sink, for non-task notifications (file watching).
    pub fn sink(&self) -> Arc<dyn TaskSink> {
        self.sink.clone()
    }

    pub fn cancel(&self, id: TaskId) -> Result<()> {
        match self.running.lock().unwrap().get(&id) {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            }
            None => Err(Error::TaskNotFound(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct ChanSink(Mutex<mpsc::Sender<TaskEvent>>);
    impl TaskSink for ChanSink {
        fn emit(&self, e: TaskEvent) {
            self.0.lock().unwrap().send(e).ok();
        }
    }

    #[test]
    fn finishes_and_cancels() {
        let (tx, rx) = mpsc::channel();
        let tm = TaskManager::new(ChanSink(Mutex::new(tx)));

        let a = tm.spawn(|ctx| {
            ctx.progress(1, 1, "x");
            Ok(())
        });
        assert!(matches!(rx.recv().unwrap(), TaskEvent::Progress { id, .. } if id == a));
        assert!(matches!(rx.recv().unwrap(), TaskEvent::Finished { id } if id == a));

        let b = tm.spawn(|ctx| {
            loop {
                ctx.check()?;
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });
        tm.cancel(b).unwrap();
        assert!(matches!(rx.recv().unwrap(), TaskEvent::Cancelled { id } if id == b));
        assert!(tm.cancel(b).is_err());
    }
}
