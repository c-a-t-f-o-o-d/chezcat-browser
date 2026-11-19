//! Same-document MessageChannel tasks in the normal document event loop.
//! https://html.spec.whatwg.org/multipage/web-messaging.html#channel-messaging
use super::*;
use crate::javascript::message_channel_interface::*;
use std::collections::VecDeque;

const CHANNEL_BASE: u32 = 0x8900_0000;
const PORT_BASE: u32 = 0x8a00_0000;
const ARENA: u32 = PORT_BASE - CHANNEL_BASE;

#[derive(Default)]
pub(super) struct State {
    channels: Vec<[Port; 2]>,
    queue: VecDeque<Message>,
}

#[derive(Default)]
struct Port {
    started: bool,
    detached: bool,
}

struct Message {
    target: u32,
    queued: Instant,
    data: Value,
    bytes: usize,
}

impl State {
    pub(super) fn port_count(&self) -> usize { self.channels.len() * 2 }

    fn channel_index(&self, channel: u32) -> Option<usize> {
        channel.checked_sub(CHANNEL_BASE).filter(|&index| index < ARENA)
            .map(|index| index as usize).filter(|&index| index < self.channels.len())
    }

    fn port_index(&self, port: u32) -> Option<(usize, usize)> {
        let index = port.checked_sub(PORT_BASE).filter(|&index| index < ARENA)? as usize;
        (index / 2 < self.channels.len()).then_some((index / 2, index % 2))
    }

    fn port_ready(&self, handle: u32) -> bool {
        self.port_index(handle).is_some_and(|(index, endpoint)| {
            let port = &self.channels[index][endpoint];
            port.started && !port.detached
        })
    }

    pub(super) fn next_deadline(&self) -> Option<Instant> {
        self.queue.iter().find(|message| self.port_ready(message.target)).map(|message| message.queued)
    }

    pub(super) fn clear(&mut self) -> usize {
        self.queue.drain(..).map(|message| message.bytes).sum()
    }

    fn close(&mut self, handle: u32, index: usize, endpoint: usize) -> usize {
        self.channels[index][endpoint].detached = true;
        let mut released = 0;
        self.queue.retain(|message| {
            if message.target != handle { return true; }
            released += message.bytes;
            false
        });
        released
    }
}

impl Page {
    pub(super) fn clear_message_tasks(&mut self) {
        self.state.retained -= self.state.message_channels.clear();
    }

    pub(super) fn poll_message_task(&mut self, doc: &mut Document, forms: &mut Forms, now: Instant) -> Option<Outcome> {
        let index = self.state.message_channels.queue.iter().position(|message|
            message.queued <= now && self.state.message_channels.port_ready(message.target))?;
        let message = self.state.message_channels.queue.remove(index).unwrap();
        self.state.retained -= message.bytes;
        self.begin_job();
        let before = self.state.errors;
        let result = {
            let mut bridge = Bridge { doc, forms, state: &mut self.state };
            bridge.begin_message_delivery(message.target, &message.data)
                .and_then(|token| self.runtime.invoke_native_dispatch(token, &mut bridge))
        };
        if let Err(error) = result { self.state.error("MessagePort message", &error); }
        self.microtask_checkpoint(doc, forms);
        Some(Outcome {
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
            ..Outcome::default()
        })
    }
}

