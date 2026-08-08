use crate::model::{LogRecord, Session};
use std::{
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("sessions"))?;
        let global = root.join("logs.jsonl");
        if !global.exists() {
            fs::write(global, "")?;
        }
        Ok(Self { root })
    }

    pub fn create(&self, session: &Session) -> io::Result<()> {
        self.create_with_prompt(session, "")
    }

    pub fn create_with_prompt(&self, session: &Session, prompt: &str) -> io::Result<()> {
        let _lock = SessionLock::acquire(&self.path(&session.metadata.id))?;
        self.save_unlocked_with_prompt(session, Some(prompt))
    }

    pub fn load(&self, id: &str) -> io::Result<Session> {
        let dir = self.path(id);
        let metadata = serde_json::from_slice(&fs::read(dir.join("metadata.json"))?)
            .map_err(io::Error::other)?;
        let events = read_jsonl(&dir.join("events.jsonl"))?;
        Ok(Session { metadata, events })
    }

    pub fn reconcile(&self, id: &str) -> io::Result<Session> {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        if session.metadata.status == crate::model::Status::Running {
            let alive = session
                .metadata
                .pid
                .map(|pid| {
                    std::process::Command::new("kill")
                        .args(["-0", &pid.to_string()])
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if !alive {
                session.metadata.status = crate::model::Status::Failed;
                session.metadata.pid = None;
                session.metadata.termination_reason = Some("worker process disappeared".into());
                session.append(crate::model::Event::Failure {
                    event_id: uuid::Uuid::new_v4().to_string(),
                    timestamp: chrono::Utc::now(),
                    message: "worker process disappeared".into(),
                    connection: None,
                    token_usage: crate::model::TokenUsage::default(),
                });
                self.save_unlocked(&session)?;
            }
        }
        Ok(session)
    }

    pub fn save(&self, session: &Session) -> io::Result<()> {
        let _lock = SessionLock::acquire(&self.path(&session.metadata.id))?;
        self.save_unlocked(session)
    }

    pub fn append_event(&self, id: &str, event: crate::model::Event) -> io::Result<Session> {
        let dir = self.path(id);
        let _lock = SessionLock::acquire(&dir)?;
        let mut session = self.load(id)?;
        session.append(event);
        self.save_unlocked(&session)?;
        Ok(session)
    }

    fn save_unlocked(&self, session: &Session) -> io::Result<()> {
        self.save_unlocked_with_prompt(session, None)
    }

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

    pub fn log(&self, id: &str, record: &LogRecord) -> io::Result<()> {
        append_log(&self.path(id).join("logs.jsonl"), record)
    }

    pub fn log_global(&self, record: &LogRecord) -> io::Result<()> {
        append_log(&self.root.join("logs.jsonl"), record)
    }

    pub fn log_both(&self, id: &str, record: &LogRecord, configured: &str) -> io::Result<()> {
        if level_rank(&record.level) < level_rank(configured) {
            return Ok(());
        }
        self.log(id, record)?;
        self.log_global(record)
    }

    pub fn log_filtered(&self, id: &str, record: &LogRecord, configured: &str) -> io::Result<()> {
        if level_rank(&record.level) >= level_rank(configured) {
            self.log(id, record)
        } else {
            Ok(())
        }
    }

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

    pub fn archive(&self, id: &str) -> io::Result<()> {
        let archive = self.root.join("archive");
        fs::create_dir_all(&archive)?;
        fs::rename(self.path(id), archive.join(id))
    }

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

    pub fn update<F>(&self, id: &str, edit: F) -> io::Result<Session>
    where
        F: FnOnce(&mut Session),
    {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        edit(&mut session);
        self.save_unlocked(&session)?;
        Ok(session)
    }

    fn path(&self, id: &str) -> PathBuf {
        self.root.join("sessions").join(id)
    }
}

struct SessionLock {
    path: PathBuf,
}

impl SessionLock {
    fn acquire(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(".lock");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "session write lock timed out",
                        ));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

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

fn event_count(path: &Path) -> io::Result<usize> {
    read_jsonl::<serde_json::Value>(path).map(|events| events.len())
}

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

fn level_rank(level: &str) -> u8 {
    match level {
        "debug" => 0,
        "info" => 1,
        "warning" => 2,
        "error" => 3,
        _ => 1,
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    fs::write(path, bytes)
}

fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
    let text = fs::read_to_string(path)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(io::Error::other))
        .collect()
}

pub fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config").join("orchid"))
        .unwrap_or_else(|| PathBuf::from(".orchid-simplified"))
}
