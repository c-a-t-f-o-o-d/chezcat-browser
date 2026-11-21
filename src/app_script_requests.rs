//! App integration for the shared asynchronous script request worker.
use super::*;
use crate::script_request_core::{Completion, Failure, Worker};
impl App {
    pub(super) fn process_native_script_requests(&mut self) -> bool {
        if self.script_jobs_paused() {
            return false;
        }
        let mut changed = false;
        let count = self
            .scripting
            .as_ref()
            .map_or(0, |page| page.native_request_pending_count());
        for _ in 0..count {
            let Some(page) = &mut self.scripting else {
                break;
            };
            let Some(request) = page.take_native_request() else {
                break;
            };
            if request.cancel.load(std::sync::atomic::Ordering::Acquire) {
                continue;
            }
            changed = true;
            let (used, limit) = page.native_request_storage();
            if self.script_requests.is_none() {
                match Worker::new(
                    self.wake.clone(),
                    self.trust.clone(),
                    self.cookies.clone(),
                    used,
                    limit,
                ) {
                    Ok(worker) => self.script_requests = Some(worker),
                    Err(error) => {
                        self.events
                            .push(format!("Script request worker refused: {error}"));
                        self.finish_native_script_request(Completion {
                            epoch: self.script_epoch,
                            request,
                            result: Err(Failure::Storage),
                            _storage: None,
                        });
                        continue;
                    }
                }
            }
            let result = self.script_requests.as_ref().unwrap().submit(
                self.script_epoch,
                request.clone(),
                std::sync::Arc::new(crate::blocking::Policy::new(
                    &self.profile.data().settings.blocking,
                )),
                self.network.clone(),
            );
            if let Err(error) = result {
                self.events
                    .push(format!("Script request queue refused: {error}"));
                self.finish_native_script_request(Completion {
                    epoch: self.script_epoch,
                    request,
                    result: Err(Failure::Storage),
                    _storage: None,
                });
            }
        }
        changed
    }
    pub(super) fn poll_native_script_requests(&mut self) -> bool {
        if self.script_jobs_paused() {
            return false;
        }
        let Some(completion) = self.script_requests.as_ref().and_then(Worker::poll) else {
            return false;
        };
        if completion.epoch != self.script_epoch {
            return false;
        }
        self.finish_native_script_request(completion);
        true
    }
    fn finish_native_script_request(&mut self, completion: Completion) {
        let Some(page) = &mut self.scripting else {
            return;
        };
        let outcome = page.complete_native_request(&mut self.document, &mut self.forms, completion);
        self.apply_history_replacement();
        if outcome.layout_changed {
            self.reflow();
        }
        self.apply_script_scroll();
        self.validate_control_focus();
        self.sync_editors();
        self.show_script_errors();
    }
}
