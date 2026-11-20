//! Live, bounded tag/class collections. Each handle retains a root
//! and filter, never a snapshot of node references. Queries share DOM work
//! limits and preserve the document's inert-subtree policy.
//! https://dom.spec.whatwg.org/#concept-getelementsbytagname
use super::*;

// Independent of adjacent Window/frame/node identities.
const COLLECTION_BASE:u32=0x8c00_0000;
const COLLECTION_END:u32=0x8d00_0000;
#[cfg(test)]
const MAX_COLLECTIONS:usize=128;

pub(super) struct TagCollection {
    root: usize,
    filter: Filter,
}

enum Filter {
    Tag(String),
    Classes(Vec<String>),
}
impl Filter {
    fn owned_bytes(&self) -> usize {
        match self {
            Self::Tag(name) => name.capacity(),
            Self::Classes(names) => names.capacity()*size_of::<String>()+names.iter().map(String::capacity).sum::<usize>(),
        }
    }
}

pub(super) fn collection_handle_index(handle: u32) -> Option<usize> {
    (COLLECTION_BASE..COLLECTION_END)
        .contains(&handle)
        .then(|| (handle - COLLECTION_BASE) as usize)
}

impl Bridge<'_> {
    pub(super) fn create_tag_collection(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<Value, String> {
        let mut name = self.text(value)?;name.make_ascii_lowercase();
        let root = if object == DOCUMENT {
            0
        } else {
            self.node(object)?
        };
        self.store_live_collection(root, Filter::Tag(name))
    }

    pub(super) fn create_class_collection(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<Value, String> {
        let root = if object == DOCUMENT {
            0
        } else {
            self.node(object)?
        };
        let text = self.text(value)?;
        self.state.work(text.len())?;
        self.state.copies(text.len())?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let count=text.split(class_whitespace).filter(|token|!token.is_empty()).count();
        self.state.work(text.len())?;
        let source=text.capacity();
        let minimum=count.checked_mul(size_of::<&str>()).and_then(|bytes|bytes.checked_add(source)).filter(|bytes|*bytes<=available).ok_or("DOM retained class-filter scratch allowance exhausted.")?;
        let mut tokens=Vec::new();tokens.try_reserve_exact(count).map_err(|_|"Could not reserve class-filter tokens.")?;
        let scratch=tokens.capacity().checked_mul(size_of::<&str>()).and_then(|bytes|bytes.checked_add(source)).filter(|bytes|*bytes<=available).ok_or("DOM retained class-filter token allocation exceeds allowance.")?;
        debug_assert!(scratch>=minimum);
        tokens.extend(text.split(class_whitespace).filter(|token|!token.is_empty()));
        let levels=(usize::BITS-count.leading_zeros()) as usize+1;
        self.state.work(text.len().saturating_mul(levels).saturating_mul(4))?;
        tokens.sort_unstable();tokens.dedup();
        tokens.len().checked_mul(size_of::<String>()).and_then(|bytes|bytes.checked_add(scratch)).filter(|bytes|*bytes<=available).ok_or("DOM retained class-filter name allowance exhausted.")?;
        let mut names=Vec::new();names.try_reserve_exact(tokens.len()).map_err(|_|"Could not reserve class-filter names.")?;
        let mut occupied=scratch.checked_add(names.capacity()*size_of::<String>()).filter(|bytes|*bytes<=available).ok_or("DOM retained class-filter name allocation exceeds allowance.")?;
        for token in tokens {
            if token.len()>available.saturating_sub(occupied){return Err("DOM retained class-filter text allowance exhausted.".into());}
            let mut name=String::new();name.try_reserve_exact(token.len()).map_err(|_|"Could not reserve class-filter text.")?;
            occupied=occupied.checked_add(name.capacity()).filter(|bytes|*bytes<=available).ok_or("DOM retained class-filter text allocation exceeds allowance.")?;
            name.push_str(token);names.push(name);
        }
        drop(text);
        self.store_live_collection(root, Filter::Classes(names))
    }

    fn store_live_collection(&mut self, root: usize, filter: Filter) -> Result<Value, String> {
        let count=self.state.dom.collections.len();
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&count>=MAX_COLLECTIONS{return Err("DOM live tag-collection limit reached (128 per document).".into());}
        if count>=(COLLECTION_END-COLLECTION_BASE) as usize{return Err("HTMLCollection native identity space exhausted.".into());}
        let payload=filter.owned_bytes();
        let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(payload).ok_or("DOM retained collection filter allowance exhausted.")?;
        let (replacement,added)=retained_storage::grow(&self.state.dom.collections,1,available)?;
        if let Some(mut next)=replacement {
            self.state.copies(count*size_of::<TagCollection>())?;
            next.append(&mut self.state.dom.collections);self.state.dom.collections=next;
        }
        let handle=COLLECTION_BASE+count as u32;
        self.state.dom.collections.push(TagCollection{root,filter});self.state.retained+=payload+added;
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
        if matches!(&collection.filter,Filter::Classes(names) if names.is_empty()){return Ok(Vec::new());}
        let mut candidates=self.query_candidates(root)?;
        if matches!(&self.state.dom.collections[index].filter,Filter::Tag(name) if name=="*"){return Ok(candidates);}
        let mut kept=0;
        for position in 0..candidates.len() {
            let node=candidates[position];self.state.work(1)?;
            match &self.state.dom.collections[index].filter {
                Filter::Tag(_)=>self.state.copies(tag(self.doc,node).len())?,
                Filter::Classes(names)=>{
                    let count=names.len();
                    let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{unreachable!()};
                    self.state.work(attrs.len())?;
                    self.state.copies(attrs.iter().map(|attr|attr.name.bytes()).sum())?;
                    let scans=self.doc.attr(node,"class").unwrap_or("").len().checked_mul(count).ok_or("Class collection scan accounting overflow.")?;
                    self.state.work(scans)?;self.state.copies(scans)?;
                }
            }
            let matches=match &self.state.dom.collections[index].filter {
                Filter::Tag(name)=>tag(self.doc,node)==name,
                Filter::Classes(names)=>{
                    let class=self.doc.attr(node,"class").unwrap_or("");
                    names.iter().all(|name|class.split(class_whitespace).any(|token|token==name))
                }
            };
            if matches {
                if kept!=position{self.state.copies(size_of::<usize>())?;candidates[kept]=node;}
                kept+=1;
            }
        }
        candidates.truncate(kept);Ok(candidates)
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
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&length>4096{return Err("HTMLCollection enumeration limit reached (4096 indices).".into());}
        self.state.index_keys(length)
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

fn class_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

impl State {
    // No script runs while keys are staged. Admit the complete vector and each
    // actual string capacity against the current shared host allowance.
    pub(super) fn index_keys(&mut self,length:usize)->Result<Vec<String>,String> {
        use std::fmt::Write;
        self.work(length)?;self.copies(length.saturating_mul(20))?;
        let available=self.retained_limit.saturating_sub(self.retained);
        let minimum=length.checked_mul(size_of::<String>()).filter(|bytes|*bytes<=available).ok_or("DOM retained collection key allowance exhausted.")?;
        let mut keys=Vec::new();keys.try_reserve_exact(length).map_err(|_|"Could not reserve collection keys.")?;
        let mut bytes=keys.capacity().checked_mul(size_of::<String>()).filter(|bytes|*bytes<=available).ok_or("DOM retained collection key allocation exceeds allowance.")?;
        debug_assert!(bytes>=minimum);
        for index in 0..length {
            let digits=if index==0{1}else{index.ilog10() as usize+1};
            if digits>available.saturating_sub(bytes){return Err("DOM retained collection key text allowance exhausted.".into());}
            let mut text=String::new();text.try_reserve_exact(digits).map_err(|_|"Could not reserve collection index text.")?;
            bytes=bytes.checked_add(text.capacity()).filter(|bytes|*bytes<=available).ok_or("DOM retained collection index allocation exceeds allowance.")?;
            write!(&mut text,"{index}").map_err(|_|"Could not format collection index.")?;keys.push(text);
        }
        Ok(keys)
    }
}

#[cfg(test)]
#[path = "script_live_collection_storage_tests.rs"]
mod storage_tests;
