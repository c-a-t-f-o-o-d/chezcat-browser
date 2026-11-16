//! Original two-view controls over real history transactions and owned snapshots.
use super::*;
use super::super::{decode, encode, DAY};
use std::fs;

struct Temp(PathBuf);
impl Temp {
    fn new()->Self {
        let mut bytes=[0u8;8];crate::random::fill(&mut bytes).unwrap();
        let token:String=bytes.iter().map(|b|format!("{b:02x}")).collect();
        let path=std::env::temp_dir().join(format!("chezcat-shared-history-{token}"));
        fs::create_dir(&path).unwrap();Self(fs::canonicalize(path).unwrap())
    }
    fn file(&self)->PathBuf {self.0.join("session/history.txt")}
    fn shared(&self)->Shared {Shared::new(Store::open(self.file()).unwrap())}
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Ok(metadata)=fs::metadata(self.file()) {
            if metadata.permissions().readonly() {let mut permissions=metadata.permissions();permissions.set_readonly(false);let _=fs::set_permissions(self.file(),permissions);}
        }
        let _=fs::remove_dir_all(&self.0);
    }
}
fn enabled()->Policy {Policy {enabled:true,retention_days:None,clear_on_exit:false}}
fn record(view:&mut View,time:u64,page:&str)->u64 {
    view.record(time,page,&format!("https://history.invalid/{page}")).unwrap().unwrap()
}

#[test]
fn shared_history_two_views_record_owned_snapshots_and_reload_latest_policy_and_clears() {
    let temp=Temp::new();let shared=temp.shared();shared.configure(enabled(),1).unwrap();
    let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();
    let one=record(&mut first,2,"one");let held=first.data();
    let two=record(&mut second,3,"two");assert_ne!(one,two);
    assert_eq!(held.entries.len(),1,"Peer changed an owned snapshot or required its borrow guard.");
    assert!(first.sync().unwrap());assert_eq!(first.data().entries.iter().map(|entry|entry.id).collect::<Vec<_>>(),vec![two,one]);
    let mut external=Store::open(temp.file()).unwrap();let mut policy=enabled();policy.enabled=false;external.configure(policy,4).unwrap();
    assert_eq!(first.record(5,"Skipped","https://history.invalid/skipped").unwrap(),None);
    second.sync().unwrap();assert!(!second.data().policy.enabled);assert_eq!(second.data().entries.len(),2);
    external.clear(ClearRange::All,6).unwrap();external.configure(enabled(),7).unwrap();
    let three=record(&mut second,8,"three");assert!(three>two);first.sync().unwrap();
    assert_eq!(first.data().entries.len(),1,"A View resurrected externally cleared visits.");assert_eq!(first.data().entries[0].id,three);
    assert_eq!(Store::open(temp.file()).unwrap().data(),first.data());
}

#[test]
fn shared_history_stale_selected_row_survives_peer_refresh_without_deleting_replacement() {
    let temp=Temp::new();let shared=temp.shared();shared.configure(enabled(),1).unwrap();
    let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();let id=record(&mut first,2,"original");second.sync().unwrap();
    let observed=first.data().entries[0].clone();let mut replacement=Store::open(temp.file()).unwrap().data().clone();
    replacement.entries[0].title="Different recovered row".into();replacement.entries[0].url="https://history.invalid/replaced".into();
    // A valid explicit external replacement reuses the observed numeric id. The
    // full original Entry identity, not just that number, must guard deletion.
    fs::write(temp.file(),encode(&replacement).unwrap()).unwrap();second.refresh(3).unwrap();
    let before=fs::read(temp.file()).unwrap();assert_eq!(first.remove(id,4).unwrap_err(),STALE_ROW);
    assert_eq!(first.data().entries[0],observed,"Refusal silently adopted a replacement selection.");
    assert_eq!(first.remove(id,5).unwrap_err(),STALE_ROW);assert_eq!(fs::read(temp.file()).unwrap(),before);
    first.sync().unwrap();assert_ne!(first.data().entries[0],observed);assert!(first.remove(id,6).unwrap());
    second.sync().unwrap();assert!(second.data().entries.is_empty());assert!(Store::open(temp.file()).unwrap().data().entries.is_empty());
}

#[test]
fn shared_history_expiry_and_delete_preserve_stable_ids_and_future_rows() {
    let shared=Shared::new(Store::memory());shared.configure(enabled(),0).unwrap();
    let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();
    let oldest=record(&mut first,0,"oldest");let middle=record(&mut second,1,"middle");let future=record(&mut first,3*DAY,"future");
    let policy=Policy {retention_days:Some(1),..enabled()};shared.configure(policy,0).unwrap();second.sync().unwrap();
    assert_eq!(second.next_expiration(),Some(DAY));assert!(first.refresh(DAY).unwrap());second.sync().unwrap();
    assert_eq!(second.data().entries.iter().map(|entry|entry.id).collect::<Vec<_>>(),vec![future,middle]);
    assert!(!second.remove(oldest,DAY).unwrap());assert!(first.refresh(DAY+1).unwrap());second.sync().unwrap();
    assert_eq!(second.data().entries.len(),1);assert_eq!(second.data().entries[0].id,future);assert_eq!(second.next_expiration(),Some(4*DAY));
    assert_eq!(first.clear(ClearRange::All,DAY+2).unwrap(),1);let next=record(&mut second,4*DAY,"next");assert!(next>future);
    first.sync().unwrap();assert_eq!(first.data().entries[0].id,next);
}

