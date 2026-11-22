//! Live, bounded tag/class/direct-element collections. Each handle retains a root
//! and filter, never a snapshot of node references. Queries share DOM work
//! limits and preserve the document's inert-subtree policy.
//! https://dom.spec.whatwg.org/#concept-getelementsbytagname
use super::*;

// Independent of adjacent Window/frame/node identities.
const COLLECTION_BASE:u32=0x8c00_0000;
const COLLECTION_END:u32=0x8d00_0000;
pub(super) fn operation(method:&str)->Option<(&'static str,bool)>{use crate::javascript::*;match method{DOCUMENT_GET_ELEMENTS_TAG=>Some(("getElementsByTagName",true)),DOCUMENT_GET_ELEMENTS_CLASS=>Some(("getElementsByClassName",true)),ELEMENT_GET_ELEMENTS_TAG=>Some(("getElementsByTagName",false)),ELEMENT_GET_ELEMENTS_CLASS=>Some(("getElementsByClassName",false)),_=>None}}
#[cfg(test)]
const MAX_COLLECTIONS:usize=128;

pub(super) struct TagCollection {
    root: u32,
    filter: Filter,
}

enum Filter {
    Children(u32),
    Tag(DomString,bool),
    Classes(Vec<String>),
}
fn qualified_matches(local:crate::dom_string::DomStringRef<'_>,prefix:Option<&DomString>,name:&DomString,lowercase:bool)->bool{
    let actual=prefix.into_iter().flat_map(DomString::code_units).chain(prefix.map(|_|0x3a)).chain(local.code_units());actual.eq(name.code_units().map(|u|if lowercase&&(0x41..=0x5a).contains(&u){u+0x20}else{u}))
}
fn tag_matches(node:&Node,name:&DomString,html:bool)->Result<bool,String>{let NodeKind::Element{tag,expanded_name,..}=&node.kind else{return Ok(false);};if name=="*"{return Ok(true);}let prefix=crate::html::element_names::prefix(node.namespace,expanded_name)?;let local=crate::html::element_names::local(node.namespace,tag,expanded_name)?;Ok(qualified_matches(local,prefix,name,html&&node.namespace==crate::html::Namespace::Html))}
impl Filter {
    fn owned_bytes(&self) -> usize {
        match self {
            Self::Children(_) => 0,
            Self::Tag(name,_) => name.capacity(),
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
    pub(super) fn collection_interface_receiver(&self,object:u32)->Option<bool>{if object==DOCUMENT||self.inert_node_type(object)==Some(9){Some(true)}else if self.inert_node_type(object)==Some(1)||node_index(object).is_some_and(|index|self.dom_accessible(index)&&matches!(self.doc.nodes[index].kind,NodeKind::Element{..})){Some(false)}else{None}}
    pub(super) fn canonical_collection_member(&self,object:u32,property:&str)->bool{matches!(property,"getElementsByTagName"|"getElementsByClassName")&&self.collection_interface_receiver(object).is_some()&&!self.form_named_control(object,property)}
    fn tag_collection_handles(&mut self,handle:u32)->Result<Vec<u32>,String>{
        let index=collection_handle_index(handle).ok_or("Invalid HTMLCollection handle.")?;let root=self.state.dom.collections.get(index).ok_or("Missing live collection.")?.root;let owner=root;
        if self.inert_node_type(owner).is_none(){let nodes=self.tag_collection_nodes(handle)?;self.state.copies(nodes.len().checked_mul(size_of::<u32>()).ok_or("Collection handle conversion overflow.")?)?;let mut result=Vec::new();result.try_reserve_exact(nodes.len()).map_err(|_|"Cannot reserve live collection handles.")?;result.extend(nodes.into_iter().map(node_handle));return Ok(result);}
        if matches!(&self.state.dom.collections[index].filter,Filter::Classes(names) if names.is_empty()){return Ok(Vec::new());}
        let mut pending=Vec::new();let mut result=Vec::new();let count=self.inert_children(owner).ok_or("Adopted collection root is unavailable.")?.len();self.state.work(count)?;self.state.copies(count.checked_mul(size_of::<u32>()).ok_or("Collection root traversal overflow.")?)?;pending.try_reserve_exact(count).map_err(|_|"Cannot reserve adopted collection traversal.")?;pending.extend(self.inert_children(owner).unwrap().iter().rev().copied());
        let maximum=self.inert_node_count();let mut visited=0usize;
        while let Some(node)=pending.pop(){
            visited=visited.checked_add(1).filter(|&count|count<=maximum).ok_or("Cyclic adopted collection topology.")?;self.state.work(1)?;
            let source=self.inert_import_source(node);let element=matches!(source,Some(script_inert_documents::ImportSource::Element(_)|script_inert_documents::ImportSource::Parsed{node:Node{kind:NodeKind::Element{..},..},..}));
            let work=match &source{Some(script_inert_documents::ImportSource::Element(tag))=>tag.len(),Some(script_inert_documents::ImportSource::Parsed{node:Node{kind:NodeKind::Element{tag,attrs,expanded_name,..},..},..})=>attrs.iter().fold(tag.len()+attrs.len()+crate::html::element_names::copied_bytes(expanded_name),|n,attr|n.saturating_add(attr.name.bytes())),_=>0};self.state.work(work)?;
            let classes=match &self.state.dom.collections[index].filter{Filter::Classes(names)=>names.len(),_=>0};
            if classes!=0{let length=match self.inert_import_source(node){Some(script_inert_documents::ImportSource::Parsed{node:Node{kind:NodeKind::Element{attrs,..},..},..})=>attrs.iter().find(|attr|attr.name.html_is("class")).map(|attr|attr.value.scalar()).transpose()?.unwrap_or("").len(),_=>0};let scans=length.checked_mul(classes).ok_or("Adopted class scan overflow.")?;self.state.work(scans)?;self.state.copies(scans)?;}
            let filter=&self.state.dom.collections[index].filter;let source=self.inert_import_source(node);
            if matches!(source,Some(script_inert_documents::ImportSource::Parsed{node:Node{namespace:crate::html::Namespace::UnknownForeign,..},..})){return Err("Adopted collection requires known namespace provenance.".into());}
            let matched=if !element{false}else{match (filter,source){
                (Filter::Tag(name,html),Some(script_inert_documents::ImportSource::Element(tag)))=>name=="*"||qualified_matches(crate::dom_string::DomStringRef::Scalar(tag),None,name,*html),
                (Filter::Tag(name,html),Some(script_inert_documents::ImportSource::Parsed{node,..}))=>tag_matches(node,name,*html)?,
                (Filter::Classes(names),Some(script_inert_documents::ImportSource::Parsed{node:Node{kind:NodeKind::Element{attrs,..},..},..}))=>{let class=attrs.iter().find(|attr|attr.name.html_is("class")).map(|attr|attr.value.scalar()).transpose()?.unwrap_or("");names.iter().all(|name|class.split(class_whitespace).any(|token|token==name))},
                (Filter::Classes(_),_)=>false,_=>return Err("Unsupported adopted collection filter.".into()),
            }};
            if matched{self.state.copies(size_of::<u32>())?;result.try_reserve(1).map_err(|_|"Cannot reserve adopted collection result.")?;result.push(node);}
            if self.template_receiver(node){continue;}
            let count=self.inert_children(node).ok_or("Adopted collection child is unavailable.")?.len();self.state.work(count)?;self.state.copies(count.checked_mul(size_of::<u32>()).ok_or("Adopted collection traversal overflow.")?)?;pending.try_reserve(count).map_err(|_|"Cannot reserve adopted collection descendants.")?;pending.extend(self.inert_children(node).unwrap().iter().rev().copied());
        }
        Ok(result)
    }
    pub(super) fn create_tag_collection(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<Value, String> {
        let name = self.dom_text(value)?;
        if object!=DOCUMENT&&!matches!(self.inert_node_type(object),Some(1|9)){self.node(object)?;}
        let root=object;
        let html=self.inert_document_kind_for_node(object).unwrap_or(self.doc.kind)==crate::html::DocumentKind::Html;
        self.store_live_collection(root, Filter::Tag(name,html))
    }

    pub(super) fn create_class_collection(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<Value, String> {
        if object!=DOCUMENT&&!matches!(self.inert_node_type(object),Some(1|9)){self.node(object)?;}
        let root=object;
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

    fn store_live_collection(&mut self, root: u32, filter: Filter) -> Result<Value, String> {
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
        let root=if collection.root==DOCUMENT{0}else{node_index(collection.root).ok_or("Native collection root is unavailable.")?};
        if matches!(collection.filter,Filter::Children(_)){return Err("Children views use stable native handles.".into());}
        if matches!(&collection.filter,Filter::Classes(names) if names.is_empty()){return Ok(Vec::new());}
        let mut candidates=self.query_candidates(root)?;
        if matches!(&self.state.dom.collections[index].filter,Filter::Tag(name,_) if name=="*"){return Ok(candidates);}
        let mut kept=0;
        for position in 0..candidates.len() {
            let node=candidates[position];self.state.work(1)?;
            match &self.state.dom.collections[index].filter {
                Filter::Children(_)=>unreachable!(),
                Filter::Tag(_,_)=>{let NodeKind::Element{tag,expanded_name,..}=&self.doc.nodes[node].kind else{unreachable!()};self.state.copies(tag.len().saturating_add(crate::html::element_names::copied_bytes(expanded_name)))?;},
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
                Filter::Children(_)=>unreachable!(),
                Filter::Tag(name,html)=>tag_matches(&self.doc.nodes[node],name,*html)?,
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
        if let Some(owner)=self.children_collection_owner(handle){
            if name.is_empty(){return Ok(Value::Undefined);}
            for node in self.parent_element_nodes(owner)? {
                if self.collection_element_named_match(node,name)?{return Ok(Value::Host(node));}
            }
            return Ok(Value::Undefined);
        }
        if name.is_empty() {
            return Ok(Value::Undefined);
        }
        for node in self.tag_collection_handles(handle)? {
            if self.collection_element_named_match(node,name)?{return Ok(Value::Host(node));}
        }
        Ok(Value::Undefined)
    }

    pub(super) fn tag_collection_get(
        &mut self,
        handle: u32,
        property: &str,
    ) -> Result<Value, String> {
        if handle==DOCUMENT_IMAGES{return self.image_collection_get(property);}
        if self.collection_prototype_member(property)&&property!="length" {return Ok(Value::Undefined);}
        if let Some(owner)=self.children_collection_owner(handle){
            if property=="length"{return self.parent_node_get(owner,"childElementCount");}
            if let Some(index)=collection_index(property){return Ok(self.parent_element_nodes(owner)?.get(index).copied().map_or(Value::Undefined,Value::Host));}
            return self.tag_collection_named(handle,property);
        }
        if property == "length" {
            return Ok(Value::Number(
                self.tag_collection_handles(handle)?.len() as f64
            ));
        }
        if let Some(index) = collection_index(property) {
            return Ok(self
                .tag_collection_handles(handle)?
                .get(index)
                .map_or(Value::Undefined, |&node| Value::Host(node)));
        }
        self.tag_collection_named(handle, property)
    }

    pub(super) fn tag_collection_call(
        &mut self,
        handle: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if handle==DOCUMENT_IMAGES{return self.call(handle,method,args);}
        let argument = args
            .first()
            .ok_or("HTMLCollection methods require one argument.")?;
        let value = match method {
            "item" => {
                let index = timer_integer(argument)? as u32 as usize;
                if let Some(owner)=self.children_collection_owner(handle){self.parent_element_nodes(owner)?.get(index).copied().map_or(Value::Undefined,Value::Host)}
                else {self.tag_collection_handles(handle)?.get(index).map_or(Value::Undefined, |&node| Value::Host(node))}
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
        let length = if let Some(owner)=self.children_collection_owner(handle){self.parent_element_nodes(owner)?.len()}else{self.tag_collection_handles(handle)?.len()};
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

impl Bridge<'_> {
    fn collection_element_named_match(&mut self,object:u32,name:&str)->Result<bool,String>{
        let node=if let Some((owner,node))=self.frame_contains_node(object){self.contains_child_document(owner)?.nodes.get(node)}
            else if self.inert_node_type(object).is_some(){self.inert_collection_element_source(object)}
            else {node_index(object).and_then(|index|self.doc.nodes.get(index))};
        let (work,found)=if let Some(Node{namespace,kind:NodeKind::Element{attrs,..},..})=node {
            let html=*namespace==crate::html::Namespace::Html;
            let work=attrs.iter().fold(attrs.len(),|sum,attr|sum.saturating_add(attr.name.qualified.len()).saturating_add(attr.value.len()));
            (work,attrs.iter().any(|attr|(attr.name.html_is("id")||html&&attr.name.html_is("name"))&&attr.value==name))
        }else{(1,false)};
        self.state.work(work)?;Ok(found)
    }
    pub(super) fn collection_receiver(&self,handle:u32)->bool {handle==DOCUMENT_IMAGES||collection_handle_index(handle).is_some_and(|index|index<self.state.dom.collections.len())}
    pub(super) fn collection_prototype_member(&self,property:&str)->bool {matches!(property,"constructor"|"length"|"item"|"namedItem")}
    fn children_collection_owner(&self,handle:u32)->Option<u32>{
        let record=self.state.dom.collections.get(collection_handle_index(handle)?)?;
        if let Filter::Children(owner)=record.filter {Some(owner)}else{None}
    }
    pub(super) fn collection_owner_window(&self,handle:u32)->Option<u32>{
        if !self.collection_receiver(handle){return None;}
        Some(self.children_collection_owner(handle).and_then(|owner|self.frame_instance_window(owner)).unwrap_or(WINDOW))
    }
    pub(super) fn create_children_collection(&mut self,owner:u32)->Result<Value,String>{
        // SameObject lookup needs no second owner registry or promotion update.
        // Pay the existing registry scan before either returning or publishing.
        self.state.work(self.state.dom.collections.len().saturating_add(1))?;
        if let Some(index)=self.state.dom.collections.iter().position(|record|matches!(record.filter,Filter::Children(parent) if parent==owner)){
            return Ok(Value::Host(COLLECTION_BASE+index as u32));
        }
        self.store_live_collection(0,Filter::Children(owner))
    }
}
