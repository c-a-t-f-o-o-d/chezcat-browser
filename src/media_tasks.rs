//! Bounded control and event order for the one supported authored media player.
//! Controls drain independently of decoder completion and queued event delivery.
use std::collections::VecDeque;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) type Shared = Arc<Mutex<Queue>>;
pub(crate) fn shared(limit: usize) -> Shared { Arc::new(Mutex::new(Queue::new(limit))) }

#[derive(Clone, Debug)]
pub(crate) struct Scope {
    pub node: usize,
    pub retired: Arc<AtomicBool>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Control {
    Pause { issued: Instant, position: Duration },
    Seek { seconds: f64 },
    Play { issued: Instant, notify_queued: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    Single(&'static str), Pause,
    Playing(u64), ResolvePlay(u64), PauseReject(u64), RejectPlay(u64),
}

struct Entry {
    id: u64,
    scope: Scope,
    control: Option<Control>,
    control_applied: bool,
    task: Option<Task>,
}
pub(crate) struct Queue {
    limit: usize,
    next: u64,
    entries: VecDeque<Entry>,
    control_cursor: usize,
}
impl Queue {
    pub(crate) fn new(limit: usize) -> Self {
        Self { limit, next: 0, entries: VecDeque::new(), control_cursor: 0 }
    }
    pub(crate) fn enqueue(&mut self, scope: Scope, control: Option<Control>, task: Option<Task>) -> Result<u64, String> {
        if control.is_none() && task.is_none() { return Err("Media entry requires a control or event task.".into()); }
        if scope.retired.load(Ordering::Relaxed) { return Err("Media source is retired.".into()); }
        self.prune_empty_front();
        if self.entries.len() >= self.limit { return Err("Media task allowance reached.".into()); }
        let id = self.next.checked_add(1).ok_or("Media control identity exhausted.")?;
        self.entries.try_reserve(1).map_err(|_| "Could not retain the media control/event entry.")?;
        self.entries.push_back(Entry { id, scope, control, control_applied: control.is_none(), task });
        self.next = id;
        Ok(id)
    }
    pub(crate) fn reserve_entries(&mut self,count:usize)->Result<(),String> {
        self.prune_empty_front();
        let total=self.entries.len().checked_add(count).ok_or("Media queue count exhausted.")?;
        if total>self.limit {return Err("Media task allowance reached.".into());}
        self.next.checked_add(u64::try_from(count).map_err(|_|"Media queue count is not representable.")?)
            .ok_or("Media control identity exhausted.")?;
        self.entries.try_reserve(count).map_err(|_|"Could not reserve media operation tasks.".into())
    }

    /// Reserve a fixed operation's separate tasks together before its visible
    /// state changes. Their positions still represent actual enqueue order.
    pub(crate) fn enqueue_many(&mut self,scope:Scope,entries:&[(Option<Control>,Option<Task>)])->Result<(),String> {
        if entries.is_empty() || entries.iter().any(|(control,task)|control.is_none()&&task.is_none()) {
            return Err("Media batch requires nonempty controls or event tasks.".into());
        }
        if scope.retired.load(Ordering::Relaxed) {return Err("Media source is retired.".into());}
        self.prune_empty_front();
        let total=self.entries.len().checked_add(entries.len()).ok_or("Media queue count exhausted.")?;
        if total>self.limit {return Err("Media task allowance reached.".into());}
        let count=u64::try_from(entries.len()).map_err(|_|"Media queue count is not representable.")?;
        let next=self.next.checked_add(count).ok_or("Media control identity exhausted.")?;
        self.entries.try_reserve(entries.len()).map_err(|_|"Could not retain the media operation tasks.")?;
        for &(control,task) in entries {
            // The whole sequence was checked before any publication.
            self.next=self.next.checked_add(1).expect("admitted media identity range");
            self.entries.push_back(Entry {id:self.next,scope:scope.clone(),control,
                control_applied:control.is_none(),task});
        }
        debug_assert_eq!(self.next,next);
        Ok(())
    }

    // Each queued entry is visited at most once by the physical-control cursor.
    // Applied taskless entries behind a live task remain charged until that task
    // leaves the front; no repeated middle removal can turn seeks quadratic.
    pub(crate) fn take_control(&mut self) -> Option<(u64, Scope, Control)> {
        self.prune_empty_front();
        while self.control_cursor < self.entries.len() {
            let index = self.control_cursor;
            self.control_cursor += 1;
            let entry = &mut self.entries[index];
            let result = if entry.scope.retired.load(Ordering::Relaxed) {
                entry.control_applied = true;
                entry.task = None;
                None
            } else if !entry.control_applied {
                entry.control_applied = true;
                entry.control.map(|control| (entry.id, entry.scope.clone(), control))
            } else { None };
            self.prune_empty_front();
            if result.is_some() { return result; }
        }
        self.prune_empty_front();
        None
    }
    pub(crate) fn take_task(&mut self) -> Option<(u64, Scope, Task)> {
        self.prune_empty_front();
        let front = self.entries.front()?;
        if !front.control_applied { return None; }
        let entry = self.pop_front()?;
        let task = entry.task.expect("front pruning retains only a live event task or unapplied control");
        self.prune_empty_front();
        Some((entry.id, entry.scope, task))
    }
    pub(crate) fn peek_pause(&self) -> Option<(u64, Scope, Instant, Duration, bool)> {
        self.entries.iter().find_map(|entry| {
            if entry.scope.retired.load(Ordering::Relaxed) { return None; }
            match entry.control {
                Some(Control::Pause { issued, position }) => Some((entry.id, entry.scope.clone(), issued, position, entry.control_applied)),
                _ => None,
            }
        })
    }
    pub(crate) fn has_pending(&self) -> bool { !self.entries.is_empty() }
    pub(crate) fn len(&self) -> usize { self.entries.len() }
    // Upper bound captured before dispatch: includes stale/unapplied task slots.
    // A callback's later appended tasks cannot expand the caller's initial batch.
    pub(crate) fn count_tasks(&self) -> usize { self.entries.iter().filter(|entry| entry.task.is_some()).count() }
    pub(crate) fn set_limit(&mut self, limit: usize) { self.limit = limit; }
    pub(crate) fn cancel(&mut self) {
        self.entries.clear();
        self.control_cursor = 0;
    }
    pub(crate) fn cancel_source(&mut self, retired: &Arc<AtomicBool>) -> usize {
        let old_cursor = self.control_cursor;
        let mut index = 0;
        let mut before_cursor = 0;
        let before = self.entries.len();
        self.entries.retain(|entry| {
            let keep = !Arc::ptr_eq(&entry.scope.retired, retired);
            if !keep && index < old_cursor { before_cursor += 1; }
            index += 1;
            keep
        });
        self.control_cursor = old_cursor.saturating_sub(before_cursor);
        let removed = before - self.entries.len();
        self.prune_empty_front();
        removed
    }
    fn pop_front(&mut self) -> Option<Entry> {
        let entry = self.entries.pop_front()?;
        self.control_cursor = self.control_cursor.saturating_sub(1);
        Some(entry)
    }
    fn prune_empty_front(&mut self) {
        while self.entries.front().is_some_and(|entry|
            entry.scope.retired.load(Ordering::Relaxed) || entry.control_applied && entry.task.is_none()) {
            self.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> Scope { Scope { node: 7, retired: Arc::new(AtomicBool::new(false)) } }
    fn pause() -> Control { Control::Pause { issued: Instant::now(), position: Duration::from_millis(750) } }
    fn assert_source(actual: &Scope, expected: &Scope) {
        assert_eq!(actual.node, expected.node);
        assert!(Arc::ptr_eq(&actual.retired, &expected.retired));
    }
    #[test]
    fn media_queue_admission_refuses_empty_retired_and_small_limit_without_ids() {
        let s=scope();let mut q=Queue::new(0);
        assert!(q.enqueue(s.clone(),None,None).is_err());assert_eq!(q.next,0);
        s.retired.store(true,Ordering::Relaxed);
        assert!(q.enqueue(s.clone(),Some(pause()),Some(Task::Pause)).is_err());assert_eq!(q.next,0);
        s.retired.store(false,Ordering::Relaxed);
        assert!(q.enqueue(s.clone(),Some(pause()),Some(Task::Pause)).is_err());assert_eq!(q.next,0);
        q.set_limit(2);let id=q.enqueue(s.clone(),Some(pause()),Some(Task::Pause)).unwrap();assert_eq!(id,1);
        assert_eq!(q.enqueue(s.clone(),None,Some(Task::Single("later"))).unwrap(),2);
        assert!(q.enqueue(s.clone(),None,Some(Task::Single("refused"))).is_err());assert_eq!(q.next,2);assert_eq!(q.len(),2);
        assert!(q.take_task().is_none(),"front control must be physically applied first");
        assert_eq!(q.count_tasks(),2);
        let(id,owner,_)=q.take_control().unwrap();assert_eq!(id,1);assert_source(&owner,&s);
        assert_eq!(q.take_task().unwrap().2,Task::Pause);assert_eq!(q.take_task().unwrap().2,Task::Single("later"));
        assert_eq!(q.enqueue(s,None,Some(Task::Single("recovered"))).unwrap(),3);
    }
    #[test]
    fn media_queue_identity_exhaustion_and_cancel_never_rewind() {
        let s=scope();let mut q=Queue::new(4);q.next=u64::MAX-1;
        assert_eq!(q.enqueue(s.clone(),None,Some(Task::Single("last"))).unwrap(),u64::MAX);
        q.cancel();assert!(!q.has_pending());assert_eq!(q.control_cursor,0);
        assert!(q.enqueue(s.clone(),Some(pause()),Some(Task::Pause)).is_err());assert_eq!(q.next,u64::MAX);
        let mut fresh=Queue::new(1);assert_eq!(fresh.enqueue(s,None,Some(Task::Single("fresh document"))).unwrap(),1);
    }
    #[test]
    fn media_queue_controls_cross_retained_tasks_and_taskless_fronts_are_pruned() {
        let s=scope();let mut q=Queue::new(8);
        let older=q.enqueue(s.clone(),None,Some(Task::Single("older"))).unwrap();
        let seek=q.enqueue(s.clone(),Some(Control::Seek{seconds:2.2}),None).unwrap();
        let paused=q.enqueue(s.clone(),Some(pause()),Some(Task::Pause)).unwrap();
        let later=q.enqueue(s.clone(),Some(Control::Seek{seconds:1.0}),None).unwrap();
        let tail=q.enqueue(s.clone(),None,Some(Task::Single("tail"))).unwrap();
        let(_,_,issued,position,applied)=q.peek_pause().unwrap();assert!(!applied);
        assert_eq!(q.take_control().unwrap().0,seek);
        assert_eq!(q.take_control().unwrap().0,paused);
        assert_eq!(q.take_control().unwrap().0,later);assert!(q.take_control().is_none());
        assert_eq!(q.peek_pause().map(|v|(v.0,v.2,v.3,v.4)),Some((paused,issued,position,true)));
        assert_eq!(q.control_cursor,5);assert_eq!(q.len(),5);assert_eq!(q.count_tasks(),3);
        assert_eq!(q.take_task().unwrap().0,older);assert_eq!(q.len(),3);assert_eq!(q.control_cursor,3);
        assert_eq!(q.take_task().unwrap().0,paused);assert_eq!(q.len(),1);assert_eq!(q.control_cursor,1);
        assert_eq!(q.take_task().unwrap().0,tail);assert!(!q.has_pending());assert_eq!(q.control_cursor,0);
        q.enqueue(s,Some(Control::Seek{seconds:0.25}),None).unwrap();
        assert_eq!(q.count_tasks(),0);assert!(q.take_control().is_some());assert!(!q.has_pending());
    }
    #[test]
    fn media_queue_retired_source_is_invisible_and_never_applied_or_delivered() {
        let old=scope();let new=scope();let mut q=Queue::new(4);
        q.enqueue(old.clone(),Some(pause()),Some(Task::Pause)).unwrap();
        let id=q.enqueue(new.clone(),Some(Control::Seek{seconds:1.0}),Some(Task::Single("new source"))).unwrap();
        old.retired.store(true,Ordering::Relaxed);assert!(q.peek_pause().is_none());assert_eq!(q.count_tasks(),2);
        let(actual,owner,control)=q.take_control().unwrap();assert_eq!(actual,id);assert_source(&owner,&new);
        assert_eq!(control,Control::Seek{seconds:1.0});assert!(q.take_control().is_none());
        let(actual,owner,event)=q.take_task().unwrap();assert_eq!(actual,id);assert_source(&owner,&new);
        assert_eq!(event,Task::Single("new source"));assert!(!q.has_pending());
    }
    #[test]
    fn media_queue_source_cancel_keeps_other_arc_and_adjusts_cursor_exactly() {
        let a=scope();let b=scope();let mut q=Queue::new(8);
        q.enqueue(a.clone(),None,Some(Task::Single("removed"))).unwrap();
        q.enqueue(b.clone(),Some(Control::Seek{seconds:1.0}),None).unwrap();
        q.enqueue(a.clone(),Some(pause()),Some(Task::Pause)).unwrap();
        let survivor=q.enqueue(b.clone(),None,Some(Task::Single("survivor"))).unwrap();
        assert!(matches!(q.take_control().unwrap().2,Control::Seek{..}));assert_eq!(q.control_cursor,2);
        assert_eq!(q.cancel_source(&a.retired),2);assert_eq!(q.len(),1);assert_eq!(q.control_cursor,0);
        assert!(q.take_control().is_none());assert_eq!(q.take_task().unwrap().0,survivor);
        assert!(!b.retired.load(Ordering::Relaxed));
        assert_eq!(q.enqueue(b,None,Some(Task::Single("new"))).unwrap(),survivor+1);
    }
    #[test]
    fn media_queue_late_seek_completion_appends_after_pause_and_initial_batch() {
        let s=scope();let mut q=Queue::new(5);
        let seek=q.enqueue(s.clone(),Some(Control::Seek{seconds:2.2}),None).unwrap();
        let paused=q.enqueue(s.clone(),Some(pause()),Some(Task::Pause)).unwrap();
        assert_eq!(q.count_tasks(),1,"seek completion is not reserved before decoding");
        assert_eq!(q.take_control().unwrap().0,seek);assert_eq!(q.take_control().unwrap().0,paused);
        let initial=q.count_tasks();assert_eq!(initial,1);
        let seeked=q.enqueue(s.clone(),None,Some(Task::Single("seeked"))).unwrap();
        let delivered=(0..initial).filter_map(|_|q.take_task()).map(|row|row.0).collect::<Vec<_>>();
        assert_eq!(delivered,vec![paused]);assert_eq!(q.count_tasks(),1);
        assert_eq!(q.take_task().unwrap().0,seeked);assert!(!q.has_pending());
    }
    #[test]
    fn media_queue_limit_changes_only_future_admission_and_growth_is_on_demand() {
        let s=scope();let mut q=Queue::new(65536);assert_eq!(q.entries.capacity(),0);
        for _ in 0..300{q.enqueue(s.clone(),None,Some(Task::Single("event"))).unwrap();}
        assert_eq!(q.len(),300);assert!(q.entries.capacity()<65536);
        q.set_limit(0);assert!(q.enqueue(s.clone(),None,Some(Task::Pause)).is_err());assert_eq!(q.len(),300);
        for _ in 0..300{assert!(q.take_task().is_some());}assert!(!q.has_pending());
        q.set_limit(1);assert!(q.enqueue(s,None,Some(Task::Pause)).is_ok());
    }
    #[test]
    fn media_queue_many_interleaved_controls_use_one_cursor_pass_and_front_pruning() {
        const PAIRS:usize=4096;
        let s=scope();let mut q=Queue::new(PAIRS*2+1);
        q.enqueue(s.clone(),None,Some(Task::Single("front"))).unwrap();
        for index in 0..PAIRS {
            q.enqueue(s.clone(),Some(Control::Seek{seconds:index as f64}),None).unwrap();
            q.enqueue(s.clone(),Some(Control::Pause{issued:Instant::now(),position:Duration::from_millis(index as u64)}),Some(Task::Pause)).unwrap();
        }
        assert_eq!(q.count_tasks(),PAIRS+1);
        for index in 0..PAIRS {
            let seek=q.take_control().unwrap();assert_eq!(seek.0,(index*2+2) as u64);assert_eq!(seek.2,Control::Seek{seconds:index as f64});
            let paused=q.take_control().unwrap();assert_eq!(paused.0,(index*2+3) as u64);assert!(matches!(paused.2,Control::Pause{..}));
        }
        assert_eq!(q.control_cursor,q.len());assert!(q.take_control().is_none());assert_eq!(q.control_cursor,PAIRS*2+1);
        assert_eq!(q.take_task().unwrap().2,Task::Single("front"));
        for index in 0..PAIRS {assert_eq!(q.take_task().unwrap().0,(index*2+3) as u64);}
        assert_eq!(q.len(),0);assert_eq!(q.control_cursor,0);assert!(!q.has_pending());
    }
}
