//! Bounded UI-thread browser ownership core. No Win32/tab-strip/link UI lives
//! here. Page workers can enqueue typed identities only, never App pointers.
use super::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, Duration};

// A checked process generation prevents retained IDs from an old/parallel
// Session from matching a different Session's local serials. IDs are ephemeral.
static NEXT_SESSION:AtomicU64=AtomicU64::new(0);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabId {generation:u64,serial:u64}
impl TabId {
    /// Diagnostic local serial only; routing must retain the complete typed ID.
    pub fn value(self)->u64{self.serial}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowId {generation:u64,serial:u64}
impl WindowId {
    /// Diagnostic local serial only; routing must retain the complete typed ID.
    pub fn value(self)->u64{self.serial}
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {pub tabs:usize,pub windows:usize,pub wakes:usize,pub poll_tabs:usize}
impl Default for Limits {fn default()->Self{Self{tabs:16,windows:4,wakes:128,poll_tabs:4}}}
impl Limits {
    fn check(self)->Result<Self,String> {
        if !(1..=32).contains(&self.tabs)||!(1..=8).contains(&self.windows)
            ||!(1..=256).contains(&self.wakes)||self.poll_tabs==0||self.poll_tabs>self.tabs {
            return Err("Invalid browser owner/work limits.".into());
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {Tab(WindowId,TabId),Window(WindowId),Downloads}
struct Messages {queue:VecDeque<Event>,limit:usize,overflow:bool,closed:bool}
#[derive(Clone)]
struct Mailbox(Arc<Mutex<Messages>>);
impl Mailbox {
    fn new(limit:usize)->Result<Self,String> {
        let mut queue=VecDeque::new();queue.try_reserve_exact(limit).map_err(|_|"Cannot reserve browser wake routing.")?;
        Ok(Self(Arc::new(Mutex::new(Messages{queue,limit,overflow:false,closed:false}))))
    }
    fn send(&self,event:Event) {
        let Ok(mut state)=self.0.lock() else{return;};
        if state.closed||state.queue.contains(&event){return;}
        if state.queue.len()==state.limit {state.overflow=true;return;}
        state.queue.push_back(event);
    }
    fn wake(&self,event:Event)->loader::Wake {let mailbox=self.clone();Arc::new(move||mailbox.send(event))}
    fn drain(&self)->Result<(Vec<Event>,bool),String> {
        let mut state=self.0.lock().map_err(|_|"Browser wake routing is unavailable.")?;
        let mut events=Vec::new();events.try_reserve_exact(state.queue.len()).map_err(|_|"Cannot retain browser wake batch.")?;
        events.extend(state.queue.drain(..));let overflow=std::mem::replace(&mut state.overflow,false);Ok((events,overflow))
    }
    fn pending(&self)->bool {self.0.lock().map_or(false,|state|!state.queue.is_empty()||state.overflow)}
    fn close(&self){if let Ok(mut state)=self.0.lock(){state.closed=true;state.queue.clear();state.overflow=false;}}
}
struct Tab {id:TabId,window:WindowId,app:Box<App>}
struct Window {id:WindowId,tabs:Vec<TabId>,active:Option<TabId>,width:i32,height:i32}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {Running,Finalizing,Closed}
#[derive(Default, Debug)]
pub struct Poll {pub changed:Vec<TabId>,pub stale_wakes:usize,pub downloads_changed:bool,pub more:bool}
#[derive(Default, Debug)]
pub struct Shutdown {pub tabs:usize,pub windows:usize,pub history_removed:usize}

/// One browser-session authority and bounded collections of real page owners.
/// References returned by with_tab/with_active are scoped to the callback; no
/// platform userdata or worker may retain a raw App pointer after that callback.
pub struct Session {
    tabs:Vec<Tab>,windows:Vec<Window>,generation:u64,next_tab:u64,next_window:u64,
    cursor:usize,sweep:usize,limits:Limits,phase:Phase,mailbox:Mailbox,
    profile:crate::profile::Shared,downloads:Option<crate::downloads::Shared>,history:crate::history::Shared,
    cookies:Arc<crate::cookies::Session>,trust:Arc<crate::trust::Store>,
    base_fonts:crate::fonts::Book,home:Location,
}
impl Session {
    fn running(&self)->Result<(),String>{if self.phase==Phase::Running{Ok(())}else{Err("This browser session is closing or closed.".into())}}
    /// Adopt the legacy initial App without reattaching observations, reopening
    /// files or moving a live Manager/Store. Failure returns the untouched App.
    pub fn adopt(mut app:Box<App>,limits:Limits)->Result<Self,(Box<App>,String)> {
        let limits=match limits.check(){Ok(limits)=>limits,Err(error)=>return Err((app,error))};
        if app.owned_downloads.is_none()||app.owned_history.is_none()||app.history_closed||app.browsing_history.is_closed(){
            return Err((app,"The initial App must own its live standalone browser services.".into()));
        }
        let mailbox=match Mailbox::new(limits.wakes){Ok(mailbox)=>mailbox,Err(error)=>return Err((app,error))};
        let (mut tabs,mut windows,mut window_tabs)=(Vec::new(),Vec::new(),Vec::new());
        if tabs.try_reserve_exact(limits.tabs).is_err()||windows.try_reserve_exact(limits.windows).is_err()
            ||window_tabs.try_reserve_exact(limits.tabs).is_err(){return Err((app,"Cannot reserve browser owner storage.".into()));}
        let generation=match NEXT_SESSION.try_update(Ordering::Relaxed,Ordering::Relaxed,|current|current.checked_add(1)) {
            Ok(previous)=>previous+1,Err(_)=>return Err((app,"Browser session identity exhausted.".into())),
        };
        let tab=TabId{generation,serial:1};let window=WindowId{generation,serial:1};
        let download_wake=mailbox.wake(Event::Downloads);
        if let Err(error)=app.owned_downloads.as_ref().unwrap().set_wake(Some(download_wake)){return Err((app,error));}
        // All fallible preflights are done. Existing Weak views retain exactly
        // their old authority; taking these outer owners does not move a Manager.
        let downloads=app.owned_downloads.take().unwrap();let history=app.owned_history.take().unwrap();
        let profile=app.profile.shared();let cookies=app.cookies.clone();let trust=app.trust.clone();
        let base_fonts=app.base_fonts.clone();let home=app.home_location.clone();
        app.set_load_wake(mailbox.wake(Event::Tab(window,tab)));
        window_tabs.push(tab);windows.push(Window{id:window,tabs:window_tabs,active:Some(tab),width:app.width,height:app.height});
        tabs.push(Tab{id:tab,window,app});
        Ok(Self{tabs,windows,generation,next_tab:1,next_window:1,cursor:0,sweep:0,limits,phase:Phase::Running,mailbox,
            profile,downloads:Some(downloads),history,cookies,trust,base_fonts,home})
    }
    pub fn tab_count(&self)->usize{self.tabs.len()}
    pub fn window_count(&self)->usize{self.windows.len()}
    pub fn window_ids(&self)->impl Iterator<Item=WindowId>+'_ {self.windows.iter().map(|window|window.id)}
    fn window_index(&self,id:WindowId)->Result<usize,String>{self.windows.iter().position(|window|window.id==id).ok_or_else(||"This browser window has closed.".into())}
    fn tab_index(&self,id:TabId)->Result<usize,String>{self.tabs.iter().position(|tab|tab.id==id).ok_or_else(||"This browser tab has closed.".into())}
    pub fn window_tabs(&self,id:WindowId)->Result<&[TabId],String>{Ok(&self.windows[self.window_index(id)?].tabs)}
    pub fn active(&self,id:WindowId)->Result<Option<TabId>,String>{Ok(self.windows[self.window_index(id)?].active)}
    /// Owned read observations only: rendering/title/cursor queries enqueue no
    /// work and expose no retained App reference to the native message layer.
    pub fn observe_tab<T>(&self,id:TabId,observe:impl FnOnce(&App)->T)->Result<T,String>{
        self.running()?;let index=self.tab_index(id)?;Ok(observe(&self.tabs[index].app))
    }
    pub fn observe_active<T>(&self,id:WindowId,observe:impl FnOnce(&App)->T)->Result<T,String>{
        self.running()?;let tab=self.active(id)?.ok_or("This window has no active tab.")?;self.observe_tab(tab,observe)
    }
    pub fn with_tab<T>(&mut self,id:TabId,action:impl FnOnce(&mut App)->T)->Result<T,String>{
        self.running()?;let index=self.tab_index(id)?;let result=action(&mut self.tabs[index].app);
        self.mailbox.send(Event::Tab(self.tabs[index].window,id));Ok(result)
    }
    pub fn with_active<T>(&mut self,id:WindowId,action:impl FnOnce(&mut App)->T)->Result<T,String>{
        self.running()?;let tab=self.active(id)?.ok_or("This window has no active tab.")?;self.with_tab(tab,action)
    }
    pub fn tab_wake(&self,id:TabId)->Result<loader::Wake,String>{self.running()?;let tab=&self.tabs[self.tab_index(id)?];Ok(self.mailbox.wake(Event::Tab(tab.window,id)))}
    pub fn window_wake(&self,id:WindowId)->Result<loader::Wake,String>{self.running()?;self.window_index(id)?;Ok(self.mailbox.wake(Event::Window(id)))}

    fn fresh_app(&self,window:WindowId,tab:TabId,width:i32,height:i32)->Result<Box<App>,String>{
        // Acquire every observation before allocating/publishing a page owner.
        let downloads=self.downloads.as_ref().ok_or("The browser download authority has closed.")?.view()?;
        let history=self.history.view()?;let commands=self.history.commands();
        let mut app=Box::new(App::new());
        app.attach_shared_profile(self.profile.clone())?;
        // Fresh default authorities have no worker, visit or configured policy.
        // Replace only those empty owners; no live Manager/Store moves here.
        app.downloads=downloads;app.owned_downloads=None;
        app.browsing_history=history;app.history_commands=commands;app.owned_history=None;app.history_closed=false;
        app.cookies=self.cookies.clone();app.trust=self.trust.clone();
        app.base_fonts=self.base_fonts.clone();app.fonts=self.base_fonts.clone();app.home_location=self.home.clone();
        app.set_load_wake(self.mailbox.wake(Event::Tab(window,tab)));
        app.width=width;app.height=height;app.reflow();app.sync_editors();
        // Pure observation: readonly/unavailable history must not trigger an
        // expiry/file transaction as a side effect of atomic tab admission.
        app.sync_shared_browsing_history()?;
        Ok(app)
    }
    pub fn new_tab(&mut self,window:WindowId)->Result<TabId,String>{
        self.running()?;let index=self.window_index(window)?;
        if self.tabs.len()>=self.limits.tabs{return Err("The browser tab limit was reached.".into());}
        let next=self.next_tab.checked_add(1).ok_or("Browser tab identity exhausted.")?;let tab=TabId{generation:self.generation,serial:next};
        // The global owner Vec and this window's list were bounded/reserved.
        self.windows[index].tabs.try_reserve(1).map_err(|_|"Cannot reserve window tab membership.")?;
        let app=self.fresh_app(window,tab,self.windows[index].width,self.windows[index].height)?;
        if let Some(old)=self.windows[index].active {let old_index=self.tab_index(old)?;self.tabs[old_index].app.invalidate_platform_requests();self.tabs[old_index].app.pointer_cancel();self.tabs[old_index].app.pending_surrogate=None;}
        self.next_tab=next;self.tabs.push(Tab{id:tab,window,app});self.windows[index].tabs.push(tab);self.windows[index].active=Some(tab);
        self.mailbox.send(Event::Tab(window,tab));Ok(tab)
    }
    pub fn new_window(&mut self,width:i32,height:i32)->Result<(WindowId,TabId),String>{
        self.running()?;
        if self.windows.len()>=self.limits.windows||self.tabs.len()>=self.limits.tabs{return Err("The browser window or tab limit was reached.".into());}
        let next_window=self.next_window.checked_add(1).ok_or("Browser window identity exhausted.")?;
        let next_tab=self.next_tab.checked_add(1).ok_or("Browser tab identity exhausted.")?;
        let window=WindowId{generation:self.generation,serial:next_window};let tab=TabId{generation:self.generation,serial:next_tab};let (width,height)=(width.clamp(1,8192),height.clamp(1,8192));
        let mut members=Vec::new();members.try_reserve_exact(self.limits.tabs).map_err(|_|"Cannot reserve new window tabs.")?;
        let app=self.fresh_app(window,tab,width,height)?;
        self.next_window=next_window;self.next_tab=next_tab;members.push(tab);
        self.windows.push(Window{id:window,tabs:members,active:Some(tab),width,height});self.tabs.push(Tab{id:tab,window,app});
        self.mailbox.send(Event::Tab(window,tab));Ok((window,tab))
    }
    fn prepare_activation(&mut self,wi:usize,tab:TabId)->Result<(),String>{
        let ti=self.tab_index(tab)?;
        if self.tabs[ti].window!=self.windows[wi].id{return Err("The selected tab belongs to another window.".into());}
        let app=&mut self.tabs[ti].app;app.refresh_profile()?;app.downloads.refresh()?;app.sync_shared_browsing_history()?;
        app.resize(self.windows[wi].width,self.windows[wi].height);Ok(())
    }
    pub fn switch_tab(&mut self,window:WindowId,tab:TabId)->Result<(),String>{
        self.running()?;let wi=self.window_index(window)?;self.prepare_activation(wi,tab)?;
        if self.windows[wi].active!=Some(tab) {if let Some(old)=self.windows[wi].active {let old_index=self.tab_index(old)?;self.tabs[old_index].app.invalidate_platform_requests();self.tabs[old_index].app.pointer_cancel();self.tabs[old_index].app.pending_surrogate=None;}}
        self.windows[wi].active=Some(tab);Ok(())
    }
    pub fn resize_window(&mut self,id:WindowId,width:i32,height:i32)->Result<(),String>{
        self.running()?;let index=self.window_index(id)?;let (width,height)=(width.clamp(1,8192),height.clamp(1,8192));
        self.windows[index].width=width;self.windows[index].height=height;
        if let Some(tab)=self.windows[index].active {let ti=self.tab_index(tab)?;self.tabs[ti].app.resize(width,height);}
        Ok(())
    }
    pub fn close_tab(&mut self,id:TabId)->Result<(),String>{
        self.running()?;let ti=self.tab_index(id)?;let wi=self.window_index(self.tabs[ti].window)?;
        let position=self.windows[wi].tabs.iter().position(|tab|*tab==id).ok_or("Browser tab routing is inconsistent.")?;
        let replacement=if self.windows[wi].active==Some(id) {
            self.windows[wi].tabs.get(position+1).or_else(||position.checked_sub(1).and_then(|previous|self.windows[wi].tabs.get(previous))).copied()
        }else{None};
        // A failing activation observation leaves the requested close uncommitted.
        // Prepare the real survivor through the same path as explicit switching.
        if let Some(replacement)=replacement{self.prepare_activation(wi,replacement)?;}
        // Remove every identity route before consuming/dropping the Box<App>.
        let tab=self.tabs.remove(ti);self.windows[wi].tabs.remove(position);
        if self.windows[wi].active==Some(id){self.windows[wi].active=replacement;}
        self.cursor%=self.tabs.len().max(1);self.sweep=self.sweep.min(self.tabs.len());
        tab.app.retire_from_session();Ok(())
    }
    pub fn close_window(&mut self,id:WindowId)->Result<(),String>{
        self.running()?;let wi=self.window_index(id)?;let mut closing=Vec::new();
        closing.try_reserve_exact(self.windows[wi].tabs.len()).map_err(|_|"Cannot retain closing window owners.")?;
        self.windows.remove(wi);
        let mut index=0;while index<self.tabs.len(){if self.tabs[index].window==id{closing.push(self.tabs.remove(index));}else{index+=1;}}
        // Every window/tab route is gone before the first page is consumed.
        self.cursor%=self.tabs.len().max(1);self.sweep=self.sweep.min(self.tabs.len());
        for tab in closing{tab.app.retire_from_session();}Ok(())
    }
    /// Round-robin visits bound owner work per call. Unserved typed wakes remain
    /// queued; overflow requests a finite sweep, never unbounded mailbox growth.
    pub fn poll(&mut self,budget:usize)->Result<Poll,String>{
        self.running()?;if budget==0{return Err("Browser poll budget must be positive.".into());}
        let (events,overflow)=self.mailbox.drain()?;if overflow{self.sweep=self.tabs.len();}
        let mut report=Poll::default();let mut pending=Vec::new();pending.try_reserve_exact(events.len()).map_err(|_|"Cannot retain routed browser wakes.")?;
        for event in events {match event {
            Event::Tab(window,tab)=>{if self.tabs.iter().any(|owner|owner.id==tab&&owner.window==window){pending.push((window,tab));}else{report.stale_wakes+=1;}},
            Event::Window(window)=>{if self.window_index(window).is_err(){report.stale_wakes+=1;}},
            Event::Downloads=>{},
        }}
        if let Some(downloads)=&self.downloads{report.downloads_changed=downloads.poll()?;}
        let count=budget.min(self.limits.poll_tabs).min(self.tabs.len());report.changed.try_reserve_exact(count).map_err(|_|"Cannot retain browser poll results.")?;
        let mut served=Vec::new();served.try_reserve_exact(count).map_err(|_|"Cannot retain visited tab identities.")?;
        for _ in 0..count {
            self.cursor%=self.tabs.len();let tab=&mut self.tabs[self.cursor];self.cursor+=1;served.push(tab.id);
            let changed=tab.app.poll_load()|tab.app.poll_page_timer()|tab.app.poll_history_maintenance();
            if changed{report.changed.push(tab.id);}
        }
        self.sweep=self.sweep.saturating_sub(count);
        for (window,tab) in pending{if !served.contains(&tab){self.mailbox.send(Event::Tab(window,tab));}}
        // Timers in an owner not visited this turn must get another bounded
        // scheduling turn even when its old network wake was coalesced/dropped.
        let now=Instant::now();for tab in &self.tabs{if !served.contains(&tab.id)&&tab.app.next_page_timer().is_some_and(|due|due<=now){self.mailbox.send(Event::Tab(tab.window,tab.id));}}
        report.more=self.sweep!=0||self.mailbox.pending();Ok(report)
    }
    pub fn next_deadline(&self)->Option<Instant>{
        if self.phase!=Phase::Running{return None;}
        let page=self.tabs.iter().flat_map(|tab|[tab.app.next_page_timer(),tab.app.next_history_maintenance()]).flatten().min();
        let pending=self.mailbox.pending()||self.sweep!=0;
        let loading=self.tabs.iter().any(|tab|tab.app.is_loading())
            ||self.downloads.as_ref().is_some_and(|downloads|downloads.active_count().map_or(true,|count|count!=0));
        let host=if pending{Some(Instant::now())}else if loading{Some(Instant::now()+Duration::from_millis(20))}else{None};
        page.into_iter().chain(host).min()
    }
    /// Explicit final browser shutdown only. No per-tab/window close clears
    /// shared history. Failed history cleanup keeps finalization retryable and
    /// refuses new owners/input; a second successful shutdown performs no I/O.
    pub fn shutdown(&mut self,now:u64)->Result<Shutdown,String>{
        if self.phase==Phase::Closed{return Ok(Shutdown::default());}
        let mut result=Shutdown::default();
        if self.phase==Phase::Running {
            self.phase=Phase::Finalizing;self.mailbox.close();result.tabs=self.tabs.len();result.windows=self.windows.len();
            self.windows.clear();for tab in self.tabs.drain(..){tab.app.retire_from_session();}
            // Browser ownership, not App drop, ends downloads. Existing Manager
            // Drop retains its real cancel/disconnect/join/partial-file cleanup.
            self.downloads=None;
        }
        result.history_removed=self.history.shutdown(now)?;self.phase=Phase::Closed;Ok(result)
    }
}
impl Drop for Session {
    fn drop(&mut self){
        self.mailbox.close();self.windows.clear();for tab in self.tabs.drain(..){tab.app.retire_from_session();}
        // No implicit history shutdown/save. Download Shared retains only its
        // original Manager Drop cleanup when the session's fields are dropped.
    }
}

impl App {
    /// The caller has already removed all routing for this identity. Consuming
    /// the Box makes every post-retire App entry point unreachable in safe code.
    fn retire_from_session(mut self:Box<Self>){
        debug_assert!(self.owned_downloads.is_none()&&self.owned_history.is_none());
        self.set_load_wake(Arc::new(||{}));self.retire_embedded_video_owner();
        self.scripting=None;self.pending=None;self.resubmission=None;self.history_maintenance=None;
        // Drop directly: do not call reflow, Stop, cancel helpers that apply DOM
        // removals/history, media sync polling or any JavaScript dispatch.
        self.worker=None;self.dynamic_scripts=None;self.dynamic_stylesheets=None;self.dynamic_images=None;
        self.embedded_video_worker=None;self.media_worker=None;self.audio_worker=None;
        self.media=None;self.audio=None;self.embedded_video=None;self.animations.clear();
        self.pending_images.clear();self.pending_backgrounds.clear();self.background_requests.clear();self.backgrounds.clear();
        drop(self);
    }
}

#[path="app_session_ui.rs"]
pub(crate) mod window_ui;

#[cfg(test)]
#[path="app_session_tests.rs"]
mod tests;
