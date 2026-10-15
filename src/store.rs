//! Provides the store functionality.
use crate::model::{LogRecord, Session, Status};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone)]
/// Performs the Store operation.
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Creates a new value.
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("sessions"))?;
        let global = root.join("logs.jsonl");
        if !global.exists() {
            fs::write(global, "")?;
        }
        Ok(Self { root })
    }

    /// Performs the create operation.
    pub fn create(&self, session: &Session) -> io::Result<()> {
        self.create_with_prompt(session, "")
    }

    /// Performs the create with prompt operation.
    pub fn create_with_prompt(&self, session: &Session, prompt: &str) -> io::Result<()> {
        let _lock = SessionLock::acquire(&self.path(&session.metadata.id))?;
        self.save_unlocked_with_prompt(session, Some(prompt))
    }

    /// Loads data from persistent storage.
    pub fn load(&self, id: &str) -> io::Result<Session> {
        let dir = self.path(id);
        let metadata = serde_json::from_slice(&fs::read(dir.join("metadata.json"))?)
            .map_err(io::Error::other)?;
        let events = read_jsonl(&dir.join("events.jsonl"))?;
        Ok(Session { metadata, events })
    }

    /// Marks a session whose worker died as failed, answering any open tool calls.
    pub fn reconcile(&self, id: &str) -> io::Result<Session> {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        if session.metadata.status == Status::Running
            && !session.metadata.pid.is_some_and(process_alive)
        {
            session.metadata.status = Status::Failed;
            session.metadata.pid = None;
            session.metadata.termination_reason = Some("worker process disappeared".into());
            session.close_pending("worker process disappeared");
            session.append(crate::model::Event::Failure {
                event_id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now(),
                message: "worker process disappeared".into(),
                connection: None,
                token_usage: crate::model::TokenUsage::default(),
            });
            self.save_unlocked(&session)?;
        }
        Ok(session)
    }

    /// Atomically marks an idle session as running for this process. Fails with
    /// WouldBlock while a live worker owns it; the caller records the worker pid.
    pub fn claim(&self, id: &str) -> io::Result<Session> {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        if session.metadata.pid.is_some_and(process_alive) {
            let busy = io::Error::new(io::ErrorKind::WouldBlock, "session is already running");
            return Err(busy);
        }
        session.metadata.status = Status::Running;
        session.metadata.pid = Some(std::process::id());
        self.save_unlocked_with_prompt(&session, None)?;
        Ok(session)
    }

    /// Persists data to storage.
    pub fn save(&self, session: &Session) -> io::Result<()> {
        let _lock = SessionLock::acquire(&self.path(&session.metadata.id))?;
        self.save_unlocked(session)
    }

    /// Performs the append event operation.
    pub fn append_event(&self, id: &str, event: crate::model::Event) -> io::Result<Session> {
        let dir = self.path(id);
        let _lock = SessionLock::acquire(&dir)?;
        let mut session = self.load(id)?;
        session.append(event);
        self.save_unlocked(&session)?;
        Ok(session)
    }

    /// Performs the save unlocked operation.
    fn save_unlocked(&self, session: &Session) -> io::Result<()> {
        let mut session = session.clone();
        self.preserve_external_fields(&mut session)?;
        self.save_unlocked_with_prompt(&session, None)
    }

    /// Keeps fields that other commands (`set`, `session`) may have changed on disk
    /// while a long-lived in-memory copy was held, e.g. by a worker.
    fn preserve_external_fields(&self, session: &mut Session) -> io::Result<()> {
        let path = self.path(&session.metadata.id).join("metadata.json");
        if !path.exists() {
            return Ok(());
        }
        let current: crate::model::Metadata =
            serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)?;
        session.metadata.policy = current.policy;
        session.metadata.label = current.label;
        session.metadata.working_dir = current.working_dir;
        Ok(())
    }

    /// Performs the save unlocked with prompt operation.
    fn save_unlocked_with_prompt(&self, session: &Session, prompt: Option<&str>) -> io::Result<()> {
        let dir = self.path(&session.metadata.id);
        fs::create_dir_all(&dir)?;
        write_json(&dir.join("metadata.json"), &session.metadata)?;
        append_events(&dir.join("events.jsonl"), &session.events)?;
        let prompt_path = dir.join("prompt.md");
        if let Some(prompt) = prompt {
            fs::write(&prompt_path, prompt)?;
        } else if !prompt_path.exists() {
            fs::write(&prompt_path, "")?;
        }
        if !dir.join("logs.jsonl").exists() {
            fs::write(dir.join("logs.jsonl"), "")?;
        }
        Ok(())
    }

    /// Records a lifecycle message.
    pub fn log(&self, id: &str, record: &LogRecord) -> io::Result<()> {
        append_log(&self.path(id).join("logs.jsonl"), record)
    }

    /// Performs the log global operation.
    pub fn log_global(&self, record: &LogRecord) -> io::Result<()> {
        append_log(&self.root.join("logs.jsonl"), record)
    }

    /// Performs the log both operation.
    pub fn log_both(&self, id: &str, record: &LogRecord, configured: &str) -> io::Result<()> {
        if level_rank(&record.level) < level_rank(configured) {
            return Ok(());
        }
        self.log(id, record)?;
        self.log_global(record)
    }

    /// Performs the log filtered operation.
    pub fn log_filtered(&self, id: &str, record: &LogRecord, configured: &str) -> io::Result<()> {
        if level_rank(&record.level) >= level_rank(configured) {
            self.log(id, record)
        } else {
            Ok(())
        }
    }

    /// Lists the available entries.
    pub fn list(&self) -> io::Result<Vec<Session>> {
        let sessions = fs::read_dir(self.root.join("sessions"))?
            .filter_map(|entry| {
                let path = match entry {
                    Ok(entry) => entry.path(),
                    Err(_) => return None,
                };
                if !path.is_dir() {
                    return None;
                }
                let id = path
                    .file_name()
                    .and_then(|x| x.to_str())
                    .map(str::to_owned)?;
                self.load(&id).ok()
            })
            .collect::<Vec<_>>();
        Ok(sessions)
    }

    /// Performs the archive operation.
    pub fn archive(&self, id: &str) -> io::Result<()> {
        let archive = self.root.join("archive");
        fs::create_dir_all(&archive)?;
        fs::rename(self.path(id), archive.join(id))
    }

    /// Performs the update with prompt operation.
    pub fn update_with_prompt<F>(&self, id: &str, prompt: &str, edit: F) -> io::Result<Session>
    where
        F: FnOnce(&mut Session),
    {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        edit(&mut session);
        self.save_unlocked_with_prompt(&session, Some(prompt))?;
        Ok(session)
    }

    /// Performs the update operation.
    pub fn update<F>(&self, id: &str, edit: F) -> io::Result<Session>
    where
        F: FnOnce(&mut Session),
    {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        edit(&mut session);
        self.save_unlocked_with_prompt(&session, None)?;
        Ok(session)
    }

    /// Performs the path operation.
    fn path(&self, id: &str) -> PathBuf {
        self.root.join("sessions").join(id)
    }
}

