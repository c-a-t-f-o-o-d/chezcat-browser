//! One browser-owned browsing-history authority and owned UI snapshots.
//! Store remains the sole transaction/file implementation. Views never shut it
//! down; explicit final browser-session shutdown belongs only to Shared.
use super::{ClearRange, Data, Policy, Store};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

const BUSY: &str = "Browsing history is busy. Try the action again.";
const CLOSED: &str = "This browser history session has closed.";
const STALE_ROW: &str = "This history entry changed in another tab or window. Reopen History before removing it.";

#[derive(Debug)]
struct Snapshot {
    data: Data,
    path: Option<PathBuf>,
    problem: Option<String>,
    next_expiration: Option<u64>,
    closed: bool,
}
impl Snapshot {
    fn capture(store:&Store, closed:bool)->Self {
        Self {data:store.data().clone(),path:store.path().map(Path::to_path_buf),
            problem:store.problem().map(str::to_owned),next_expiration:store.next_expiration(),closed}
    }
}

#[derive(Debug)]
struct Authority {
    store: Store,
    current: Rc<Snapshot>,
    closed: bool,
}
impl Authority {
    fn publish(&mut self) {
        if self.current.data!=*self.store.data() || self.current.path.as_deref()!=self.store.path()
            || self.current.problem.as_deref()!=self.store.problem()
            || self.current.next_expiration!=self.store.next_expiration() || self.current.closed!=self.closed {
            self.current=Rc::new(Snapshot::capture(&self.store,self.closed));
        }
    }
}

/// A single browser-session owner. No Drop implementation performs shutdown or
/// saves policy. Do not infer the last window from Rc/Weak reference counts.
#[derive(Debug)]
pub struct Shared { authority:Rc<RefCell<Authority>> }
impl Shared {
    pub fn new(store:Store)->Self {
        let current=Rc::new(Snapshot::capture(&store,false));
        Self {authority:Rc::new(RefCell::new(Authority {store,current,closed:false}))}
    }
    pub fn view(&self)->Result<View,String> {
        let authority=self.authority.try_borrow().map_err(|_|BUSY.to_owned())?;
        if authority.closed {return Err(CLOSED.into());}
        Ok(View {authority:Rc::downgrade(&self.authority),current:authority.current.clone(),access_problem:None,gone:false})
    }
    fn change<T>(&self,action:impl FnOnce(&mut Store)->Result<T,String>)->Result<T,String> {
        let mut authority=self.authority.try_borrow_mut().map_err(|_|BUSY.to_owned())?;
        if authority.closed {return Err(CLOSED.into());}
        let result=action(&mut authority.store);authority.publish();result
    }
    pub fn refresh(&self,now:u64)->Result<bool,String> {self.change(|store|store.refresh(now))}
    /// Explicit browser-owned action only, not a stale page/editor save. This
    /// retains Store::configure semantics. Future settings UI must capture an
    /// editor conflict token before refresh and resolve stale policy explicitly;
    /// this ownership adapter adds no field merging or new conflict guarantee.
    pub fn configure(&self,policy:Policy,now:u64)->Result<bool,String> {self.change(|store|store.configure(policy,now))}
    /// Exactly one successful final shutdown transaction. Failure remains open
    /// and retryable; another call after success performs no Store/file work.
    pub fn shutdown(&self,now:u64)->Result<usize,String> {
        let mut authority=self.authority.try_borrow_mut().map_err(|_|BUSY.to_owned())?;
        if authority.closed {return Ok(0);}
        let result=authority.store.shutdown(now);
        if result.is_ok() {authority.closed=true;}
        authority.publish();result
    }
}

