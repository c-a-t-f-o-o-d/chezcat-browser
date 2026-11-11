//! Live, bounded getElementsByTagName collections. Each handle retains a root
//! and tag filter, never a snapshot of node references. Queries share DOM work
//! limits and preserve the document's inert-subtree policy.
//! https://dom.spec.whatwg.org/#concept-getelementsbytagname
use super::*;

const COLLECTION_BASE: u32 = 256;
const MAX_COLLECTIONS: usize = 128;

pub(super) struct TagCollection {
    root: usize,
    name: String,
}

pub(super) fn collection_handle_index(handle: u32) -> Option<usize> {
    (COLLECTION_BASE..COLLECTION_BASE + MAX_COLLECTIONS as u32)
        .contains(&handle)
        .then(|| (handle - COLLECTION_BASE) as usize)
}

impl Bridge<'_> {
    pub(super) fn create_tag_collection(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<Value, String> {
        let name = self.text(value)?.to_ascii_lowercase();
        let root = if object == DOCUMENT {
            0
        } else {
            self.node(object)?
        };
        if self.state.dom.collections.len() >= MAX_COLLECTIONS {
            return Err("DOM live tag-collection limit reached (128 per document).".into());
        }
        self.state.replace_bytes(0, name.len())?;
        let handle = COLLECTION_BASE + self.state.dom.collections.len() as u32;
        self.state
            .dom
            .collections
            .push(TagCollection { root, name });
        Ok(Value::Host(handle))
    }

    fn tag_collection_nodes(&mut self, handle: u32) -> Result<Vec<usize>, String> {
        let index = collection_handle_index(handle).ok_or("Invalid HTMLCollection handle.")?;
        let collection = self
            .state
            .dom
            .collections
            .get(index)
            .ok_or("Invalid HTMLCollection handle.")?;
        let root = collection.root;
        let bytes = collection.name.len();
        self.state.copies(bytes)?;
        let name = self.state.dom.collections[index].name.clone();
        let candidates = self.query_candidates(root)?;
        if name == "*" {
            return Ok(candidates);
        }
        let mut found = Vec::new();
        for node in candidates {
            self.state.work(1)?;
            let candidate = tag(self.doc, node);
            self.state.copies(candidate.len())?;
            if candidate == name {
                found.push(node);
            }
        }
        Ok(found)
    }

    fn tag_collection_named(&mut self, handle: u32, name: &str) -> Result<Value, String> {
        if name.is_empty() {
            return Ok(Value::Undefined);
        }
        for node in self.tag_collection_nodes(handle)? {
            self.state.work(1)?;
            let id = self.doc.attr(node, "id");
            let candidate = self.doc.attr(node, "name");
            self.state.copies(
                id.map_or(0, str::len)
                    .saturating_add(candidate.map_or(0, str::len)),
            )?;
            if id == Some(name) || candidate == Some(name) {
                return Ok(Value::Host(node_handle(node)));
            }
        }
        Ok(Value::Undefined)
    }

    pub(super) fn tag_collection_get(
        &mut self,
        handle: u32,
        property: &str,
    ) -> Result<Value, String> {
        if property == "length" {
            return Ok(Value::Number(
                self.tag_collection_nodes(handle)?.len() as f64
            ));
        }
        if let Some(index) = collection_index(property) {
            return Ok(self
                .tag_collection_nodes(handle)?
                .get(index)
                .map_or(Value::Undefined, |&node| Value::Host(node_handle(node))));
        }
        self.tag_collection_named(handle, property)
    }

    pub(super) fn tag_collection_call(
        &mut self,
        handle: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        let argument = args
            .first()
            .ok_or("HTMLCollection methods require one argument.")?;
        let value = match method {
            "item" => {
                let index = timer_integer(argument)? as u32 as usize;
                self.tag_collection_nodes(handle)?
                    .get(index)
                    .map_or(Value::Undefined, |&node| Value::Host(node_handle(node)))
            }
            "namedItem" => {
                let name = self.text(argument)?;
                self.tag_collection_named(handle, &name)?
            }
            _ => return Err("HTMLCollection method is not implemented.".into()),
        };
        Ok(if matches!(value, Value::Undefined) {
            Value::Null
        } else {
            value
        })
    }

    pub(super) fn tag_collection_keys(&mut self, handle: u32) -> Result<Vec<String>, String> {
        let length = self.tag_collection_nodes(handle)?.len();
        if length > 4096 {
            return Err("HTMLCollection enumeration limit reached (4096 indices).".into());
        }
        self.state.work(length)?;
        self.state.copies(length.saturating_mul(5))?;
        Ok((0..length).map(|index| index.to_string()).collect())
    }

    pub(super) fn tag_collection_own_data(
        &mut self,
        handle: u32,
        property: &str,
    ) -> Result<Option<Value>, String> {
        if collection_index(property).is_some() {
            return self
                .tag_collection_get(handle, property)
                .map(|value| (!matches!(value, Value::Undefined)).then_some(value));
        }
        if matches!(property, "length" | "item" | "namedItem") {
            return Ok(None);
        }
        if !matches!(
            self.tag_collection_named(handle, property)?,
            Value::Undefined
        ) {
            // Named properties are nonenumerable in HTMLCollection. The host
            // descriptor API currently represents indexed data only; never
            // return a false enumerable descriptor for a named member.
            return Err("Named HTMLCollection property descriptors are not implemented.".into());
        }
        Ok(None)
    }
}
