//! Readonly state and real native events for the one bound authored video.
use super::*;
pub(crate) fn event_name(name: &str) -> bool {
    matches!(
        name,
        "loadstart"
            | "abort"
            | "error"
            | "loadedmetadata"
            | "durationchange"
            | "loadeddata"
            | "canplay"
            | "canplaythrough"
            | "play"
            | "playing"
            | "pause"
            | "seeking"
            | "seeked"
            | "timeupdate"
            | "ended"
    )
}
impl Page {
    pub fn dispatch_media(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        node: usize,
        event: &str,
    ) -> Outcome {
        if !self.state.enabled
            || !doc.scripting_enabled
            || !event_name(event)
            || !self
                .state
                .embedded_video
                .as_ref()
                .is_some_and(|s| s.node == node)
            || !doc.nodes.get(node).is_some_and(|n| {
                n.namespace == crate::html::Namespace::Html
                    && matches!(&n.kind,NodeKind::Element {tag,..} if tag=="video")
            })
        {
            return Outcome::default();
        }
        self.begin_job();
        let before = self.state.errors;
        self.dispatch_to(doc, forms, node_handle(node), event, false, false);
        Outcome {
            prevented: false,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }
}
impl Bridge<'_> {
    pub(super) fn media_handler_event<'a>(
        &self,
        object: u32,
        property: &'a str,
    ) -> Option<&'a str> {
        handler_event(property).or_else(|| {
            self.media_element_receiver(object)
                .then(|| property.strip_prefix("on").filter(|name| event_name(name)))
                .flatten()
        })
    }
    pub(super) fn media_element_receiver(&self, object: u32) -> bool {
        node_index(object).is_some_and(|node| {
            self.state
                .embedded_video
                .as_ref()
                .is_some_and(|s| s.node == node)
                && self.doc.nodes.get(node).is_some_and(|n| {
                    n.namespace == crate::html::Namespace::Html
                        && matches!(&n.kind,NodeKind::Element {tag,..} if tag=="video")
                })
        })
    }
    pub(super) fn media_element_has(
        &mut self,
        object: u32,
        key: &str,
    ) -> Result<Option<bool>, String> {
        self.node(object)?;
        Ok(match key {
            "src" | "currentSrc" | "controls" | "videoWidth" | "videoHeight" | "duration"
            | "currentTime" | "paused" | "ended" | "seeking" | "readyState" | "networkState" => {
                Some(true)
            }
            "play" | "pause" | "load" | "canPlayType" => Some(false),
            _ => None,
        })
    }
    pub(super) fn media_element_get(
        &mut self,
        object: u32,
        key: &str,
    ) -> Result<Option<Value>, String> {
        let node = self.node(object)?;
        if key == "src" {
            return self.image_source(node).map(Some);
        }
        if key == "controls" {
            return Ok(Some(Value::Bool(self.doc.attr(node, "controls").is_some())));
        }
        if key == "currentSrc" {
            let source = self
                .state
                .embedded_video
                .as_ref()
                .unwrap()
                .current_src
                .clone();
            return self.string(&source).map(Some);
        }
        let s = self.state.embedded_video.as_ref().unwrap();
        Ok(Some(match key {
            "videoWidth" => Value::Number(f64::from(s.width)),
            "videoHeight" => Value::Number(f64::from(s.height)),
            "duration" => Value::Number(s.duration.map_or(f64::NAN, |d| d.as_secs_f64())),
            "currentTime" => Value::Number(s.position().as_secs_f64()),
            "paused" => Value::Bool(!s.playing),
            "ended" => Value::Bool(s.ended),
            "seeking" => Value::Bool(s.seeking),
            "readyState" => Value::Number(f64::from(s.ready)),
            "networkState" => Value::Number(f64::from(s.network)),
            "play" | "pause" | "load" | "canPlayType" => Value::Undefined,
            _ => return Ok(None),
        }))
    }
}