#[test]
fn shared_history_view_drop_never_clears_and_final_shutdown_transacts_once() {
    let temp=Temp::new();let shared=temp.shared();let mut policy=enabled();policy.clear_on_exit=true;shared.configure(policy,1).unwrap();
    let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();record(&mut first,2,"one");record(&mut second,3,"two");
    let before=fs::read(temp.file()).unwrap();drop(first);assert_eq!(fs::read(temp.file()).unwrap(),before);assert_eq!(Store::open(temp.file()).unwrap().data().entries.len(),2);
    assert_eq!(shared.shutdown(4).unwrap(),2);second.sync().unwrap();assert!(second.is_closed());assert!(second.data().entries.is_empty());assert!(second.next_expiration().is_none());
    assert_eq!(second.record(5,"closed","https://history.invalid/closed").unwrap_err(),CLOSED);
    assert!(shared.configure(enabled(),5).is_err());assert!(shared.view().is_err());
    // A later independent browser may record. A second finalization of this
    // already-closed authority must not run Store::shutdown and clear its visit.
    let mut external=Store::open(temp.file()).unwrap();external.record(6,"Later browser","https://history.invalid/later").unwrap();
    let after=fs::read(temp.file()).unwrap();assert_eq!(shared.shutdown(7).unwrap(),0);assert_eq!(fs::read(temp.file()).unwrap(),after);
    drop(second);drop(shared);assert_eq!(fs::read(temp.file()).unwrap(),after);
}

#[test]
fn shared_history_busy_closed_errors_and_drop_have_no_hidden_save() {
    let temp=Temp::new();let shared=temp.shared();let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();
    let guard=shared.authority.borrow_mut();assert_eq!(first.record(1,"busy","https://history.invalid/busy").unwrap_err(),BUSY);
    assert!(second.sync().is_err());assert!(!temp.file().parent().unwrap().exists());drop(guard);
    let mut policy=enabled();policy.clear_on_exit=true;shared.configure(policy,2).unwrap();record(&mut first,3,"kept");second.sync().unwrap();
    let before=fs::read(temp.file()).unwrap();drop(first);drop(shared);
    assert_eq!(fs::read(temp.file()).unwrap(),before,"Authority Drop performed implicit clear-on-exit.");
    assert_eq!(second.refresh(4).unwrap_err(),CLOSED);assert!(second.is_closed());assert_eq!(second.problem(),Some(CLOSED));assert_eq!(second.next_expiration(),None);
    assert_eq!(second.clear(ClearRange::All,5).unwrap_err(),CLOSED);assert_eq!(fs::read(temp.file()).unwrap(),before);
}

#[test]
fn shared_history_failed_final_shutdown_keeps_data_and_can_explicitly_retry() {
    let temp=Temp::new();let shared=temp.shared();let mut policy=enabled();policy.clear_on_exit=true;shared.configure(policy,1).unwrap();
    let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();record(&mut first,2,"retained");second.sync().unwrap();
    let before=fs::read(temp.file()).unwrap();let data=first.data().clone();let original=fs::metadata(temp.file()).unwrap().permissions();
    let mut readonly=original.clone();readonly.set_readonly(true);fs::set_permissions(temp.file(),readonly).unwrap();
    assert!(shared.shutdown(3).unwrap_err().contains("read-only"));second.sync().unwrap();assert!(!second.is_closed());assert_eq!(second.data(),&data);assert!(second.problem().is_some());assert_eq!(fs::read(temp.file()).unwrap(),before);
    fs::set_permissions(temp.file(),original).unwrap();assert_eq!(shared.shutdown(4).unwrap(),1);second.sync().unwrap();assert!(second.is_closed());assert!(second.data().entries.is_empty());
}

#[test]
fn shared_history_unavailable_corrupt_state_refuses_until_explicit_all_recovery() {
    let temp=Temp::new();fs::create_dir_all(temp.file().parent().unwrap()).unwrap();let bad=b"OwnBrowser history 999\nprivate malformed records\n";fs::write(temp.file(),bad).unwrap();
    let shared=Shared::new(Store::unavailable(temp.file(),"bad file".into()));let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();
    assert!(first.problem().is_some());assert!(first.record(1,"refused","https://history.invalid/refused").is_err());assert!(second.refresh(2).is_err());assert!(shared.configure(enabled(),3).is_err());
    assert!(shared.shutdown(4).is_err());assert!(!shared.authority.borrow().closed);assert_eq!(fs::read(temp.file()).unwrap(),bad);
    assert_eq!(first.clear(ClearRange::All,5).unwrap(),0);second.sync().unwrap();assert!(second.problem().is_none());assert_eq!(second.data().policy,Policy::default());assert!(second.data().entries.is_empty());
    assert_eq!(decode(&fs::read(temp.file()).unwrap()).unwrap(),*second.data());
}

#[test]
fn shared_history_disabled_absent_store_stays_absent_through_views_and_shutdown() {
    let temp=Temp::new();let shared=temp.shared();let mut first=shared.view().unwrap();let mut second=shared.view().unwrap();
    assert_eq!(first.record(1,"Not recorded","https://history.invalid/disabled").unwrap(),None);assert!(!second.refresh(2).unwrap());
    assert_eq!(shared.shutdown(3).unwrap(),0);drop(first);drop(second);drop(shared);assert!(!temp.file().parent().unwrap().exists());
}
