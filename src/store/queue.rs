//! Per-session message queue. While a live worker owns a session, other writers
//! queue events here; the worker delivers them before its next request.
use super::{append_serialized, process_alive, read_jsonl, SessionLock, Store};
use crate::model::{Event, Session, Status};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

/// A queued event, and whether its sender asked for a turn (`send` without
/// `--no-run`); messages queued with `--no-run` are delivered as context only.
#[derive(Serialize, Deserialize)]
struct Queued {
    run: bool,
    event: Event,
}

impl Store {
    /// Stores `event`, or queues it (returning None) while a live worker owns the
    /// session. With `run`, a stored event also claims the session for this
    /// process, in the same locked step, so only one worker can start.
    pub fn deliver(&self, id: &str, event: Event, run: bool) -> io::Result<Option<Session>> {
        let _lock = SessionLock::acquire(&self.path(id))?;
        let mut session = self.load(id)?;
        if session.metadata.pid.is_some_and(process_alive) {
            append_serialized(&self.queue(id), &[Queued { run, event }])?;
            return Ok(None);
        }
        let earlier = self.queued(id)?;
        earlier
            .into_iter()
            .for_each(|queued| session.append(queued.event));
        session.append(event);
        if run {
            session.metadata.status = Status::Running;
            session.metadata.pid = Some(std::process::id());
            session.metadata.termination_reason = None;
        }
        self.save_unlocked(&session)?;
        self.clear_queue(id);
        Ok(Some(session))
    }

    /// Moves queued events into a worker's session before its next request.
    pub fn drain(&self, session: &mut Session) -> io::Result<()> {
        self.take(session, None).map(drop)
    }

    /// Ends a worker's turn atomically: delivers queued events, then returns true
    /// if the turn succeeded and one of them asked for a turn (the worker runs
    /// another), or records `status` and releases the session.
    pub fn finish(&self, session: &mut Session, status: Status) -> io::Result<bool> {
        self.take(session, Some(status))
    }

    /// Delivers queued events under the lock; when finishing, also decides
    /// whether to continue and otherwise sets the final status.
    fn take(&self, session: &mut Session, finish: Option<Status>) -> io::Result<bool> {
        let id = session.metadata.id.clone();
        let _lock = SessionLock::acquire(&self.path(&id))?;
        let queued = self.queued(&id)?;
        let delivered = !queued.is_empty();
        let mut again = false;
        for item in queued {
            again |= item.run;
            session.append(item.event);
        }
        let again = again && finish == Some(Status::Idle);
        if let Some(status) = finish.filter(|_| !again) {
            session.metadata.status = status;
            session.metadata.pid = None;
        }
        if delivered || finish.is_some() {
            self.save_unlocked(session)?;
        }
        self.clear_queue(&id);
        Ok(again)
    }

    /// Reads queued events (none if there is no queue file).
    fn queued(&self, id: &str) -> io::Result<Vec<Queued>> {
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
