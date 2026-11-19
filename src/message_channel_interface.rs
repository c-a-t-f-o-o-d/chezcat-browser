//! Native entry points for document-owned HTML channel messaging.

pub(crate) const CHANNEL: &str = "MessageChannel";
pub(crate) const PORT: &str = "MessagePort";
pub(crate) const EVENT: &str = "MessageEvent";
pub(crate) const PORT1: &str = "\0MessageChannel.get.port1";
pub(crate) const PORT2: &str = "\0MessageChannel.get.port2";
pub(crate) const POST: &str = "\0MessagePort.postMessage";
pub(crate) const START: &str = "\0MessagePort.start";
pub(crate) const CLOSE: &str = "\0MessagePort.close";
pub(crate) const MESSAGE: &str = "\0MessagePort.get.onmessage";
pub(crate) const SET_MESSAGE: &str = "\0MessagePort.set.onmessage";
pub(crate) const DATA: &str = "\0MessageEvent.get.data";
pub(crate) const ORIGIN: &str = "\0MessageEvent.get.origin";
pub(crate) const SOURCE: &str = "\0MessageEvent.get.source";
pub(crate) const LAST_ID: &str = "\0MessageEvent.get.lastEventId";

pub(crate) fn operation(method: &str) -> bool {
    matches!(method, CHANNEL | PORT | EVENT | PORT1 | PORT2 | POST | START | CLOSE
        | MESSAGE | SET_MESSAGE | DATA | ORIGIN | SOURCE | LAST_ID)
}
