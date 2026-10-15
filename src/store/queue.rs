//! Per-session message queue. While a live worker owns a session, other writers
//! queue events here; the worker delivers them before its next request.
use super::{append_serialized, process_alive, read_jsonl, SessionLock, Store};
use crate::model::{Event, Session, Status};
use std::{fs, io, path::PathBuf};

impl Store {
    /// Stores `event`, or queues it (returning None) while a live worker owns the
    /// session. With `run`, a stored event also claims the session for this
    /// process, in the same locked step, so only one worker can start.
    pub fn deliver(&self, id: &str, event: Event, run: bool) -> io::Result<Option<Session>> {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        if session.metadata.pid.is_some_and(process_alive) {
            append_serialized(&self.queue(id), &[event])?;
            return Ok(None);
        }
        let earlier = self.queued(id)?;
        earlier
            .into_iter()
            .for_each(|queued| session.append(queued));
        session.append(event);
        if run {
            session.metadata.status = Status::Running;
            session.metadata.pid = Some(std::process::id());
        }
        self.save_unlocked(&session)?;
        self.clear_queue(id);
        Ok(Some(session))
    }

    /// Moves queued events into a worker's session; returns whether there were any.
    pub fn drain(&self, session: &mut Session) -> io::Result<bool> {
        self.take(session, false)
    }

    /// Ends a worker's turn atomically: delivers queued events and returns true
    /// (run another turn), or marks the session idle and returns false.
    pub fn finish(&self, session: &mut Session) -> io::Result<bool> {
        self.take(session, true)
    }

    /// Delivers queued events under the lock; with `finish`, an empty queue
    /// marks the session idle instead.
    fn take(&self, session: &mut Session, finish: bool) -> io::Result<bool> {
        let id = session.metadata.id.clone();
        let _lock = SessionLock::acquire(&self.path(&id))?;
        let queued = self.queued(&id)?;
        let delivered = !queued.is_empty();
        queued.into_iter().for_each(|event| session.append(event));
        if finish && !delivered {
            session.metadata.status = Status::Idle;
            session.metadata.pid = None;
        }
        if delivered || finish {
            self.save_unlocked(session)?;
        }
        self.clear_queue(&id);
        Ok(delivered)
    }

    /// Reads queued events (none if there is no queue file).
    fn queued(&self, id: &str) -> io::Result<Vec<Event>> {
        match read_jsonl(&self.queue(id)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            result => result,
        }
    }

    /// Removes the queue file once its events are saved.
    fn clear_queue(&self, id: &str) {
        let _ = fs::remove_file(self.queue(id));
    }

    fn queue(&self, id: &str) -> PathBuf {
        self.path(id).join("queue.jsonl")
    }
}