impl Bridge<'_> {
    pub(super) fn message_port_owner(&self, object: u32) -> Option<u32> {
        self.state.message_channels.port_index(object).map(|_| WINDOW)
    }

    pub(super) fn message_instance(&self, object: u32) -> Option<&'static str> {
        if self.message_port_owner(object).is_some() { return Some(PORT); }
        self.state.message_channels.channel_index(object).map(|_| CHANNEL)
    }

    pub(super) fn message_handler_event(&self, object: u32, property: &str) -> Option<&'static str> {
        (property == "onmessage" && self.message_port_owner(object).is_some()).then_some("message")
    }

    pub(super) fn matches_message_receiver(&self, object: u32, method: &str) -> bool {
        match method {
            PORT1 | PORT2 => self.message_instance(object) == Some(CHANNEL),
            POST | START | CLOSE | MESSAGE | SET_MESSAGE => self.message_port_owner(object).is_some(),
            DATA | ORIGIN | SOURCE | LAST_ID => self.message_event_receiver(object),
            _ => false,
        }
    }

    pub(super) fn validate_message_call(&self, object: u32, method: &str, args: &[Value])
        -> Result<(), (&'static str, String)>
    {
        if method == CHANNEL && object == WINDOW { return Ok(()); }
        if !self.matches_message_receiver(object, method) {
            return Err(("TypeError", "Message operation requires a genuine receiver.".into()));
        }
        if method == POST {
            if args.is_empty() { return Err(("TypeError", "postMessage requires a message.".into())); }
            if args.get(1).is_some_and(|value| !matches!(value, Value::Null | Value::Undefined)) {
                return Err(("NotSupportedError", "MessagePort transfer lists and options are unavailable.".into()));
            }
            if matches!(args[0], Value::Object(_) | Value::Host(_) | Value::Function(_) | Value::Symbol(_)) {
                return Err(("DataCloneError", "This MessagePort supports serialized primitive data.".into()));
            }
        }
        Ok(())
    }

    pub(super) fn message_call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        self.validate_message_call(object, method, args).map_err(|(_, message)| message)?;
        self.state.work(1)?;
        match method {
            CHANNEL => self.create_message_channel(),
            PORT1 | PORT2 => {
                let index = self.state.message_channels.channel_index(object).unwrap();
                Ok(Value::Host(PORT_BASE + 2 * index as u32 + u32::from(method == PORT2)))
            }
            POST => self.post_message(object, &args[0]),
            START => {
                self.start_message_port(object);
                Ok(Value::Undefined)
            }
            CLOSE => {
                let (index, endpoint) = self.state.message_channels.port_index(object).unwrap();
                let released = self.state.message_channels.close(object, index, endpoint);
                self.state.retained -= released;
                Ok(Value::Undefined)
            }
            MESSAGE => Ok(self.handler(object, "onmessage")?.unwrap_or(Value::Null)),
            SET_MESSAGE => {
                let value = args.first().filter(|value| matches!(value, Value::Function(_))).cloned().unwrap_or(Value::Null);
                self.set_handler(object, "onmessage", value)?;
                self.start_message_port(object);
                Ok(Value::Undefined)
            }
            DATA => self.message_event_data(object),
            ORIGIN | LAST_ID => Ok(Value::String(String::new())),
            SOURCE => Ok(Value::Null),
            _ => Err("Unsupported message operation.".into()),
        }
    }

    fn create_message_channel(&mut self) -> Result<Value, String> {
        if !self.state.enabled || !self.doc.scripting_enabled {
            return Err("MessageChannel requires an active scripting document.".into());
        }
        let channels = &self.state.message_channels;
        let count = channels.port_count();
        let occupied = self.event_target_count();
        if occupied.saturating_add(2) > self.state.limits.host.event_targets || count >= ARENA as usize {
            return Err("MessagePort target limit reached.".into());
        }
        self.state.message_channels.channels.try_reserve(1).map_err(|_| "Could not reserve channel endpoints.")?;
        self.state.replace_bytes(0, size_of::<[Port; 2]>())?;
        self.state.message_channels.channels.push([Port::default(), Port::default()]);
        Ok(Value::Host(CHANNEL_BASE + (count / 2) as u32))
    }

    fn start_message_port(&mut self, handle: u32) {
        let (index, endpoint) = self.state.message_channels.port_index(handle).unwrap();
        self.state.message_channels.channels[index][endpoint].started = true;
    }

    fn post_message(&mut self, handle: u32, data: &Value) -> Result<Value, String> {
        let (index, endpoint) = self.state.message_channels.port_index(handle).unwrap();
        let channels = &self.state.message_channels;
        if channels.channels[index].iter().any(|port| port.detached) { return Ok(Value::Undefined); }
        if channels.queue.len() >= self.state.limits.host.timers { return Err("Queued message limit reached.".into()); }
        let payload = data.bytes();
        if payload > self.state.limits.host.string_bytes { return Err("Message payload exceeds the host string bound.".into()); }
        let bytes = size_of::<Message>().saturating_add(payload);
        self.state.copies(payload)?;
        self.state.message_channels.queue.try_reserve(1).map_err(|_| "Could not reserve queued message.")?;
        self.state.replace_bytes(0, bytes)?;
        self.state.message_channels.queue.push_back(Message {
            target: PORT_BASE + (index * 2 + (1 - endpoint)) as u32,
            queued: Instant::now(),
            data: data.clone(),
            bytes,
        });
        Ok(Value::Undefined)
    }

    fn begin_message_delivery(&mut self, target: u32, data: &Value) -> Result<usize, String> {
        let event = self.construct_message_event(data)?;
        let token = match self.begin_synthetic_dispatch(target, &[Value::Host(event)]) {
            Ok(token) => token,
            Err(error) => {
                self.rollback_base_event(event)?;
                return Err(error);
            }
        };
        let index = self.created_event_index(event).unwrap();
        self.state.created_events[index].trusted = true;
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(source: &str) -> (Document, Forms, Page) {
        let mut doc = crate::html::parse("<html><body><p>Original channel fixture</p></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let outcome = page.start(&mut doc, &mut forms, &[Script {
            node: 0, source: source.into(), label: "original message-channel fixture".into(),
        }]);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        (doc, forms, page)
    }

    fn evaluate(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
        let outcome = page.console_eval(doc, forms, source);
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }

    #[test]
    fn channel_ports_deliver_trusted_fifo_tasks_after_microtasks() {
        let (mut doc, mut forms, mut page) = fixture(r#"
var ch=new MessageChannel(), left=ch.port1, right=ch.port2, trace='sync', saved;
if(!(ch instanceof MessageChannel)||!(left instanceof MessagePort)||!(right instanceof EventTarget)||
   left===right||ch.port1!==left||ch.port2!==right||Object.getPrototypeOf(right)!==MessagePort.prototype||
   right.onmessage!==null)throw 'native channel identity';
right.addEventListener('message',function(ev){
   if(ev.target!==right||ev.currentTarget!==right||this!==right||!(ev instanceof MessageEvent)||
      !(ev instanceof Event)||ev instanceof CustomEvent||ev.origin!==''||ev.lastEventId!==''||
      ev.source!==null||ev.bubbles||ev.cancelable||!ev.isTrusted)throw 'genuine trusted message';
   saved=ev;trace+=' '+ev.data;
   Promise.resolve().then(function(){trace+=' micro';});
});
left.postMessage('first');left.postMessage(2);
Promise.resolve().then(function(){trace+=' initial-micro';});
"#);
        assert!(page.next_timer_deadline().is_none());
        assert!(page.poll_timer(&mut doc, &mut forms).is_none());
        evaluate(&mut page, &mut doc, &mut forms, "if(trace!=='sync initial-micro')throw trace; right.start();right.start();");
        let first = page.poll_timer(&mut doc, &mut forms).unwrap();
        assert_eq!(first.errors, 0, "{}", page.report());
        evaluate(&mut page, &mut doc, &mut forms, "if(trace!=='sync initial-micro first micro'||saved.currentTarget!==null||!saved.isTrusted)throw trace;");
        let second = page.poll_timer(&mut doc, &mut forms).unwrap();
        assert_eq!(second.errors, 0, "{}", page.report());
        evaluate(&mut page, &mut doc, &mut forms, "if(trace!=='sync initial-micro first micro 2 micro'||saved.data!==2)throw trace;");
        assert!(page.next_timer_deadline().is_none());
    }

    #[test]
    fn channel_activation_close_and_invalid_data_leave_queues_consistent() {
        let (mut doc, mut forms, mut page) = fixture(r#"
var c=new MessageChannel(), sent=c.port1, recv=c.port2, received='', known='snap';
sent.postMessage(known);known='changed';
recv.onmessage=function(event){received+=event.data;};
if(received!=='')throw 'synchronous messages';
var failures=0;
for(var run of [function(){sent.postMessage();},function(){MessageChannel();},function(){new MessagePort();},
               function(){MessagePort.prototype.start.call({});},function(){sent.postMessage(Symbol('x'));},
               function(){sent.postMessage(function(){});},function(){sent.postMessage('x',[recv]);}]){
  try{run();}catch(e){if(e.name==='TypeError'||e.name==='DataCloneError'||e.name==='NotSupportedError'){failures++;continue;}throw e;}
}
if(failures!==7)throw 'invalid message or receiver';
"#);
        assert_eq!(page.state.message_channels.queue.len(), 1);
        let delivered = page.poll_timer(&mut doc, &mut forms).unwrap();
        assert_eq!(delivered.errors, 0, "{}", page.report());
        evaluate(&mut page, &mut doc, &mut forms, r#"
if(received!=='snap')throw 'copied payload';
sent.postMessage('lost');recv.close();recv.close();
sent.postMessage('also lost');recv.postMessage('third');
var open=new MessageChannel();
open.port2.onmessage=function(){received+='unexpected';};
open.port1.postMessage('cancelled');
"#);
        assert_eq!(page.state.message_channels.queue.len(), 1);
        page.clear_host_requests();
        assert!(page.state.message_channels.queue.is_empty());
        assert!(page.poll_timer(&mut doc, &mut forms).is_none());
        evaluate(&mut page, &mut doc, &mut forms, "if(received!=='snap')throw 'closed or cancelled message delivered';");
    }
}
