//! Native same-document session entries. No interpreter identities or IO.
//! All data and replacement vector backing are admitted before publication.
use crate::{http::Request, navigation::Location};
use std::mem::size_of;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
pub(crate) struct Shared(Rc<RefCell<Stack>>);
impl Shared {
    pub(crate) fn from_stack(stack: Stack) -> Self {
        Self(Rc::new(RefCell::new(stack)))
    }
    pub(crate) fn len(&self) -> usize {
        self.0.borrow().len()
    }
}
impl std::ops::Deref for Shared {
    type Target = RefCell<Stack>;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
pub(crate) const BYTE_ALLOWANCE: usize = crate::javascript::limits::BROWSER.host.retained_bytes;

#[derive(Clone, Debug, Default)]
pub enum Data {
    #[default]
    Null,
    Undefined,
    Bool(bool),
    Number(f64),
    String(String),
    StringUnits(Vec<u16>),
    BigInt(std::sync::Arc<str>),
}
impl Data {
    pub(crate) fn try_copy(&self) -> Result<Self, String> {
        Ok(match self {
            Self::String(text) => Self::String(copy_text(text)?),
            Self::StringUnits(units) => {
                let mut next = Vec::new();
                next.try_reserve_exact(units.len())
                    .map_err(|_| "Could not reserve session state code units.")?;
                next.extend_from_slice(units);
                Self::StringUnits(next)
            }
            _ => self.clone(),
        })
    }
    pub(crate) fn payload_bytes(&self) -> usize {
        match self {
            Self::String(text) => text.capacity(),
            Self::StringUnits(units) => units.capacity().saturating_mul(2),
            Self::BigInt(text) => text.len(),
            _ => 0,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub location: Location,
    pub scroll: i32,
    pub request: Request,
    pub document: u64,
    pub data: Data,
}
impl Entry {
    pub(crate) fn payload_bytes(&self) -> Result<usize, String> {
        let request = match &self.request {
            Request::Get => 0,
            Request::Script {..}=>return Err("Script subresource requests cannot be stored as session navigation.".into()),
            Request::FormPost { body, origin } => {
                let url = origin.as_ref().map_or(0, |url| {
                    url.host
                        .capacity()
                        .saturating_add(url.path.capacity())
                        .saturating_add(url.query.as_ref().map_or(0, String::capacity))
                });
                body.capacity()
                    .checked_add(url)
                    .ok_or("Session request payload overflow.")?
            }
        };
        self.location
            .payload_bytes()
            .checked_add(request)
            .and_then(|bytes| bytes.checked_add(self.data.payload_bytes()))
            .ok_or_else(|| "Session entry payload overflow.".into())
    }
}

fn copy_text(text: &str) -> Result<String, String> {
    let mut next = String::new();
    next.try_reserve_exact(text.len())
        .map_err(|_| "Could not reserve session request text.")?;
    next.push_str(text);
    Ok(next)
}
pub(crate) fn request_copy_work(request: &Request) -> Result<usize, String> {
    match request {
        Request::Get => Ok(0),
        Request::Script {..}=>Err("Script subresource requests cannot be copied into session navigation.".into()),
        Request::FormPost { body, origin } => {
            let text = origin.as_ref().map_or(0, |url| {
                url.host
                    .len()
                    .saturating_add(url.path.len())
                    .saturating_add(url.query.as_ref().map_or(0, String::len))
            });
            body.len()
                .checked_add(text)
                .ok_or_else(|| "Session request copying overflow.".into())
        }
    }
}
pub(crate) fn copy_request(request: &Request) -> Result<Request, String> {
    Ok(match request {
        Request::Get => Request::Get,
        Request::Script {..}=>return Err("Script subresource requests cannot be replayed as session navigation.".into()),
        Request::FormPost { body, origin } => {
            let mut next = Vec::new();
            next.try_reserve_exact(body.len())
                .map_err(|_| "Could not reserve session POST data.")?;
            next.extend_from_slice(body);
            let origin = origin
                .as_ref()
                .map(|url| {
                    Ok::<_, String>(crate::url::HttpUrl {
                        secure: url.secure,
                        host: copy_text(&url.host)?,
                        port: url.port,
                        path: copy_text(&url.path)?,
                        query: url.query.as_ref().map(|text| copy_text(text)).transpose()?,
                    })
                })
                .transpose()?;
            Request::FormPost { body: next, origin }
        }
    })
}

pub(crate) struct Stack {
    entries: Vec<Entry>,
    index: usize,
    payload: usize,
    limit: usize,
    revision: u64,
}
pub(crate) struct Change {
    revision: u64,
    next_revision: u64,
    keep: usize,
    replace: bool,
    index: usize,
    entry: Entry,
    replacement: Option<Vec<Entry>>,
    payload: usize,
    movement: usize,
    prior_scroll: Option<(usize, i32)>,
}
impl Change {
    pub(crate) fn movement_bytes(&self) -> usize {
        self.movement
    }
    pub(crate) fn preserve_scroll(mut self, index: usize, scroll: i32) -> Self {
        self.prior_scroll = Some((index, scroll));
        self
    }
}
impl Stack {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            entries: Vec::new(),
            index: 0,
            payload: 0,
            limit,
            revision: 0,
        }
    }
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(crate) fn index(&self) -> usize {
        self.index
    }
    pub(crate) fn entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }
    pub(crate) fn current(&self) -> Option<&Entry> {
        self.entry(self.index)
    }
    #[cfg(test)]
    pub(crate) fn storage_bytes(&self) -> usize {
        self.entries.capacity() * size_of::<Entry>() + self.payload
    }
    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }
    #[cfg(test)]
    pub(crate) fn set_limit_for_test(&mut self, limit: usize) {
        self.limit = limit;
    }
    pub(crate) fn prepare(&self, entry: Entry, replace: bool) -> Result<Change, String> {
        self.prepare_at(entry, replace, self.index)
    }
    pub(crate) fn prepare_at(
        &self,
        entry: Entry,
        replace: bool,
        index: usize,
    ) -> Result<Change, String> {
        if !self.entries.is_empty() && index >= self.entries.len() {
            return Err("Session target index is outside the entry list.".into());
        }
        if replace && self.entries.is_empty() {
            return Err("Session replacement requires an active entry.".into());
        }
        let keep = if replace {
            self.entries.len()
        } else if self.entries.is_empty() {
            0
        } else {
            index + 1
        };
        let removed = self.entries[keep..]
            .iter()
            .try_fold(0usize, |bytes, entry| {
                bytes
                    .checked_add(entry.payload_bytes()?)
                    .ok_or_else(|| "Session branch payload overflow.".to_string())
            })?;
        let replaced = if replace {
            self.entries[index].payload_bytes()?
        } else {
            0
        };
        let incoming = entry.payload_bytes()?;
        let payload = self
            .payload
            .checked_sub(removed)
            .and_then(|bytes| bytes.checked_sub(replaced))
            .and_then(|bytes| bytes.checked_add(incoming))
            .ok_or("Session payload accounting overflow.")?;
        let available = self
            .limit
            .checked_sub(payload)
            .ok_or("Session retained entry payload allowance exhausted.")?;
        let required = keep
            .checked_add(usize::from(!replace))
            .ok_or("Session entry count overflow.")?;
        let slot = size_of::<Entry>();
        let before = self.entries.capacity();
        let replacement = if required > before {
            let growth = before
                .max(4)
                .max(required - before)
                .min((available / slot).saturating_sub(before));
            let capacity = before
                .checked_add(growth)
                .filter(|&capacity| capacity >= required && capacity <= available / slot)
                .ok_or("Session retained entry backing allowance exhausted.")?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(capacity)
                .map_err(|_| "Could not reserve session entry backing.")?;
            if entries.capacity() > available / slot {
                return Err("Session allocated backing exceeds retained allowance.".into());
            }
            Some(entries)
        } else {
            if before > available / slot {
                return Err("Session retained entry backing allowance exhausted.".into());
            }
            None
        };
        let movement = if replacement.is_some() {
            self.entries
                .len()
                .checked_mul(slot)
                .ok_or("Session entry movement overflow.")?
        } else {
            0
        };
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or("Session revision identity exhausted.")?;
        Ok(Change {
            revision: self.revision,
            next_revision,
            keep,
            replace,
            index: if replace { index } else { keep },
            entry,
            replacement,
            payload,
            movement,
            prior_scroll: None,
        })
    }
    pub(crate) fn commit(&mut self, mut change: Change) -> Result<(), String> {
        if self.revision != change.revision {
            return Err("Session entry preparation became stale.".into());
        }
        if let Some(mut entries) = change.replacement.take() {
            entries.append(&mut self.entries);
            self.entries = entries;
        }
        if let Some((index, scroll)) = change.prior_scroll {
            if let Some(entry) = self.entries.get_mut(index) {
                entry.scroll = scroll;
            }
        }
        self.entries.truncate(change.keep);
        if change.replace {
            self.entries[change.index] = change.entry;
        } else {
            self.entries.push(change.entry);
        }
        self.index = change.index;
        self.payload = change.payload;
        self.revision = change.next_revision;
        Ok(())
    }
    pub(crate) fn shared(entry: Entry) -> Result<Shared, String> {
        let mut stack = Self::new(BYTE_ALLOWANCE);
        stack.update(entry, false)?;
        Ok(Shared::from_stack(stack))
    }
    pub(crate) fn set_scroll(&mut self, scroll: i32) -> Result<(), String> {
        if self.current().is_none_or(|entry| entry.scroll == scroll) {
            return Ok(());
        }
        let next = self
            .revision
            .checked_add(1)
            .ok_or("Session revision identity exhausted.")?;
        self.entries[self.index].scroll = scroll;
        self.revision = next;
        Ok(())
    }
    pub(crate) fn retag_document(
        &mut self,
        previous: u64,
        next: u64,
        current: &Location,
    ) -> Result<(), String> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Session revision identity exhausted.")?;
        for entry in &mut self.entries {
            let compatible = match (&entry.location.url, &current.url) {
                (Some(a), Some(b)) => a.same_origin(b),
                (None, None) => {
                    entry.location.path == current.path && entry.location.root == current.root
                }
                _ => false,
            };
            if entry.document == previous && compatible {
                entry.document = next;
            }
        }
        self.revision = revision;
        Ok(())
    }
    pub(crate) fn update(&mut self, entry: Entry, replace: bool) -> Result<(), String> {
        let change = self.prepare(entry, replace)?;
        self.commit(change)
    }
    pub(crate) fn traverse(&mut self, index: usize) -> Result<(), String> {
        if index >= self.entries.len() {
            return Err("Session traversal index is outside the entry list.".into());
        }
        let next = self
            .revision
            .checked_add(1)
            .ok_or("Session revision identity exhausted.")?;
        self.index = index;
        self.revision = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(index: usize) -> Entry {
        let location = crate::navigation::address(
            &format!("https://session-entry.test/path?q={index}"),
            &Location::home(),
        )
        .unwrap();
        Entry {
            location,
            scroll: index as i32,
            request: Request::Get,
            document: 7,
            data: Data::String(format!("state-{index}")),
        }
    }
    #[test]
    fn session_entries_exceed_former_count_then_truncate_only_the_forward_branch() {
        let mut stack = Stack::new(crate::javascript::limits::BROWSER.host.retained_bytes);
        for index in 0..300 {
            stack.update(entry(index), false).unwrap();
        }
        assert_eq!((stack.len(), stack.index()), (300, 299));
        assert_eq!(
            stack
                .entry(128)
                .unwrap()
                .location
                .url
                .as_ref()
                .unwrap()
                .query
                .as_deref(),
            Some("q=128")
        );
        assert_eq!(stack.current().unwrap().scroll, 299);
        assert_eq!(stack.current().unwrap().document, 7);
        stack.traverse(100).unwrap();
        stack.update(entry(400), false).unwrap();
        assert_eq!((stack.len(), stack.index()), (102, 101));
        assert_eq!(
            stack
                .entry(100)
                .unwrap()
                .location
                .url
                .as_ref()
                .unwrap()
                .query
                .as_deref(),
            Some("q=100")
        );
        stack.update(entry(500), true).unwrap();
        assert_eq!((stack.len(), stack.index()), (102, 101));
        assert_eq!(
            stack
                .current()
                .unwrap()
                .location
                .url
                .as_ref()
                .unwrap()
                .query
                .as_deref(),
            Some("q=500")
        );
        let payload = stack
            .entries
            .iter()
            .map(|entry| entry.payload_bytes().unwrap())
            .sum::<usize>();
        assert_eq!(
            stack.storage_bytes(),
            stack.entries.capacity() * size_of::<Entry>() + payload
        );
    }
    #[test]
    fn session_entry_exact_backing_and_payload_refusal_preserve_entries_and_recover() {
        let first = entry(0);
        let need = first.payload_bytes().unwrap() + size_of::<Entry>();
        let mut stack = Stack::new(need - 1);
        assert!(stack.update(first, false).is_err());
        assert_eq!((stack.len(), stack.storage_bytes()), (0, 0));
        stack.limit = need;
        stack.update(entry(0), false).unwrap();
        assert_eq!(stack.storage_bytes(), need);
        assert_eq!(stack.entries.capacity(), 1);
        let before = stack.storage_bytes();
        let location = stack.current().unwrap().location.clone();
        assert!(stack.update(entry(1), false).is_err());
        assert_eq!(stack.storage_bytes(), before);
        assert_eq!(stack.current().unwrap().location, location);
        let mut larger = entry(2);
        larger.data = Data::String("large".repeat(500));
        assert!(stack.update(larger, true).is_err());
        assert_eq!(stack.current().unwrap().location, location);
        stack.limit = crate::javascript::limits::BROWSER.host.retained_bytes;
        stack.update(entry(3), false).unwrap();
        assert_eq!((stack.len(), stack.index()), (2, 1));
    }
    #[test]
    fn session_entry_stale_plan_and_post_payload_capacity_never_publish_partial_state() {
        let mut stack = Stack::new(crate::javascript::limits::BROWSER.host.retained_bytes);
        stack.update(entry(0), false).unwrap();
        let mut post = entry(1);
        let mut body = Vec::with_capacity(80);
        body.extend_from_slice(b"q=a");
        post.request = Request::FormPost {
            body,
            origin: post.location.url.clone(),
        };
        let payload = post.payload_bytes().unwrap();
        let staged = stack.prepare(post, false).unwrap();
        assert!(staged.movement_bytes() == 0 || staged.movement_bytes() >= size_of::<Entry>());
        stack.update(entry(2), false).unwrap();
        let before = stack.storage_bytes();
        let current = stack.current().unwrap().location.clone();
        assert!(stack.commit(staged).is_err());
        assert_eq!(stack.storage_bytes(), before);
        assert_eq!(stack.current().unwrap().location, current);
        assert!(payload >= 80);
        assert_eq!(stack.len(), 2);
    }
    #[test]
    fn session_entry_metadata_invalidates_prepared_changes_and_partial_growth_fits_actual_allowance(
    ) {
        let mut stack = Stack::new(BYTE_ALLOWANCE);
        for index in 0..4 {
            stack.update(entry(index), false).unwrap();
        }
        let next = entry(4);
        let total = stack.payload + next.payload_bytes().unwrap() + 6 * size_of::<Entry>();
        stack.limit = total;
        stack.update(next, false).unwrap();
        assert_eq!(stack.entries.capacity(), 6);
        assert_eq!(stack.storage_bytes(), total);
        let stale = stack.prepare(entry(5), true).unwrap();
        stack.set_scroll(123).unwrap();
        assert!(stack.commit(stale).is_err());
        assert_eq!(stack.current().unwrap().scroll, 123);
        let stale = stack.prepare(entry(5), true).unwrap();
        let location = stack.current().unwrap().location.clone();
        stack.retag_document(7, 8, &location).unwrap();
        assert!(stack.commit(stale).is_err());
        assert_eq!(stack.current().unwrap().document, 8);
    }
}

#[cfg(test)]
#[test]
fn script_subresource_requests_are_never_copied_or_replayed_as_navigation() {
    let origin=crate::url::parse("https://owned-script.test/source").unwrap().0;
    let request=Request::Script {origin,request:crate::http::ScriptRequest {method:crate::http::ScriptMethod::Post,headers:Vec::new(),body:b"original body".to_vec()}};
    assert!(request_copy_work(&request).is_err());
    assert!(copy_request(&request).is_err());
}
