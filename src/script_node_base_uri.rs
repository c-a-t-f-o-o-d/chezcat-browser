//! Read-only Node base URLs share the native document/resource authority.
//! https://dom.spec.whatwg.org/#dom-node-baseuri
use super::*;

impl Bridge<'_> {
    pub(super) fn node_base_uri_get(&mut self, object: u32) -> Result<Value, String> {
        if !self.text_content_receiver(object) {
            return Err("Node baseURI requires a represented genuine Node.".into());
        }
        if let Some(value) = self.frame_node_base_uri(self.state.navigation_incumbent, object) {
            return value;
        }
        if self.inert_node_type(object).is_some() {
            return self.inert_get(object, "baseURI");
        }
        if let Some(id)=script_attr::index(object).filter(|&id|id<self.state.attributes.len()){let owner=self.state.attributes[id].document;if owner!=DOCUMENT{return self.inert_get(owner,"baseURI");}}
        self.main_node_base_uri()
    }
    pub(super) fn main_node_base_uri(&mut self) -> Result<Value, String> {
        let base = self.state.dynamic.base.as_ref().or_else(|| {
            self.state
                .location
                .as_ref()
                .map(|location| &location.source)
        });
        let cost = dom_compat::url_copy_cost("", base)?;
        self.state.work(cost)?;
        self.state.copies(cost)?;
        let base = self.state.dynamic.base.as_ref().or_else(|| {
            self.state
                .location
                .as_ref()
                .map(|location| &location.source)
        });
        let value = base.map_or_else(
            || crate::navigation::Location::home().display(),
            crate::navigation::Location::display,
        );
        self.string(&value)
    }
}
