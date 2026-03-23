//! WebSocket broadcast + filesystem watch (`notify`) for live UI updates.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

/// Paths written by the server process; FS watcher ignores matching events briefly to avoid echo reloads.
#[derive(Clone, Default)]
pub struct FsSuppress {
    inner: Arc<Mutex<HashMap<PathBuf, Instant>>>,
}

impl FsSuppress {
    /// Mark `path` (any file under `data_dir`) as server-originated until `ttl` elapses.
    pub fn mark_path(&self, path: &Path, ttl: Duration) {
        let mut m = self.inner.lock().expect("fs suppress lock");
        m.insert(path.to_path_buf(), Instant::now() + ttl);
        m.retain(|_, until| *until > Instant::now());
    }

    fn is_suppressed(&self, path: &Path) -> bool {
        let mut m = self.inner.lock().expect("fs suppress lock");
        m.retain(|_, until| *until > Instant::now());
        m.iter()
            .any(|(p, _)| path.starts_with(p) || p.starts_with(path))
    }
}

/// Whether live reload is enabled (WebSocket + fs watcher). Disabled in e2e via env.
#[must_use]
pub fn live_reload_enabled_from_env() -> bool {
    std::env::var("PRIME_AGENT_DISABLE_LIVE_RELOAD")
        .ok()
        .as_deref()
        != Some("1")
}

#[must_use]
pub fn live_broadcast_channel() -> broadcast::Sender<String> {
    broadcast::Sender::new(256)
}

/// Returns true if a path under `data_dir` is a skill or pipeline file change we care about.
pub(crate) fn should_notify_fs_event(
    event: &Event,
    data_dir: &Path,
    suppress: &FsSuppress,
) -> bool {
    for path in &event.paths {
        if should_notify_path(path, data_dir) && !suppress.is_suppressed(path) {
            return true;
        }
    }
    false
}

fn should_notify_path(path: &Path, data_dir: &Path) -> bool {
    let lossy = path.to_string_lossy();
    if lossy.contains("/.git/") || lossy.contains("\\.git\\") {
        return false;
    }
    if lossy.ends_with(".tmp") {
        return false;
    }
    let Ok(rel) = path.strip_prefix(data_dir) else {
        return false;
    };
    let r = rel.to_string_lossy();
    r.starts_with("skills/") || r.starts_with("pipelines/")
}

pub fn spawn_fs_watcher<F>(data_dir: PathBuf, suppress: &FsSuppress, on_fs_batch: F)
where
    F: Fn() + Send + 'static,
{
    let dd = data_dir.clone();
    let suppress_c = suppress.clone();
    thread::spawn(move || {
        let (debounce_tx, debounce_rx) = std::sync::mpsc::channel::<()>();

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res
                    && should_notify_fs_event(&event, &dd, &suppress_c)
                {
                    let _ = debounce_tx.send(());
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("live_reload: notify init failed: {e}");
                return;
            }
        };

        let skills = data_dir.join("skills");
        let pipelines = data_dir.join("pipelines");
        for dir in [skills, pipelines] {
            if dir.exists()
                && let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive)
            {
                eprintln!("live_reload: watch {} failed: {e}", dir.display());
            }
        }

        while debounce_rx.recv().is_ok() {
            thread::sleep(Duration::from_millis(200));
            while debounce_rx.try_recv().is_ok() {}
            on_fs_batch();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn notifies_under_skills() {
        let data = PathBuf::from("/data");
        let p = data.join("skills").join("x").join("SKILL.md");
        assert!(should_notify_path(&p, &data));
    }

    #[test]
    fn notifies_under_pipelines() {
        let data = PathBuf::from("/data");
        let p = data.join("pipelines").join("p").join("pipeline.json");
        assert!(should_notify_path(&p, &data));
    }

    #[test]
    fn ignores_git() {
        let data = PathBuf::from("/data");
        let p = data.join(".git").join("HEAD");
        assert!(!should_notify_path(&p, &data));
    }

    #[test]
    fn ignores_tmp() {
        let data = PathBuf::from("/data");
        let p = data.join("skills").join("x").join("pipeline.json.tmp");
        assert!(!should_notify_path(&p, &data));
    }

    #[test]
    fn fs_event_respects_suppress() {
        use notify::EventKind;
        let data = PathBuf::from("/data");
        let p = data.join("skills").join("x").join("SKILL.md");
        let sup = FsSuppress::default();
        sup.mark_path(&p, Duration::from_secs(60));
        let ev = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Any),
            paths: vec![p],
            attrs: notify::event::EventAttributes::default(),
        };
        assert!(!should_notify_fs_event(&ev, &data, &sup));
    }
}