/// An exclusive flock on the session's lock file. The kernel releases it when the
/// file is dropped or the process dies, so a killed writer never wedges a session.
/// (A distinct filename from the old marker-file lock, which it replaces.)
struct SessionLock {
    _file: fs::File,
}

impl SessionLock {
    /// Blocks until the session lock is held.
    fn acquire(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(".flock"))?;
        file.lock()?;
        Ok(Self { _file: file })
    }
}

/// Returns whether a process exists (signal 0), without writing to stderr.
pub fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Performs the append events operation.
fn append_events<T: serde::Serialize>(path: &Path, events: &[T]) -> io::Result<()> {
    if !path.exists() {
        fs::File::create(path)?;
    }
    let existing = event_count(path)?;
    if existing > events.len() {
        return Err(io::Error::other("event stream cannot shrink"));
    }
    if existing == events.len() {
        return Ok(());
    }
    append_serialized(path, &events[existing..])
}

/// Performs the event count operation.
fn event_count(path: &Path) -> io::Result<usize> {
    read_jsonl::<serde_json::Value>(path).map(|events| events.len())
}

/// Performs the append serialized operation.
fn append_serialized<T: serde::Serialize>(path: &Path, events: &[T]) -> io::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    for event in events {
        writeln!(
            file,
            "{}",
            serde_json::to_string(event).map_err(io::Error::other)?
        )?;
    }
    Ok(())
}

/// Performs the append log operation.
fn append_log(path: &Path, record: &LogRecord) -> io::Result<()> {
    let mut line = serde_json::to_string(record).map_err(io::Error::other)?;
    line.push('\n');
    use std::io::Write;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())
}

/// Performs the level rank operation.
fn level_rank(level: &str) -> u8 {
    match level {
        "debug" => 0,
        "info" => 1,
        "warning" => 2,
        "error" => 3,
        _ => 1,
    }
}

/// Writes JSON to a temporary file and renames it, so readers never see a partial file.
fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(value).map_err(io::Error::other)?,
    )?;
    fs::rename(tmp, path)
}

/// Performs the read jsonl operation.
fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
    let text = fs::read_to_string(path)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(io::Error::other))
        .collect()
}

/// Performs the default root operation.
pub fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config").join("orchid"))
        .unwrap_or_else(|| PathBuf::from(".orchid-simplified"))
}
