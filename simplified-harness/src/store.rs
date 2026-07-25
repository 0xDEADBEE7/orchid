use crate::model::Session;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("sessions"))?;
        Ok(Self { root })
    }

    pub fn create(&self, session: &Session) -> io::Result<()> {
        self.save(session)
    }

    pub fn load(&self, id: &str) -> io::Result<Session> {
        let dir = self.path(id);
        let metadata = serde_json::from_slice(&fs::read(dir.join("metadata.json"))?)
            .map_err(io::Error::other)?;
        let state =
            serde_json::from_slice(&fs::read(dir.join("state.json"))?).map_err(io::Error::other)?;
        let events = read_jsonl(&dir.join("events.jsonl"))?;
        Ok(Session {
            metadata,
            state,
            events,
        })
    }

    pub fn reconcile(&self, id: &str) -> io::Result<Session> {
        let mut session = self.load(id)?;
        if session.state.status == crate::model::Status::Running {
            let alive = session
                .state
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
                session.state.status = crate::model::Status::Failed;
                session.state.pid = None;
                self.save(&session)?;
            }
        }
        Ok(session)
    }

    pub fn save(&self, session: &Session) -> io::Result<()> {
        let dir = self.path(&session.metadata.id);
        fs::create_dir_all(&dir)?;
        write_json(&dir.join("metadata.json"), &session.metadata)?;
        write_json(&dir.join("state.json"), &session.state)?;
        let events = session
            .events
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?
            .join("\n");
        fs::write(
            dir.join("events.jsonl"),
            if events.is_empty() {
                String::new()
            } else {
                format!("{events}\n")
            },
        )?;
        if !dir.join("logs.jsonl").exists() {
            fs::write(dir.join("logs.jsonl"), "")?;
        }
        Ok(())
    }

    pub fn list(&self) -> io::Result<Vec<Session>> {
        fs::read_dir(self.root.join("sessions"))?
            .map(|entry| {
                let path = entry?.path();
                if !path.is_dir() {
                    return Err(io::Error::other("unexpected session entry"));
                }
                let id = path
                    .file_name()
                    .and_then(|x| x.to_str())
                    .ok_or_else(|| io::Error::other("invalid session directory"))?;
                self.load(id)
            })
            .collect()
    }

    pub fn archive(&self, id: &str) -> io::Result<()> {
        let archive = self.root.join("archive");
        fs::create_dir_all(&archive)?;
        fs::rename(self.path(id), archive.join(id))
    }

    pub fn update<F>(&self, id: &str, edit: F) -> io::Result<Session>
    where
        F: FnOnce(&mut Session),
    {
        let mut session = self.load(id)?;
        edit(&mut session);
        self.save(&session)?;
        Ok(session)
    }

    fn path(&self, id: &str) -> PathBuf {
        self.root.join("sessions").join(id)
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

pub fn default_root() -> &'static Path {
    Path::new(".orchid-simplified")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Event, Session};

    #[test]
    fn round_trips_and_archives_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path()).unwrap();
        let mut session = Session::new(Some("test".into()), None, None);
        session.append(Event::Message {
            role: "user".into(),
            content: "hello".into(),
        });
        store.create(&session).unwrap();
        assert_eq!(store.load(&session.metadata.id).unwrap(), session);
        store.archive(&session.metadata.id).unwrap();
        assert!(store.load(&session.metadata.id).is_err());
    }

    #[test]
    fn reconciles_dead_running_processes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path()).unwrap();
        let mut session = Session::new(None, None, None);
        session.state.status = crate::model::Status::Running;
        session.state.pid = Some(999_999);
        store.create(&session).unwrap();
        let recovered = store.reconcile(&session.metadata.id).unwrap();
        assert_eq!(recovered.state.status, crate::model::Status::Failed);
    }
}