/// An App-local read observation and access handle, not a writer owner. Immutable
/// snapshot data can outlive refresh without retaining RefCell guards. A View
/// drop never expires/clears/writes history or prolongs Store beyond its owner.
#[derive(Clone, Debug)]
pub struct View {
    authority:Weak<RefCell<Authority>>,
    current:Rc<Snapshot>,
    access_problem:Option<String>,
    gone:bool,
}
impl View {
    pub fn data(&self)->&Data {&self.current.data}
    pub fn path(&self)->Option<&Path> {self.current.path.as_deref()}
    pub fn is_closed(&self)->bool {self.gone||self.current.closed}
    pub fn problem(&self)->Option<&str> {
        if self.is_closed() {Some(CLOSED)} else {self.access_problem.as_deref().or(self.current.problem.as_deref())}
    }
    pub fn next_expiration(&self)->Option<u64> {if self.is_closed(){None}else{self.current.next_expiration}}
    fn owner(&mut self)->Result<Rc<RefCell<Authority>>,String> {
        if let Some(authority)=self.authority.upgrade(){return Ok(authority);}
        self.gone=true;self.access_problem=Some(CLOSED.into());Err(CLOSED.into())
    }
    fn observe(&mut self,current:Rc<Snapshot>)->bool {
        let changed=!Rc::ptr_eq(&self.current,&current)||self.access_problem.is_some();
        self.current=current;self.access_problem=None;changed
    }
    /// Pure UI snapshot refresh, with no Store transaction or expiry/file work.
    pub fn sync(&mut self)->Result<bool,String> {
        let owner=self.owner()?;
        let authority=match owner.try_borrow(){Ok(authority)=>authority,Err(_)=>{self.access_problem=Some(BUSY.into());return Err(BUSY.into());}};
        Ok(self.observe(authority.current.clone()))
    }
    fn change<T>(&mut self,action:impl FnOnce(&mut Store)->Result<T,String>)->Result<(T,bool),String> {
        let owner=self.owner()?;
        let mut authority=match owner.try_borrow_mut(){Ok(authority)=>authority,Err(_)=>{self.access_problem=Some(BUSY.into());return Err(BUSY.into());}};
        if authority.closed {
            self.observe(authority.current.clone());self.access_problem=Some(CLOSED.into());return Err(CLOSED.into());
        }
        let result=action(&mut authority.store);authority.publish();let current=authority.current.clone();drop(authority);
        let observed=self.observe(current);
        if let Err(error)=&result {self.access_problem=Some(error.clone());}
        result.map(|value|(value,observed))
    }
    /// Existing Store refresh: reload latest persisted state and expire under
    /// its writer lock. Call sync() instead for a no-I/O activation observation.
    pub fn refresh(&mut self,now:u64)->Result<bool,String> {
        self.change(|store|store.refresh(now)).map(|(changed,observed)|changed||observed)
    }
    pub fn record(&mut self,now:u64,title:&str,url:&str)->Result<Option<u64>,String> {
        self.change(|store|store.record(now,title,url)).map(|(id,_)|id)
    }
    pub fn remove(&mut self,id:u64,now:u64)->Result<bool,String> {
        // Preserve the caller's selected identity before consulting shared state.
        // Peer refresh must not replace this expected row with a newer same-id row.
        let expected=self.current.data.entries.iter().find(|entry|entry.id==id).cloned();
        {
            let owner=self.owner()?;
            let authority=match owner.try_borrow(){Ok(authority)=>authority,Err(_)=>{self.access_problem=Some(BUSY.into());return Err(BUSY.into());}};
            if !authority.closed {
                if let Some(current)=authority.store.data().entries.iter().find(|entry|entry.id==id) {
                    if expected.as_ref()!=Some(current) {
                        // Keep the observed row until explicit sync/reopen; an
                        // error alone must not bless a replacement selection.
                        self.access_problem=Some(STALE_ROW.into());return Err(STALE_ROW.into());
                    }
                }
            }
        }
        // No UI callback runs between the UI-thread check and the transaction.
        // Store still rechecks current disk identity under its writer lock.
        self.change(|store|store.remove(id,now)).map(|(removed,_)|removed)
    }
    pub fn clear(&mut self,range:ClearRange,now:u64)->Result<usize,String> {
        self.change(|store|store.clear(range,now)).map(|(removed,_)|removed)
    }
    // Deliberately no View::configure or View::shutdown. Policy editors and final
    // session lifetime belong to the future browser coordinator, not each page.
}

#[cfg(test)]
#[path = "history_shared_tests.rs"]
mod tests;
