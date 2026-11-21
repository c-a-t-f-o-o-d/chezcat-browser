//! Native anchor release input only. Created events remain untrusted records;
//! this payload exists only during one genuine trusted dispatch slot.
use super::*;

/// Actual host input in CSS viewport coordinates. The App validates rendering
/// and supplies native screen coordinates; this API never queries an OS device.
#[derive(Clone, Copy, Debug, Default)]
pub struct AnchorMouseInput {
    pub button: u8,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    pub client_x: f64,
    pub client_y: f64,
    pub screen_x: f64,
    pub screen_y: f64,
}

impl Page {
    /// App begins the native input job before calling. Only primary click and
    /// middle auxclick are admitted. A failed dispatch always vetoes default.
    pub fn anchor_mouse(&mut self, doc: &mut Document, forms: &mut Forms,
        target: usize, input: AnchorMouseInput) -> Outcome {
        let before = self.state.errors;
        let invalid = if input.button > 1 {
            Some("Only primary and middle anchor release input is supported.")
        } else if ![input.client_x,input.client_y,input.screen_x,input.screen_y].iter().all(|value|value.is_finite()) {
            Some("Native anchor mouse coordinates must be finite.")
        } else if !doc.nodes.get(target).is_some_and(|node|node.namespace==crate::html::Namespace::Html
            && matches!(&node.kind,NodeKind::Element{tag,..} if tag=="a")) {
            Some("Native anchor input requires a genuine HTML anchor.")
        } else { None };
        if let Some(error) = invalid {
            self.state.error("Anchor mouse input", error);
            return Outcome { prevented:true,errors:self.state.errors.saturating_sub(before),..Outcome::default() };
        }
        // No script callback exists when the document policy disabled scripts.
        // The App's independent rendered/connected source guard still applies.
        if !self.state.enabled || !doc.scripting_enabled { return Outcome::default(); }
        if self.runtime.invocation_failed_fatally() {
            self.runtime.cancel_microtasks();self.cancel_media_selection();
            return Outcome { prevented:true,..Outcome::default() };
        }
        if self.state.host_work==0 || self.state.copy_work==0 {
            self.state.error("Anchor mouse input","The native job's host/copy budget is exhausted.");
            return Outcome { prevented:true,errors:self.state.errors.saturating_sub(before),..Outcome::default() };
        }
        if let Err(error)=self.state.rebuild(doc) {
            self.state.error("Anchor mouse input",&error);
            return Outcome { prevented:true,errors:self.state.errors.saturating_sub(before),..Outcome::default() };
        }
        if !self.state.tree.active.get(target).copied().unwrap_or(false)
            || !self.state.dom.connected.get(target).copied().unwrap_or(false) {
            self.state.error("Anchor mouse input","The anchor is no longer connected.");
            return Outcome { prevented:true,errors:self.state.errors.saturating_sub(before),..Outcome::default() };
        }
        self.notify_user_activation();
        let event=if input.button==0 {"click"} else {"auxclick"};
        let (prevented,complete)=self.dispatch_composed_status(doc,forms,node_handle(target),event,true,true,true,Some(input),None);
        Outcome {
            prevented:prevented || !complete || self.state.host_work==0 || self.state.copy_work==0,
            layout_changed:std::mem::take(&mut self.state.changed),
            errors:self.state.errors.saturating_sub(before),
        }
    }
}

impl Bridge<'_> {
    pub(super) fn trusted_mouse_get(&self,object:u32,property:&str)->Option<Result<Value,String>> {
        let input=self.state.event.as_ref().filter(|event|event.handle==object)?.mouse.as_ref()?;
        let value=match property {
            "view"=>Value::Host(WINDOW),"detail"=>Value::Number(1.0),
            "which"=>Value::Number(f64::from(input.button)+1.0),
            "screenX"=>Value::Number(input.screen_x),"screenY"=>Value::Number(input.screen_y),
            "clientX"|"x"=>Value::Number(input.client_x),"clientY"|"y"=>Value::Number(input.client_y),
            "pageX"=>Value::Number(0.0+input.client_x),
            "pageY"=>Value::Number(input.client_y+self.state.geometry.as_ref().map_or(0.0,|geometry|geometry.css_scroll_y())),
            "button"=>Value::Number(f64::from(input.button)),"buttons"=>Value::Number(0.0),
            "ctrlKey"=>Value::Bool(input.ctrl),"shiftKey"=>Value::Bool(input.shift),
            "altKey"=>Value::Bool(input.alt),"metaKey"=>Value::Bool(input.meta),
            "relatedTarget"=>Value::Null,
            // A target padding/layer measurement is not supplied by this seam.
            // Do not substitute the undispatched-created-event page aliases.
            "offsetX"|"offsetY"|"layerX"|"layerY"=>return Some(Err("Trusted mouse target-relative coordinates require measured target geometry.".into())),
            _=>return None,
        };
        Some(Ok(value))
    }
    pub(super) fn trusted_mouse_call(&mut self,object:u32,method:&str,args:&[Value])->Option<Result<Value,String>> {
        let input=self.state.event.as_ref().filter(|event|event.handle==object)?.mouse?;
        match method {
            "initMouseEvent"=>Some(self.validate_created_event_call(object,method,args)
                .map_err(|(_,message)|message).map(|()|Value::Undefined)),
            "getModifierState"=>Some((||{
                let Some(Value::String(name))=args.first() else{return Err("getModifierState requires a converted key.".into());};
                self.state.work(4)?;
                Ok(Value::Bool(match name.as_str(){"Alt"=>input.alt,"Control"=>input.ctrl,"Meta"=>input.meta,"Shift"=>input.shift,_=>false}))
            })()),
            _=>None,
        }
    }
}

#[cfg(test)]
#[path="script_anchor_mouse_tests.rs"]
mod tests;
