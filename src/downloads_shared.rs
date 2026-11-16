//! UI-thread ownership adapter around the existing bounded download Manager.
//! Views own read snapshots only. The browser retains Shared past View/App drop.
//! Workers capture a stable dispatcher, never a particular tab's wake target.
use super::{Entry, Manager, Wake};
use crate::{http, navigation::Location, trust};
use std::cell::RefCell;
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

const BUSY: &str = "Downloads are busy. Try the action again.";
const CLOSED: &str = "This browser download session has closed.";

#[derive(Clone, Default)]
struct Dispatcher {
    target: Arc<Mutex<Option<Wake>>>,
}

impl Dispatcher {
    fn set(&self, target: Option<Wake>) -> Result<(), String> {
        let mut slot = self.target.lock().map_err(|_| "Download wake routing is unavailable.")?;
        // Drop the prior closure after releasing the routing lock, too.
        let previous = std::mem::replace(&mut *slot, target);
        drop(slot);
        drop(previous);
        Ok(())
    }

    fn notify(&self) {
        // A poisoned routing lock suppresses delivery. No callback runs while a
        // routing lock is held; the browser target may itself retarget delivery.
        let target = self.target.lock().ok().and_then(|slot| slot.clone());
        if let Some(target) = target { target(); }
    }

    fn wake(&self) -> Wake {
        let dispatcher = self.clone();
        Arc::new(move || dispatcher.notify())
    }
}

/// One UI-thread browser authority. This is deliberately not Clone: views must
/// not keep a worker/file writer alive after final browser-session destruction.
/// Dropping this authority retains the existing Manager cancellation/join policy;
/// no new file I/O or hidden state save is introduced by this adapter.
pub struct Shared {
    manager: Rc<RefCell<Manager>>,
    dispatcher: Dispatcher,
}

impl Default for Shared {
    fn default() -> Self { Self::new() }
}

impl Shared {
    pub fn new() -> Self {
        Self {manager:Rc::new(RefCell::new(Manager::default())), dispatcher:Dispatcher::default()}
    }

    /// Supply a browser-owned signal target, never a raw App pointer or tab
    /// closure. An in-flight cloned callback cannot be recalled by retargeting;
    /// its target must reject a closed WindowId before accessing native state.
    pub fn set_wake(&self, wake: Option<Wake>) -> Result<(), String> {
        self.dispatcher.set(wake)
    }

    pub fn view(&self) -> Result<View, String> {
        let mut view = View {
            manager:Rc::downgrade(&self.manager), dispatcher:self.dispatcher.clone(),
            entries:Vec::new(), active:0, problem:None,
        };
        view.refresh()?;
        Ok(view)
    }

    /// The browser coordinator polls work even if there are no live App views.
    pub fn poll(&self) -> Result<bool, String> {
        let mut manager = self.manager.try_borrow_mut().map_err(|_| BUSY.to_owned())?;
        Ok(manager.poll())
    }

    pub fn active_count(&self) -> Result<usize, String> {
        let manager = self.manager.try_borrow().map_err(|_| BUSY.to_owned())?;
        Ok(manager.active_count())
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // Detach before Manager fields drop/cancel/join their workers. Poisoned
        // routing already suppresses callbacks in Dispatcher::notify.
        let _ = self.dispatcher.set(None);
    }
}

/// App-local owned bounded observation. Refresh on tab activation and after the
/// browser polls. No RefCell guard or writer reference escapes to caller UI code.
#[derive(Clone)]
pub struct View {
    manager: Weak<RefCell<Manager>>,
    dispatcher: Dispatcher,
    entries: Vec<Entry>,
    active: usize,
    problem: Option<String>,
}

impl View {
    pub fn entries(&self) -> &[Entry] { &self.entries }
    pub fn active_count(&self) -> usize { self.active }
    pub fn problem(&self) -> Option<&str> { self.problem.as_deref() }

    fn authority(&mut self) -> Result<Rc<RefCell<Manager>>, String> {
        if let Some(manager) = self.manager.upgrade() { return Ok(manager); }
        self.active = 0;
        self.problem = Some(CLOSED.into());
        Err(CLOSED.into())
    }

    fn observe(&mut self, manager: &Manager) -> bool {
        let changed = self.active != manager.active_count()
            || !same_entries(&self.entries, manager.entries());
        self.entries = manager.entries().to_vec();
        self.active = manager.active_count();
        self.problem = None;
        changed
    }

    pub fn refresh(&mut self) -> Result<bool, String> {
        let authority = self.authority()?;
        let manager = match authority.try_borrow() {
            Ok(manager) => manager,
            Err(_) => { self.problem=Some(BUSY.into()); return Err(BUSY.into()); }
        };
        Ok(self.observe(&manager))
    }

    fn change<T>(&mut self, action: impl FnOnce(&mut Manager, Wake) -> Result<T, String>) -> Result<(T, bool), String> {
        let authority = self.authority()?;
        let mut manager = match authority.try_borrow_mut() {
            Ok(manager) => manager,
            Err(_) => { self.problem=Some(BUSY.into()); return Err(BUSY.into()); }
        };
        // The action uses only original Manager entry points. It performs no
        // App/native UI callback. Worker wake targets only signal the browser.
        let result = action(&mut manager, self.dispatcher.wake());
        // launch() can poll finished work before refusing admission, so refresh
        // the observation even when the requested new operation fails.
        let changed = self.observe(&manager);
        drop(manager);
        result.map(|value| (value, changed))
    }

    pub fn start(&mut self, location:Location, suggested:Option<String>, folder:&Path, trust:Arc<trust::Store>) -> Result<u64, String> {
        self.change(|manager,wake| manager.start(location,suggested,folder,trust,wake)).map(|(id,_)|id)
    }

    pub fn start_received(&mut self, response:http::Response, folder:&Path, trust:Arc<trust::Store>) -> Result<u64, String> {
        self.change(|manager,wake| manager.start_received(response,folder,trust,wake)).map(|(id,_)|id)
    }

    pub fn retry(&mut self, id:u64, folder:&Path, trust:Arc<trust::Store>) -> Result<u64, String> {
        self.change(|manager,wake| manager.retry(id,folder,trust,wake)).map(|(id,_)|id)
    }

    pub fn cancel(&mut self, id:u64) -> Result<bool, String> {
        self.change(|manager,_| Ok(manager.cancel(id))).map(|(cancelled,_)|cancelled)
    }

    pub fn clear_finished(&mut self) -> Result<usize, String> {
        self.change(|manager,_| Ok(manager.clear_finished())).map(|(removed,_)|removed)
    }

    pub fn poll(&mut self) -> Result<bool, String> {
        self.change(|manager,_| Ok(manager.poll())).map(|(worker_changed,observed)| worker_changed||observed)
    }
}

fn same_entries(old:&[Entry], new:&[Entry]) -> bool {
    old.len()==new.len() && old.iter().zip(new).all(|(old,new)|
        old.id==new.id && old.name==new.name && old.source==new.source
        && old.folder==new.folder && old.status==new.status && old.bytes==new.bytes
        && old.total==new.total && old.path==new.path && old.started==new.started
        && old.elapsed==new.elapsed && old.can_retry()==new.can_retry())
}

#[cfg(test)]
#[path = "downloads_shared_tests.rs"]
mod tests;
