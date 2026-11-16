//! Native keys never contain App addresses. All routable identity is copied,
//! process-scoped and resolved again after a reentrant native operation.
use crate::app::session::{TabId, WindowId};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
static NEXT_NATIVE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Main(WindowId),
    Inspector(WindowId, TabId),
}
impl Owner {
    pub(super) fn window(self) -> WindowId {
        match self {
            Self::Main(w) | Self::Inspector(w, _) => w,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Binding {
    pub key: usize,
    pub hwnd: usize,
    pub owner: Owner,
    pub suspended: bool,
    pub closing: bool,
    pub timer: Option<Instant>,
}
pub(super) struct Registry {
    records: Vec<Binding>,
    limit: usize,
}
impl Registry {
    pub(super) fn new(limit: usize) -> Result<Self, String> {
        let mut records = Vec::new();
        records
            .try_reserve_exact(limit)
            .map_err(|_| "Cannot reserve native browser routes.")?;
        Ok(Self { records, limit })
    }
    pub(super) fn reserve(&mut self, owner: Owner) -> Result<usize, String> {
        if self.records.len() == self.limit {
            return Err("The native browser window limit was reached.".into());
        }
        let key = NEXT_NATIVE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                old.checked_add(1).filter(|key| *key <= isize::MAX as usize)
            })
            .map_err(|_| "Native browser identity exhausted.")?
            + 1;
        self.records.push(Binding {
            key,
            hwnd: 0,
            owner,
            suspended: false,
            closing: false,
            timer: None,
        });
        Ok(key)
    }
    // Called by NCCREATE; the caller owns no allocation passed to Windows.
    pub(super) fn attach(&mut self, key: usize, hwnd: usize) -> bool {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|r| r.key == key && r.hwnd == 0 && !r.closing)
        else {
            return false;
        };
        record.hwnd = hwnd;
        true
    }
    pub(super) fn lookup(&self, key: usize, hwnd: usize) -> Option<Binding> {
        self.records
            .iter()
            .find(|r| r.key == key && r.hwnd == hwnd && !r.closing && !r.suspended)
            .copied()
    }
    pub(super) fn get(&self, key: usize) -> Option<Binding> {
        self.records.iter().find(|r| r.key == key).copied()
    }
    pub(super) fn records(&self) -> Vec<Binding> {
        self.records.clone()
    }
    pub(super) fn remove(&mut self, key: usize) -> Option<Binding> {
        self.records
            .iter()
            .position(|r| r.key == key)
            .map(|i| self.records.remove(i))
    }
    pub(super) fn close_key(&mut self, key: usize) -> Option<Binding> {
        let r = self.records.iter_mut().find(|r| r.key == key)?;
        r.closing = true;
        r.suspended = false;
        r.timer = None;
        Some(*r)
    }
    pub(super) fn set_timer(&mut self, key: usize, due: Option<Instant>) {
        if let Some(r) = self.records.iter_mut().find(|r| r.key == key) {
            r.timer = due;
        }
    }
    pub(super) fn suspend_inspectors(&mut self) {
        for r in &mut self.records {
            if matches!(r.owner, Owner::Inspector(..)) {
                r.suspended = true;
            }
        }
    }
    pub(super) fn reconcile(&mut self, alive: impl Fn(Owner) -> bool) -> Vec<Binding> {
        let mut retired = Vec::new();
        for r in &mut self.records {
            if !r.closing && !alive(r.owner) {
                r.closing = true;
                r.suspended = false;
                r.timer = None;
                retired.push(*r);
            } else if !r.closing {
                r.suspended = false;
            }
        }
        retired
    }
    pub(super) fn close_window(&mut self, window: WindowId) -> Vec<Binding> {
        let mut closed = Vec::new();
        for r in &mut self.records {
            if r.owner.window() == window && !r.closing {
                r.closing = true;
                r.timer = None;
                closed.push(*r);
            }
        }
        closed
    }
    pub(super) fn restore_window(&mut self, window: WindowId) {
        for r in &mut self.records {
            if r.owner.window() == window {
                r.closing = false;
                r.suspended = false;
            }
        }
    }
    pub(super) fn close_all(&mut self) -> Vec<Binding> {
        for r in &mut self.records {
            r.closing = true;
            r.timer = None;
        }
        self.records.clone()
    }
}

