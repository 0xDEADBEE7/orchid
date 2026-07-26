use crate::{config::Settings, model::Status, store::Store};
use std::{fs, io, path::PathBuf};

pub struct HookState {
    store: Store,
    id: String,
    previous: Status,
    token_path: PathBuf,
}

impl HookState {
    pub fn enter(settings: &Settings, id: &str) -> io::Result<Self> {
        let store = Store::new(&settings.root)?;
        let mut session = store.load(id)?;
        let previous = session.metadata.status;
        session.metadata.status = Status::HookRunning;
        store.save(&session)?;
        let token_path = settings.root.join("sessions").join(id).join(".hook-token");
        fs::write(&token_path, uuid::Uuid::new_v4().to_string())?;
        Ok(Self {
            store,
            id: id.into(),
            previous,
            token_path,
        })
    }
}

impl Drop for HookState {
    fn drop(&mut self) {
        if let Ok(mut session) = self.store.load(&self.id) {
            if session.metadata.status == Status::HookRunning {
                session.metadata.status = self.previous;
                let _ = self.store.save(&session);
            }
        }
        let _ = fs::remove_file(&self.token_path);
    }
}
