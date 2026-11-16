//! UI-thread shared profile authority with owned, revisioned read snapshots.
//! The existing Store remains the only filesystem writer. No borrow guard or
//! callback crosses this interface; refreshing a View performs no page work.
use super::{encode, normalize, Data, Store};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const BUSY: &str = "Preferences are busy. Try the action again.";
const STALE: &str = "Settings or bookmarks changed in another tab or window. Reopen this editor before saving.";

#[derive(Debug)]
pub struct Snapshot {
    revision: u64,
    data: Data,
    path: Option<PathBuf>,
    problem: Option<String>,
}

impl Snapshot {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn data(&self) -> &Data { &self.data }
    pub fn path(&self) -> Option<&Path> { self.path.as_deref() }
    pub fn problem(&self) -> Option<&str> { self.problem.as_deref() }
}

#[derive(Debug)]
struct Authority {
    store: Store,
    current: Rc<Snapshot>,
}

/// A UI-thread handle to one Store. Worker threads and page scripts do not own
/// this authority. Cloning the handle does not reopen a file or copy a writer.
#[derive(Clone, Debug)]
pub struct Shared {
    authority: Rc<RefCell<Authority>>,
}

impl Shared {
    pub fn new(store: Store) -> Self {
        Self::initial(store).0
    }

    fn initial(store: Store) -> (Self, Rc<Snapshot>) {
        let current = Rc::new(Snapshot {
            revision: 0,
            data: store.data().clone(),
            path: store.path().map(Path::to_path_buf),
            problem: store.problem().map(str::to_owned),
        });
        let shared = Self { authority: Rc::new(RefCell::new(Authority {
            store, current: Rc::clone(&current),
        })) };
        (shared, current)
    }

    /// Return an owned immutable value; the RefCell guard ends in this method.
    pub fn snapshot(&self) -> Result<Rc<Snapshot>, String> {
        let authority = self.authority.try_borrow().map_err(|_| BUSY.to_owned())?;
        Ok(Rc::clone(&authority.current))
    }

    pub fn view(&self) -> Result<View, String> {
        Ok(View { shared: self.clone(), current: self.snapshot()? })
    }

    /// Commit the caller's staged value only against its captured revision.
    /// Refusal preserves the Store, its file and every issued snapshot.
    pub fn save_at(&self, expected_revision: u64, mut next: Data) -> Result<Rc<Snapshot>, String> {
        let mut authority = self.authority.try_borrow_mut().map_err(|_| BUSY.to_owned())?;
        if expected_revision != authority.current.revision {
            return Err(STALE.to_owned());
        }
        // Match Store's validation and complete encoded-size admission before
        // preparing any changed snapshot or allowing filesystem publication.
        normalize(&mut next)?;
        drop(encode(&next)?);
        if next == *authority.store.data() {
            // Store deliberately rejects unavailable profiles even on a no-op.
            // A valid no-op neither touches disk nor consumes a revision.
            authority.store.save(next)?;
            return Ok(Rc::clone(&authority.current));
        }
        let revision = authority.current.revision.checked_add(1)
            .ok_or("No profile revisions remain.")?;
        // Every allocation needed to publish the immutable snapshot precedes
        // Store's disk write. Nothing fallible follows a successful save.
        let prepared = Rc::new(Snapshot {
            revision,
            data: next.clone(),
            path: authority.current.path.clone(),
            problem: authority.current.problem.clone(),
        });
        authority.store.save(next)?;
        authority.current = prepared;
        Ok(Rc::clone(&authority.current))
    }
}

impl Default for Shared {
    fn default() -> Self { Self::new(Store::memory()) }
}

/// An App-owned read snapshot. Reads never hold a dynamic borrow. Explicit
/// refresh changes only this cache; it does not attach/reset an App profile.
#[derive(Clone, Debug)]
pub struct View {
    shared: Shared,
    current: Rc<Snapshot>,
}

impl View {
    pub fn data(&self) -> &Data { self.current.data() }
    pub fn path(&self) -> Option<&Path> { self.current.path() }
    pub fn problem(&self) -> Option<&str> { self.current.problem() }
    pub fn revision(&self) -> u64 { self.current.revision() }
    pub fn shared(&self) -> Shared { self.shared.clone() }
    pub fn snapshot(&self) -> Rc<Snapshot> { Rc::clone(&self.current) }

    pub fn refresh(&mut self) -> Result<bool, String> {
        let next = self.shared.snapshot()?;
        let changed = !Rc::ptr_eq(&self.current, &next);
        self.current = next;
        Ok(changed)
    }

    /// Immediate commands stage against this View's current snapshot. Editors
    /// must instead retain their original token and call save_at explicitly.
    pub fn save(&mut self, next: Data) -> Result<(), String> {
        self.save_at(self.current.revision, next)
    }

    pub fn save_at(&mut self, expected_revision: u64, next: Data) -> Result<(), String> {
        let committed = self.shared.save_at(expected_revision, next)?;
        self.current = committed;
        Ok(())
    }
}

