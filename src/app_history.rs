//! Saved browsing history belongs to the browser, independent of the current
//! page, its JavaScript, and the in-memory Back/Forward navigation stack.

use super::*;
use std::time::{Duration, Instant};

const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(60);

impl App {
    /// Desktop startup only. Headless rendering never attaches this disk store.
    pub fn attach_browsing_history(&mut self, store: crate::history::Store) {
        let shared = crate::history::Shared::new(store);
        let view = shared.view().expect("A fresh history authority is available.");
        let commands = shared.commands();
        self.browsing_history = view;
        self.history_commands = commands;
        self.owned_history = Some(shared);
        self.history_closed = false;
        let _ = self.refresh_browsing_history();
    }

    /// Startup-only attachment. The browser coordinator retains the authority
    /// and owns final cleanup; App/View/command handles cannot shut it down.
    pub fn attach_shared_browsing_history(&mut self, shared: &crate::history::Shared) -> Result<(), String> {
        self.browsing_history.sync()?;
        if self.history_closed || !self.browsing_history.data().entries.is_empty()
            || self.browsing_history.data().policy != crate::history::Policy::default() {
            return Err("Attach the browser history session before retaining visits or configuring history.".into());
        }
        let view = shared.view()?;
        let commands = shared.commands();
        self.browsing_history = view;
        self.history_commands = commands;
        self.owned_history = None;
        self.history_closed = false;
        self.refresh_browsing_history().map(|_| ())
    }

    /// Pure activation observation; expiry/file work remains in refresh.
    pub fn sync_shared_browsing_history(&mut self) -> Result<bool, String> {
        let result = self.browsing_history.sync();
        if !matches!(&result, Ok(false)) { self.cancel_history_panel_pointers(); }
        match &result {
            Ok(_) => self.browsing_history_changed(),
            Err(error) => self.browsing_history_failed(error.clone()),
        }
        result
    }

    pub(super) fn configure_browsing_history(&mut self, expected: &crate::history::PolicyToken,
        policy: crate::history::Policy, now: u64) -> Result<bool, String> {
        if self.history_closed { return Err("This window's history owner has closed.".into()); }
        self.cancel_history_panel_pointers();
        let command = self.history_commands.configure_checked(expected, policy, now);
        let observed = self.browsing_history.sync();
        let result = command.and_then(|changed| observed.map(|observed| changed || observed));
        match &result {
            Ok(_) => self.browsing_history_changed(),
            Err(error) => self.browsing_history_failed(error.clone()),
        }
        result
    }

    pub(super) fn refresh_browsing_history(&mut self) -> Result<bool, String> {
        self.refresh_browsing_history_at(crate::history::now())
    }

    pub(super) fn refresh_browsing_history_at(&mut self, now: u64) -> Result<bool, String> {
        let result = self.browsing_history.refresh(now);
        if !matches!(&result, Ok(false)) { self.cancel_history_panel_pointers(); }
        match &result {
            Ok(_) => self.browsing_history_changed(),
            Err(error) => self.browsing_history_failed(error.clone()),
        }
        result
    }

    /// Called after a successful UI mutation as well as automatic refreshes.
    /// Preserve an already armed future wake so ordinary page/paint activity
    /// cannot indefinitely postpone age cleanup.
    pub(super) fn browsing_history_changed(&mut self) {
        self.browsing_history_error = self.browsing_history.problem().map(str::to_owned);
        let active = self.browsing_history.data().policy.enabled
            || !self.browsing_history.data().entries.is_empty()
            || self.browsing_history_error.is_some();
        self.history_maintenance = if !active || self.history_closed {
            None
        } else {
            let now = Instant::now();
            let remaining = self
                .browsing_history
                .next_expiration()
                .map(|expiration| {
                    Duration::from_secs(expiration.saturating_sub(crate::history::now()).max(1))
                })
                .unwrap_or(MAINTENANCE_INTERVAL)
                .min(MAINTENANCE_INTERVAL);
            let desired = now + remaining;
            Some(
                self.history_maintenance
                    .filter(|old| *old > now)
                    .map_or(desired, |old| old.min(desired)),
            )
        };
        self.rebuild_link_colors();
        self.sync_browser_panel();
    }

    fn browsing_history_failed(&mut self, error: String) {
        self.browsing_history_error = Some(error);
        // A busy/read-only/corrupt file must not cause a tight retry loop.
        self.history_maintenance =
            (!self.history_closed).then(|| Instant::now() + MAINTENANCE_INTERVAL);
    }

    pub(super) fn record_browsing_visit(&mut self) {
        if self.history_closed || (self.location.url.is_none() && self.location.path.is_none()) {
            return;
        }
        // Only committed top-level documents call this, once per load. The
        // store checks its latest persisted policy before any append. Sources,
        // resources, rejected navigations, fragments and POST bodies never enter.
        let result = self.browsing_history.record(
            crate::history::now(),
            &self.document.title,
            &self.location.display(),
        );
        match result {
            Ok(_) => self.browsing_history_changed(),
            Err(error) => self.browsing_history_failed(error),
        }
    }

    pub fn next_history_maintenance(&self) -> Option<Instant> {
        self.history_maintenance
    }

    pub fn poll_history_maintenance(&mut self) -> bool {
        if self
            .history_maintenance
            .is_none_or(|due| due > Instant::now())
        {
            return false;
        }
        self.history_maintenance = None;
        let previous_error = self.browsing_history_error.clone();
        let changed = self.refresh_browsing_history().unwrap_or(false);
        changed || previous_error != self.browsing_history_error
    }

    pub fn history_timer_unavailable(&mut self) {
        self.history_maintenance = None;
        self.browsing_history_error = Some(
            "Automatic history cleanup could not be scheduled. Cleanup still runs when opening History or loading a page.".into(),
        );
    }

    /// Legacy final-window cleanup uses its owned browser authority. Attached
    /// Apps close local participation only; no destructor writes or clears data.
    pub fn finish_browsing_session(&mut self) -> Result<(), String> {
        if self.history_closed {
            return Ok(());
        }
        self.cancel_pending();
        self.scripting = None;
        self.history_maintenance = None;
        // A legacy App owns the whole single-window authority. An attached App
        // closes only local participation; the browser calls Shared.shutdown
        // once after its last window, independently of App/View lifetimes.
        let result = match &self.owned_history {
            Some(shared) => shared.shutdown(crate::history::now()),
            None => Ok(0),
        };
        let result = result.and_then(|removed| {
            if self.owned_history.is_some() { self.browsing_history.sync()?; }
            Ok(removed)
        });
        match result {
            Ok(_) => {
                self.history_closed = true;
                self.browsing_history_error = None;
                Ok(())
            }
            Err(error) => {
                self.browsing_history_error = Some(error.clone());
                Err(format!("History cleanup on exit failed: {error}"))
            }
        }
    }
}