// Windows does not carry a TabId in CHAR/KEYUP. Retain the exact originating
// owner beside each translated native message, never infer it from active-tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InputOwner {
    pub window: WindowId,
    pub tab: TabId,
    pub inspector: bool,
}
#[derive(Clone, Copy)]
struct TextRoute {
    hwnd: usize,
    scan: usize,
    owner: InputOwner,
}
pub(super) struct InputRoutes {
    text: std::collections::VecDeque<TextRoute>,
    keys: Vec<(usize, u32, InputOwner)>,
}
impl InputRoutes {
    pub(super) fn new() -> Self {
        Self {
            text: std::collections::VecDeque::new(),
            keys: Vec::new(),
        }
    }
    pub(super) fn translated(&mut self, hwnd: usize, lparam: isize, owner: InputOwner) {
        if self.text.len() == 128 {
            self.text.pop_front();
        }
        self.text.push_back(TextRoute {
            hwnd,
            scan: (lparam as usize >> 16) & 0x1ff,
            owner,
        });
    }
    pub(super) fn text(&self, hwnd: usize, lparam: isize) -> Option<InputOwner> {
        let scan = (lparam as usize >> 16) & 0x1ff;
        self.text
            .iter()
            .rev()
            .find(|r| r.hwnd == hwnd && r.scan == scan)
            .map(|r| r.owner)
    }
    pub(super) fn key_down(&mut self, hwnd: usize, key: u32, owner: InputOwner) {
        if self.keys.iter().any(|r| r.0 == hwnd && r.1 == key) {
            return;
        }
        if self.keys.len() < 128 {
            self.keys.push((hwnd, key, owner));
        }
    }
    pub(super) fn key_up(&mut self, hwnd: usize, key: u32) -> Option<InputOwner> {
        self.keys
            .iter()
            .position(|r| r.0 == hwnd && r.1 == key)
            .map(|i| self.keys.remove(i).2)
    }
    pub(super) fn forget(&mut self, hwnd: usize) {
        self.text.retain(|r| r.hwnd != hwnd);
        self.keys.retain(|r| r.0 != hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        session::{Limits, Session},
        App,
    };
    fn owners() -> (Session, WindowId, TabId) {
        let s = Session::adopt(Box::new(App::new()), Limits::default())
            .map_err(|(_, e)| e)
            .unwrap();
        let w = s.window_ids().next().unwrap();
        let t = s.active(w).unwrap().unwrap();
        (s, w, t)
    }
    #[test]
    fn native_key_is_never_reused_even_when_hwnd_is_recycled() {
        let (_s, w, _) = owners();
        let mut r = Registry::new(2).unwrap();
        let old = r.reserve(Owner::Main(w)).unwrap();
        assert!(r.attach(old, 70));
        r.remove(old);
        let new = r.reserve(Owner::Main(w)).unwrap();
        assert_ne!(old, new);
        assert!(r.attach(new, 70));
        assert!(r.lookup(old, 70).is_none());
        assert!(r.lookup(new, 70).is_some());
    }
    #[test]
    fn foreign_session_and_closed_inspector_cannot_alias_an_owner() {
        let (_s, w, t) = owners();
        let (_other, other_w, other_t) = owners();
        assert_eq!(w.value(), other_w.value());
        assert_ne!(w, other_w);
        assert_ne!(t, other_t);
        let mut r = Registry::new(2).unwrap();
        let key = r.reserve(Owner::Inspector(w, t)).unwrap();
        assert!(r.attach(key, 90));
        r.suspend_inspectors();
        assert!(r.lookup(key, 90).is_none());
        let retired = r.reconcile(|owner| owner == Owner::Inspector(other_w, other_t));
        assert_eq!(retired.len(), 1);
        assert!(r.lookup(key, 90).is_none());
    }
    #[test]
    fn translated_text_and_keyup_keep_original_tab_across_switch() {
        let (mut s, w, t) = owners();
        let replacement = s.new_tab(w).unwrap();
        let origin = InputOwner {
            window: w,
            tab: t,
            inspector: false,
        };
        let mut r = InputRoutes::new();
        r.translated(4, 0x1e0001, origin);
        r.key_down(4, 65, origin);
        assert_eq!(r.text(4, 0x1e0001), Some(origin));
        assert_ne!(r.text(4, 0x1e0001).unwrap().tab, replacement);
        assert!(r.text(4, 0x200001).is_none());
        assert_eq!(r.key_up(4, 65), Some(origin));
        assert!(r.key_up(4, 65).is_none());
        r.forget(4);
        assert!(r.text(4, 0x1e0001).is_none());
    }
    #[test]
    fn shutdown_routes_refuse_before_any_native_handle_is_destroyed() {
        let (_s, w, t) = owners();
        let mut r = Registry::new(2).unwrap();
        let main = r.reserve(Owner::Main(w)).unwrap();
        r.attach(main, 5);
        let tools = r.reserve(Owner::Inspector(w, t)).unwrap();
        r.attach(tools, 6);
        let closed = r.close_all();
        assert_eq!(closed.len(), 2);
        assert!(r.lookup(main, 5).is_none());
        assert!(r.lookup(tools, 6).is_none());
        // Retry disposition retains records but never reopens page routing.
        assert_eq!(r.records().len(), 2);
    }
}
