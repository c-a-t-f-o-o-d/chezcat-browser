//! Checked admission of an already accepted, one-shot App anchor destination.
//! This is a Session owner factory only. Native input/menu capture and OS window
//! activation belong to later UI/native integration.
use super::*;
use crate::app::anchor::OwnedLinkTarget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinkDisposition {
    Current,
    ForegroundTab,
    BackgroundTab,
    NewWindow,
}

/// Full typed destination identities for the UI/native continuation. NewWindow
/// alone needs a native host. Its content bounds come from the source Session
/// Window; SessionUi adds its strip exactly once when binding the native client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LinkAdmission {
    pub window: WindowId,
    pub tab: TabId,
    pub disposition: LinkDisposition,
}

impl Session {
    /// Revalidate the original complete source IDs without source observation,
    /// refresh, invalidation, editor synchronization, or author work. Callers
    /// retain these IDs alongside the App hit/context across native reentry.
    fn link_source(&self, window: WindowId, tab: TabId) -> Result<(usize, usize), String> {
        self.running()?;
        let wi = self.window_index(window)?;
        let ti = self.tab_index(tab)?;
        if self.tabs[ti].window != window || !self.windows[wi].tabs.contains(&tab) {
            return Err("The original link tab is not in its browser window.".into());
        }
        if self.windows[wi].active != Some(tab) {
            return Err("The original link tab is no longer active.".into());
        }
        Ok((wi, ti))
    }

    /// Consume the exact target exported by App's accepted anchor default.
    /// No string address, explicit grant, request replay, realm, document policy,
    /// or resource base is accepted here. Refusal consumes the one-shot target;
    /// every current owner, selected tab, and identity counter stays in place.
    ///
    /// App hit/context freshness must already have been checked by its consuming
    /// App seam with these same original source IDs. This method checks Session
    /// routing and caps; it does not reconstruct or re-hit an authored link.
    pub(crate) fn open_link(
        &mut self,
        source_window: WindowId,
        source_tab: TabId,
        target: OwnedLinkTarget,
        disposition: LinkDisposition,
    ) -> Result<LinkAdmission, String> {
        let (wi, ti) = self.link_source(source_window, source_tab)?;
        if disposition == LinkDisposition::Current {
            self.tabs[ti].app.open_link_target(target);
            self.mailbox.send(Event::Tab(source_window, source_tab));
            return Ok(LinkAdmission { window: source_window, tab: source_tab, disposition });
        }

        if self.tabs.len() >= self.limits.tabs {
            return Err("The browser tab limit was reached.".into());
        }
        let next_tab = self.next_tab.checked_add(1).ok_or("Browser tab identity exhausted.")?;
        let tab = TabId { generation: self.generation, serial: next_tab };
        let new_window = disposition == LinkDisposition::NewWindow;
        if new_window && self.windows.len() >= self.limits.windows {
            return Err("The browser window limit was reached.".into());
        }
        let next_window = if new_window {
            Some(self.next_window.checked_add(1).ok_or("Browser window identity exhausted.")?)
        } else {
            None
        };
        let window = next_window.map_or(source_window, |serial| WindowId {
            generation: self.generation, serial,
        });
        let (width, height) = (self.windows[wi].width, self.windows[wi].height);

        // Owner Vecs are bounded and initially reserved by adopt. Retain checked
        // local reservations here as well so a later storage change cannot turn
        // publication into an implicit fallible allocation.
        self.tabs.try_reserve(1).map_err(|_| "Cannot reserve a link page owner.")?;
        let mut members = Vec::new();
        if new_window {
            self.windows.try_reserve(1).map_err(|_| "Cannot reserve a link window owner.")?;
            members.try_reserve_exact(self.limits.tabs).map_err(|_| "Cannot reserve link window tabs.")?;
        } else {
            self.windows[wi].tabs.try_reserve(1).map_err(|_| "Cannot reserve link tab membership.")?;
        }

        // Reuse the same shared-service factory as ordinary real page owners.
        // Observation/preparation failure affects only this unpublished App.
        let mut app = self.fresh_app(window, tab, width, height)?;
        // No fallible admission step remains. Existing App's consuming seam is
        // the only executor of the exact resolved Location/root/top-URL origin.
        app.open_link_target(target);

        if disposition == LinkDisposition::ForegroundTab {
            let outgoing = &mut self.tabs[ti].app;
            outgoing.invalidate_platform_requests();
            outgoing.pointer_cancel();
            outgoing.pending_surrogate = None;
        }
        self.next_tab = next_tab;
        self.tabs.push(Tab { id: tab, window, app });
        if let Some(next_window) = next_window {
            self.next_window = next_window;
            members.push(tab);
            self.windows.push(Window { id: window, tabs: members, active: Some(tab), width, height });
        } else {
            self.windows[wi].tabs.push(tab);
            if disposition == LinkDisposition::ForegroundTab {
                self.windows[wi].active = Some(tab);
            }
            // BackgroundTab publishes directly with the original selection.
            // Never select then switch back: that would invalidate live source
            // presses/editors/requests and run source activation observations.
        }
        self.mailbox.send(Event::Tab(window, tab));
        Ok(LinkAdmission { window, tab, disposition })
    }
}

#[cfg(test)]
#[path = "app_session_link_tests.rs"]
mod tests;