impl From<Store> for View {
    fn from(store: Store) -> Self {
        let (shared, current) = Shared::initial(store);
        Self { shared, current }
    }
}

impl Default for View {
    fn default() -> Self { Store::memory().into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Explicit cleanup keeps all test filesystem effects visible. Production
    // Shared/View/Snapshot have no Drop implementation or shutdown disk I/O.
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("chezcat-shared-profile-{}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf { self.0.join("profile.txt") }
        fn finish(self) {
            if self.file().exists() { fs::remove_file(self.file()).unwrap(); }
            fs::remove_dir(self.0).unwrap();
        }
    }

    #[test]
    fn shared_profile_stale_editor_stays_stale_after_view_refresh() {
        let shared = Shared::default();
        let mut first = shared.view().unwrap();
        let mut second = shared.view().unwrap();
        let editor_revision = first.revision();
        let mut draft = first.data().clone();
        draft.settings.javascript = false;
        let mut peer = second.data().clone();
        peer.settings.dark_mode = true;
        second.save(peer.clone()).unwrap();
        assert_eq!(second.revision(), 1);
        assert!(!first.data().settings.dark_mode);
        assert!(first.refresh().unwrap());
        assert_eq!(first.data(), &peer);
        assert!(first.save_at(editor_revision, draft).unwrap_err().contains("changed"));
        assert_eq!(first.data(), &peer);
        // An identical payload does not turn an old editor token into permission.
        assert!(first.save_at(editor_revision, peer.clone()).is_err());
        let mut fresh = first.data().clone();
        fresh.settings.javascript = false;
        first.save(fresh.clone()).unwrap();
        assert_eq!(first.revision(), 2);
        assert_eq!(shared.snapshot().unwrap().data(), &fresh);
        assert_eq!(second.data(), &peer);
        assert!(second.refresh().unwrap());
        assert_eq!(second.data(), &fresh);
    }

    #[test]
    fn shared_profile_issued_snapshots_remain_owned_and_immutable() {
        let shared = Shared::default();
        let mut view = shared.view().unwrap();
        let old = view.snapshot();
        let mut changed = view.data().clone();
        changed.settings.default_zoom = 125;
        view.save(changed.clone()).unwrap();
        let current = shared.snapshot().unwrap();
        assert!(!Rc::ptr_eq(&old, &current));
        assert_eq!(old.revision(), 0);
        assert_eq!(old.data().settings.default_zoom, 100);
        assert_eq!(current.revision(), 1);
        assert_eq!(current.data(), &changed);
        assert!(!view.refresh().unwrap());
        // Owning old snapshots does not hold a borrow or block another commit.
        changed.settings.default_zoom = 150;
        view.save(changed).unwrap();
        assert_eq!(old.data().settings.default_zoom, 100);
    }

    #[test]
    fn shared_profile_busy_refusal_preserves_view_and_recovers() {
        let shared = Shared::default();
        let mut view = shared.view().unwrap();
        let old = view.snapshot();
        let mut next = view.data().clone();
        next.settings.default_zoom = 125;
        let busy = shared.authority.try_borrow_mut().unwrap();
        assert!(shared.snapshot().unwrap_err().contains("busy"));
        assert!(shared.view().unwrap_err().contains("busy"));
        assert!(view.refresh().unwrap_err().contains("busy"));
        assert!(view.save(next.clone()).unwrap_err().contains("busy"));
        assert!(Rc::ptr_eq(&old, &view.snapshot()));
        drop(busy);
        view.save(next.clone()).unwrap();
        assert_eq!(view.revision(), 1);
        assert_eq!(shared.snapshot().unwrap().data(), &next);
    }

    #[test]
    fn shared_profile_revision_exhaustion_precedes_any_changed_file_write() {
        let fixture = Fixture::new();
        let shared = Shared::new(Store::open(fixture.file()).unwrap());
        let mut view = shared.view().unwrap();
        let mut initial = view.data().clone();
        initial.settings.default_zoom = 125;
        view.save(initial.clone()).unwrap();
        let bytes = fs::read(fixture.file()).unwrap();
        {
            let mut authority = shared.authority.try_borrow_mut().unwrap();
            let prepared = Snapshot {
                revision: u64::MAX,
                data: authority.current.data.clone(),
                path: authority.current.path.clone(),
                problem: authority.current.problem.clone(),
            };
            authority.current = Rc::new(prepared);
        }
        view.refresh().unwrap();
        let before = view.snapshot();
        let mut next = initial.clone();
        next.settings.default_zoom = 150;
        assert!(view.save(next).unwrap_err().contains("revisions"));
        assert_eq!(view.revision(), u64::MAX);
        assert_eq!(view.data(), &initial);
        assert!(Rc::ptr_eq(&before, &shared.snapshot().unwrap()));
        assert_eq!(fs::read(fixture.file()).unwrap(), bytes);
        view.save(initial).unwrap();
        assert_eq!(view.revision(), u64::MAX);
        assert_eq!(fs::read(fixture.file()).unwrap(), bytes);
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
        fixture.finish();
    }

    #[test]
    fn shared_profile_unavailable_noop_is_refused_without_creating_a_writer() {
        let fixture = Fixture::new();
        let bytes = b"Unavailable original profile bytes";
        fs::write(fixture.file(), bytes).unwrap();
        let reason = "Original profile could not be decoded.";
        let mut view = View::from(Store::unavailable(fixture.file(), reason.into()));
        let old = view.snapshot();
        assert_eq!(view.path(), Some(fixture.file().as_path()));
        assert_eq!(view.problem(), Some(reason));
        assert_eq!(view.data().settings.search_engine, super::super::SearchEngine::None);
        assert!(view.save(view.data().clone()).unwrap_err().contains("read-only"));
        assert_eq!(view.revision(), 0);
        assert!(Rc::ptr_eq(&old, &view.snapshot()));
        assert_eq!(fs::read(fixture.file()).unwrap(), bytes);
        fixture.finish();
    }

    #[test]
    fn shared_profile_valid_noop_keeps_revision_and_missing_file_absent() {
        let fixture = Fixture::new();
        let mut view = View::from(Store::open(fixture.file()).unwrap());
        let old = view.snapshot();
        view.save(view.data().clone()).unwrap();
        assert_eq!(view.revision(), 0);
        assert!(Rc::ptr_eq(&old, &view.snapshot()));
        assert!(!fixture.file().exists());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
        fixture.finish();
    }

    #[test]
    fn shared_profile_changed_readonly_save_keeps_file_and_revision() {
        let fixture = Fixture::new();
        let mut view = View::from(Store::open(fixture.file()).unwrap());
        let mut initial = view.data().clone();
        initial.settings.default_zoom = 125;
        view.save(initial.clone()).unwrap();
        let old = view.snapshot();
        let bytes = fs::read(fixture.file()).unwrap();
        let permissions = fs::metadata(fixture.file()).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(fixture.file(), readonly).unwrap();
        let mut next = initial.clone();
        next.settings.default_zoom = 150;
        let changed = view.save(next);
        // Retain Store's existing valid-noop behavior on a read-only file.
        let noop = view.save(initial.clone());
        fs::set_permissions(fixture.file(), permissions).unwrap();
        assert!(changed.unwrap_err().contains("read-only"));
        noop.unwrap();
        assert_eq!(view.revision(), 1);
        assert_eq!(view.data(), &initial);
        assert!(Rc::ptr_eq(&old, &view.snapshot()));
        assert_eq!(fs::read(fixture.file()).unwrap(), bytes);
        fixture.finish();
    }

    #[test]
    fn shared_profile_external_disk_conflict_does_not_refresh_or_publish() {
        let fixture = Fixture::new();
        let shared = Shared::new(Store::open(fixture.file()).unwrap());
        let mut first = shared.view().unwrap();
        let mut second = shared.view().unwrap();
        let mut initial = first.data().clone();
        initial.settings.default_zoom = 125;
        first.save(initial.clone()).unwrap();
        second.refresh().unwrap();
        let old = second.snapshot();
        let external = b"The file was replaced by an independent owner.";
        fs::write(fixture.file(), external).unwrap();
        let mut next = initial.clone();
        next.settings.default_zoom = 150;
        assert!(second.save(next).unwrap_err().contains("changed on disk"));
        assert_eq!(second.revision(), 1);
        assert_eq!(second.data(), &initial);
        assert!(Rc::ptr_eq(&old, &shared.snapshot().unwrap()));
        assert_eq!(fs::read(fixture.file()).unwrap(), external);
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
        fixture.finish();
    }

    #[test]
    fn shared_profile_invalid_and_encoded_oversize_values_never_publish() {
        let fixture = Fixture::new();
        let shared = Shared::new(Store::open(fixture.file()).unwrap());
        let mut view = shared.view().unwrap();
        let old = view.snapshot();
        let mut invalid = view.data().clone();
        invalid.settings.default_zoom = 301;
        assert!(view.save(invalid).unwrap_err().contains("zoom"));
        let mut oversized = view.data().clone();
        for index in 0..72 {
            oversized.bookmarks.push(super::super::Bookmark {
                title: format!("Original bookmark {index}"),
                url: format!("https://example.com/{}?item={index}", "a".repeat(15_000)),
                folder: 0,
                order: index,
            });
        }
        assert!(view.save(oversized).unwrap_err().contains("1 MiB"));
        assert_eq!(view.revision(), 0);
        assert_eq!(view.data(), &Data::default());
        assert!(Rc::ptr_eq(&old, &shared.snapshot().unwrap()));
        assert!(!fixture.file().exists());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
        fixture.finish();
    }
}
