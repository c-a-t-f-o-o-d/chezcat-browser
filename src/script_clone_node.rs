//! Detached copies of ordinary nodes in the main document arena.
//! https://dom.spec.whatwg.org/#concept-node-clone
use super::*;
use script_inert_documents::ImportSource;

#[path="script_import_controls.rs"]
mod import_controls;

pub(super) const CLONE_METHOD: &str = "\0Node.cloneNode";

pub(super) fn ordinary_clone_element(name: &str) -> bool {
    crate::html::valid_custom_element_name(name)
        || (text_container(name) && !matches!(name, "button" | "legend"))
        || matches!(name, "nav" | "main" | "section" | "article" | "aside" | "header"
            | "footer" | "address" | "figure" | "figcaption" | "ol" | "ul" | "dl"
            | "table" | "caption" | "col" | "colgroup" | "thead" | "tbody" | "tfoot"
            | "tr" | "td" | "th" | "hr" | "time" | "slot")
}

pub(super) fn copy_text(text:&str)->Result<String,String>{
    let mut copied=String::new();copied.try_reserve_exact(text.len()).map_err(|_|"Cannot reserve copied DOM text.")?;
    copied.push_str(text);Ok(copied)
}
pub(super) fn copy_node_kind(kind: &NodeKind) -> Result<NodeKind,String> {
    Ok(match kind {
        NodeKind::Element { tag, id, href, attrs } => {
            let mut copied=Vec::new();copied.try_reserve_exact(attrs.len()).map_err(|_|"Cannot reserve copied element attributes.")?;
            for attr in attrs {
                let namespace=attr.name.namespace.as_ref().map(|ns|Ok::<_,String>(crate::html::AttributeNamespace{
                    uri:copy_text(&ns.uri)?,prefix:ns.prefix.as_deref().map(copy_text).transpose()?,local:copy_text(&ns.local)?
                })).transpose()?;
                copied.push(crate::html::Attribute{name:crate::html::AttributeName{qualified:copy_text(&attr.name.qualified)?,namespace},value:attr.value.try_clone()?});
            }
            NodeKind::Element{tag:copy_text(tag)?,id:id.as_deref().map(copy_text).transpose()?,href:href.as_deref().map(copy_text).transpose()?,attrs:copied}
        },
        NodeKind::Text(text)=>NodeKind::Text(text.try_clone()?),
        NodeKind::Comment(text)=>NodeKind::Comment(text.try_clone()?),
        NodeKind::ProcessingInstruction{target,data}=>NodeKind::ProcessingInstruction{target:copy_text(target)?,data:data.try_clone()?},
        NodeKind::DocumentFragment=>NodeKind::DocumentFragment,
        NodeKind::DocumentType(data)=>NodeKind::DocumentType(data.try_clone()?),
    })
}

struct DetachedCopy {
    node: Node,
    parent: Option<usize>,
    unsafe_nonce: bool,
}

impl Bridge<'_> {
    pub(super) fn clone_node_receiver(&self, object: u32) -> bool {
        self.root_node_receiver(object) || self.inert_clone_receiver(object)
    }

    pub(super) fn validate_clone_receiver(&self, object: u32) -> Result<(), (&'static str, String)> {
        if !self.clone_node_receiver(object) {
            return Err(("TypeError", "cloneNode requires a genuine accessible Node.".into()));
        }
        if !self.inert_clone_receiver(object) && !node_index(object).is_some_and(|node| self.dom_accessible(node)) {
            return Err(("NotSupportedError", "Cloning this Node interface or child document is unavailable.".into()));
        }
        Ok(())
    }

    pub(super) fn validate_import_source(&self, args: &[Value]) -> Result<(), (&'static str, String)> {
        let Some(Value::Host(source)) = args.first() else {
            return Err(("TypeError", "importNode requires a genuine Node.".into()));
        };
        if *source == DOCUMENT || self.inert_document(*source) || self.active_frame_document(*source) {
            return Err(("NotSupportedError", "importNode cannot import a Document.".into()));
        }
        if self.clone_node_receiver(*source) || self.inert_node_type(*source).is_some() {
            return Ok(());
        }
        Err(("TypeError", "importNode requires a genuine Node.".into()))
    }

    fn template_import_source(&self,object:u32)->Option<ImportSource<'_>>{
        self.inert_import_source(object).or_else(||{if self.template_receiver(object){let index=node_index(object)?;Some(ImportSource::Parsed{node:self.doc.nodes.get(index)?,unsafe_nonce:self.doc.nonce_unsafe_nodes.contains(&index)})}else{None}})
    }
    fn template_import_children(&self,object:u32)->Option<&[u32]>{if self.template_receiver(object){Some(&[])}else{self.inert_children(object)}}
    pub(super) fn import_node(&mut self, args: &[Value], fields: Option<&[(&str, Value)]>) -> Result<Value, String> {
        let Some(Value::Host(source)) = args.first() else { return Err("importNode requires a Node.".into()); };
        let deep = if let Some(fields) = fields {
            !fields.iter().find(|(key, _)| *key == "selfOnly").is_some_and(|(_, value)| value.truthy())
        } else {
            args.get(1).is_some_and(|value| matches!(value, Value::Null) || value.truthy())
        };
        if !self.inert_clone_receiver(*source)&&!self.template_receiver(*source) {
            return self.clone_node(*source, deep);
        }
        if !deep&&self.inert_interface(*source)==Some("HTMLTextAreaElement"){return Err("Shallow inert TEXTAREA import requires raw-value cloning; no node was created.".into());}

        let base = self.doc.nodes.len();
        let remaining = self.state.node_limit.saturating_sub(base.saturating_add(self.state.blank_frame_nodes))
            .min(((STYLE_BASE - NODE_BASE) as usize).saturating_sub(base));
        if remaining == 0 { return Err("DOM appended-node limit reached during import.".into()); }
        let mut pending = vec![(*source, None, 1usize)];
        let mut staged = Vec::new();
        let mut bytes = 0usize;
        let mut templates=Vec::new();
        while let Some(&(handle, parent, depth)) = pending.get(staged.len()) {
            let children = self.template_import_children(handle).ok_or("Inert import source is unavailable.")?;
            let count = if deep { children.len() } else { 0 };
            if count > remaining - pending.len() {
                return Err("DOM appended-node limit reached during import.".into());
            }
            if count != 0 && depth >= dom_tree::MAX_DOM_DEPTH {
                return Err("DOM depth limit reached during import.".into());
            }
            let attributes = match self.template_import_source(handle) {
                Some(ImportSource::Parsed { node: Node { kind: NodeKind::Element { attrs, .. }, .. }, .. }) => attrs.iter().fold(attrs.len(),|n,a|n.saturating_add(a.name.bytes()).saturating_add(a.value.used_bytes())),
                _ => 0,
            };
            self.state.work(count.saturating_add(attributes).saturating_add(1))?;
            if self.template_receiver(handle){templates.try_reserve(1).map_err(|_|"Cannot reserve main template import hosts.")?;templates.push((handle,node_handle(base+staged.len())));}
            let source = self.template_import_source(handle).ok_or("Inert import source is unavailable.")?;
            let cost = match &source {
                ImportSource::Parsed { node, .. } => {
                    let mut metadata = 0usize;
                    if let NodeKind::Element { tag, href, attrs, .. } = &node.kind {
                        let svg=node.namespace==crate::html::Namespace::Svg;
                        if svg{if !crate::html::svg_dom_transfer(node){return Err("SVG import requires supported passive shapes/attributes and local references.".into());}for attr in attrs{if attr.name.html_is("style"){validate_style(&attr.value,self.state.limits.host.string_bytes)?;}}}
                        if !svg&&(node.namespace != crate::html::Namespace::Html || !(ordinary_clone_element(tag)||matches!(tag.as_str(),"input"|"textarea"|"select"|"option"|"optgroup"|"button"|"img"|"style"|"template"))) {
                            return Err("Import supports ordinary HTML containers and inert data nodes.".into());
                        }
                        if tag=="img"&&(count!=0||attrs.iter().any(|attr|attr.name.html_is("srcset"))){return Err("Image import requires a plain childless image without srcset selection.".into());}
                        if tag=="style"&&self.template_import_children(handle).unwrap().iter().take(count).any(|&child|self.inert_node_type(child)!=Some(3)){return Err("Imported STYLE requires plain direct text children.".into());}
                        if !svg&&attrs.iter().any(|attr| attr.name.namespace.is_some()
                            || script_element_properties::event_handler_content_attribute(tag,&attr.name) || tag!="style" && attr.name.html_is("is")) {
                            return Err("Import cannot transfer active handlers or unsupported namespaced attributes.".into());
                        }
                        metadata = tag.len().saturating_add(href.as_ref().map_or(0, String::len));
                    }
                    copied_node_bytes(node).saturating_add(metadata)
                }
                ImportSource::Element(tag) => {
                    if !(ordinary_clone_element(tag)||matches!(*tag,"input"|"textarea"|"select"|"option"|"optgroup"|"button"|"img"|"style"|"template")) {
                        return Err("Import supports ordinary HTML containers and inert data nodes.".into());
                    }
                    tag.len()
                }
                ImportSource::Text(data) => data.used_bytes(),
                ImportSource::ProcessingInstruction { target, data } => target.len().saturating_add(data.used_bytes()),
                ImportSource::Fragment => 0,
            };
            bytes = bytes.checked_add(cost).ok_or("DOM import size overflow.")?;
            if bytes > self.state.retained_limit.saturating_sub(self.state.retained) {
                return Err("DOM retained text/attribute growth limit reached during import.".into());
            }
            self.state.copies(cost)?;
            let source = self.template_import_source(handle).ok_or("Inert import source is unavailable.")?;
            let (kind, namespace, unsafe_nonce) = match source {
                ImportSource::Parsed { node, unsafe_nonce } => (copy_node_kind(&node.kind)?, node.namespace, unsafe_nonce),
                ImportSource::Element(tag) => (NodeKind::Element {
                    tag: tag.into(), id: None, href: None, attrs: Vec::new(),
                }, crate::html::Namespace::Html, false),
                ImportSource::Text(data) => (NodeKind::Text(data.try_clone()?), crate::html::Namespace::None, false),
                ImportSource::ProcessingInstruction { target, data } => (NodeKind::ProcessingInstruction {
                    target: target.into(), data: data.try_clone()?,
                }, crate::html::Namespace::None, false),
                ImportSource::Fragment => (NodeKind::DocumentFragment, crate::html::Namespace::None, false),
            };
            let extra=copied_payload_extra(&kind);
            bytes=bytes.checked_add(extra).filter(|bytes|*bytes<=self.state.retained_limit.saturating_sub(self.state.retained)).ok_or("Imported element attribute backing exceeds allowance.")?;
            self.state.copies(extra)?;
            let start = base + pending.len();
            let index = base + staged.len();
            pending.try_reserve(count).map_err(|_| "Cannot reserve DOM import traversal.")?;
            staged.try_reserve(1).map_err(|_| "Cannot reserve DOM import nodes.")?;
            let children = self.template_import_children(handle).ok_or("Inert import source is unavailable.")?;
            pending.extend(children.iter().take(count).map(|&child| (child, Some(index), depth + 1)));
            let mut children = Vec::new();
            self.state.copies(count.checked_mul(size_of::<usize>()).ok_or("Imported child-link copy work overflow.")?)?;
            children.try_reserve_exact(count).map_err(|_| "Cannot reserve DOM import child links.")?;
            bytes = bytes.checked_add(child_link_bytes(&children))
                .filter(|bytes| *bytes <= self.state.retained_limit.saturating_sub(self.state.retained))
                .ok_or("Imported child-link backing exceeds allowance.")?;
            children.extend(start..start + count);
            staged.push(DetachedCopy { node: Node { kind, namespace, children }, parent, unsafe_nonce });
        }
        let form_plan=self.prepare_imported_controls(&mut staged,bytes)?;
        let bytes=bytes.checked_add(form_plan.as_ref().map_or(0,|plan|plan.retained_bytes)).ok_or("Imported native control payload overflow.")?;
        if templates.is_empty(){return self.publish_detached_copy(staged,bytes,form_plan);}
        let checkpoint=self.template_import_checkpoint();
        let result=(||{
            // Reserve the outer identities before content clones can claim
            // native slots. These unpublished placeholders roll back together.
            self.reserve_detached_import_slots(staged.len(),bytes)?;
            self.prepare_main_template_contents(&templates,deep)?;
            self.publish_detached_copy_at(base,true,staged,bytes,form_plan)
        })();
        if result.is_err(){self.rollback_template_import(checkpoint);}result
    }

    pub(super) fn clone_node(&mut self, object: u32, deep: bool) -> Result<Value, String> {
        self.validate_clone_receiver(object).map_err(|(_, message)| message)?;
        if self.template_receiver(object)&&!self.inert_clone_receiver(object){return self.import_node(&[Value::Host(object),Value::Bool(deep)],None);}
        if self.inert_clone_receiver(object) {return self.clone_inert_tree(object,deep);}
        let root = self.dom_node(object)?;
        let base = self.doc.nodes.len();
        let remaining = self.state.node_limit.saturating_sub(base.saturating_add(self.state.blank_frame_nodes))
            .min(((STYLE_BASE - NODE_BASE) as usize).saturating_sub(base));
        if remaining == 0 {
            return Err("DOM appended-node limit reached during cloning.".into());
        }

        // Each parent's direct children occupy a contiguous range in the plan.
        // All checks and copies finish before the first new handle is published.
        let mut plan = vec![(root, None, 1usize)];
        let mut staged = Vec::new();
        let mut textareas = Vec::new();
        let mut radios = Vec::new();
        let mut bytes = 0usize;
        while staged.len() < plan.len() {
            let offset = staged.len();
            let (source, parent, depth) = plan[offset];
            self.validate_svg_node(source)?;
            let image=tag(self.doc,source)=="img";
            let style=tag(self.doc,source)=="style";
            if image{self.validate_plain_image(source)?;}
            let item = &self.doc.nodes[source];
            let children = if deep { item.children.as_slice() } else { &[] };
            if style&&children.iter().any(|&child|!matches!(self.doc.nodes[child].kind,NodeKind::Text(_))){return Err("Cloned STYLE requires plain direct text children.".into());}
            self.state.work(1 + children.len())?;
            if children.len() > remaining - plan.len() {
                return Err("DOM appended-node limit reached during cloning.".into());
            }
            if !children.is_empty() && depth >= dom_tree::MAX_DOM_DEPTH {
                return Err("DOM depth limit reached during cloning.".into());
            }
            let first = base + plan.len();
            plan.try_reserve(children.len()).map_err(|_| "DOM clone plan reservation failed.")?;
            plan.extend(children.iter().map(|&child| (child, Some(base + offset), depth + 1)));

            let mut metadata_bytes = 0;
            if let NodeKind::Element { tag, href, attrs, .. } = &item.kind {
                self.state.work(attrs.len())?;
                let textarea = tag == "textarea" && self.owned_native_textarea(source);
                let radio = tag == "input" && self.owned_native_radio(source);
                let svg=item.namespace==crate::html::Namespace::Svg;
                if !svg&&(item.namespace != crate::html::Namespace::Html || !(ordinary_clone_element(tag) || textarea || radio || image || style)
                    || attrs.iter().any(|attr| attr.name.namespace.is_none()
                        && (script_element_properties::event_handler_content_attribute(tag,&attr.name) || tag!="style" && attr.name.html_is("is")))) {
                    return Err("Cloning this element or its native state is not implemented.".into());
                }
                if textarea || radio {
                    if self.forms.controls.len().saturating_add(textareas.len()).saturating_add(radios.len()) >= forms::MAX_CONTROLS {
                        return Err("Native form control limit reached before native control cloning.".into());
                    }
                    let controls=if textarea {&mut textareas} else {&mut radios};
                    controls.try_reserve(1).map_err(|_| "Native control clone plan reservation failed.")?;
                    controls.push((source, base + offset));
                }
                metadata_bytes = tag.len() + href.as_ref().map_or(0, String::len);
            }
            let size = copied_node_bytes(item) + metadata_bytes;
            bytes = bytes.checked_add(size).ok_or("DOM clone size overflow.")?;
            if bytes > self.state.retained_limit.saturating_sub(self.state.retained) {
                return Err("DOM retained text/attribute growth limit reached during cloning.".into());
            }
            self.state.copies(size)?;
            let kind = copy_node_kind(&item.kind)?;
            let extra=copied_payload_extra(&kind);
            bytes=bytes.checked_add(extra).filter(|bytes|*bytes<=self.state.retained_limit.saturating_sub(self.state.retained)).ok_or("Copied element attribute backing exceeds allowance.")?;
            self.state.copies(extra)?;
            let unsafe_nonce = self.doc.nonce_unsafe_nodes.contains(&source);
            self.state.copies(children.len().checked_mul(size_of::<usize>()).ok_or("Cloned child-link copy work overflow.")?)?;
            let mut child_links = Vec::new();
            child_links.try_reserve_exact(children.len()).map_err(|_| "Cannot reserve cloned child links.")?;
            bytes = bytes.checked_add(child_link_bytes(&child_links))
                .filter(|bytes| *bytes <= self.state.retained_limit.saturating_sub(self.state.retained))
                .ok_or("Cloned child-link backing exceeds allowance.")?;
            child_links.extend(first..first + children.len());
            staged.push(DetachedCopy {
                node: Node { kind, namespace: item.namespace, children: child_links },
                parent,
                unsafe_nonce,
            });
        }

        let retained_limit = self.state.retained_limit.saturating_sub(self.state.retained)
            .saturating_sub(bytes);
        let mut form_plan = if textareas.is_empty() { None } else {
            let state = &mut *self.state;
            Some(self.forms.prepare_textarea_clones(self.doc, &textareas, deep, retained_limit, |work, copies| {
                state.work(work)?;
                state.copies(copies)
            })?)
        };
        if !radios.is_empty() {
            let mut plan=form_plan.unwrap_or_else(||forms::SelectFormPlan {
                additions:std::collections::BTreeMap::new(),metadata:Vec::new(),radio_checked:Vec::new(),retained_bytes:0,
            });
            let state=&mut *self.state;
            self.forms.add_radio_clones(self.doc,&radios,base,&mut plan,retained_limit,|work,copies| {
                state.work(work)?;state.copies(copies)
            })?;
            form_plan=Some(plan);
        }
        let bytes = bytes.checked_add(form_plan.as_ref().map_or(0, |plan| plan.retained_bytes))
            .ok_or("DOM/native control clone size overflow.")?;
        self.publish_detached_copy(staged, bytes, form_plan)
    }

    fn publish_detached_copy(&mut self, staged: Vec<DetachedCopy>, bytes: usize,
        form_plan: Option<forms::SelectFormPlan>) -> Result<Value, String>
    {
        let base = self.doc.nodes.len();
        self.publish_detached_copy_at(base,false,staged,bytes,form_plan)
    }

    fn reserve_detached_import_slots(&mut self,count:usize,bytes:usize)->Result<(),String> {
        let available=self.state.retained_limit.checked_sub(self.state.retained)
            .and_then(|n|n.checked_sub(bytes)).ok_or("Template import outer payload allowance exhausted.")?;
        let (growth,backing)=self.prepare_main_nodes(count,available)?;
        self.state.replace_storage(0,bytes.checked_add(backing).ok_or("Template import outer bundle overflow.")?,bytes)?;
        self.commit_main_nodes(growth);
        for _ in 0..count {
            self.doc.nodes.push(Node {namespace:crate::html::Namespace::None,kind:NodeKind::DocumentFragment,children:Vec::new()});
            self.state.tree.parents.push(None);self.state.tree.active.push(false);self.state.dom.connected.push(false);
        }
        Ok(())
    }

    fn publish_detached_copy_at(&mut self,base:usize,reserved:bool,staged:Vec<DetachedCopy>,bytes:usize,
        form_plan:Option<forms::SelectFormPlan>)->Result<Value,String>
    {
        let count = staged.len();
        let nonce_count = staged.iter().filter(|copy| copy.unsafe_nonce).count();
        let end=base.checked_add(count).ok_or("DOM clone sibling identity overflow.")?;
        let mut links=0usize;
        for copy in &staged {
            links=links.checked_add(copy.node.children.len()).ok_or("DOM clone sibling work overflow.")?;
            if copy.node.children.iter().any(|&child|child<base||child>=end) {return Err("DOM clone sibling link escaped the staged forest.".into());}
        }
        self.state.work(links)?;
        self.state.copies(links.checked_mul(size_of::<usize>()).ok_or("DOM clone sibling copy overflow.")?)?;
        let (node_growth,node_bytes)=if reserved {
            if end>self.doc.nodes.len(){return Err("Template import reserved range disappeared.".into());}
            self.state.work(count)?;
            for node in base..end {
                if !matches!(self.doc.nodes[node].kind,NodeKind::DocumentFragment)
                    || self.doc.nodes[node].namespace!=crate::html::Namespace::None
                    || self.inert_adopted_node(node_handle(node))
                    || self.state.dom.created.contains(&node) || self.state.tree.parents[node].is_some()
                    || self.state.dom.connected[node] {
                    return Err("Template import reserved destination changed before publication.".into());
                }
            }
            (None,0)
        } else {
            let available = self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(bytes)
                .ok_or("DOM clone node/payload backing allowance exhausted.")?;
            let (growth,bytes)=self.prepare_main_nodes(count,available)?;(Some(growth),bytes)
        };
        self.doc.nonce_unsafe_nodes.try_reserve(nonce_count).map_err(|_| "DOM clone nonce reservation failed.")?;
        if reserved {
            self.state.copies(count.checked_mul(size_of::<Node>()).ok_or("Template import reserved record copy overflow.")?)?;
        } else {
            self.state.replace_storage(0, bytes.checked_add(node_bytes).ok_or("DOM clone node/payload storage overflow.")?, bytes)?;
        }
        if let Some(growth)=node_growth {self.commit_main_nodes(growth);}
        for (offset,copy) in staged.into_iter().enumerate() {
            let node = base+offset;
            for (slot,&child) in copy.node.children.iter().enumerate() {self.state.tree.sibling_slots[child]=slot;}
            if reserved {self.doc.nodes[node]=copy.node;}else{self.doc.nodes.push(copy.node);}
            self.state.dom.created.insert(node);
            self.state.detached_nodes.insert(node);
            if reserved {self.state.tree.parents[node]=copy.parent;}else{
                self.state.tree.parents.push(copy.parent);
                self.state.tree.active.push(false);
                self.state.dom.connected.push(false);
            }
            if copy.unsafe_nonce { self.doc.nonce_unsafe_nodes.insert(node); }
        }
        if let Some(plan) = form_plan { plan.commit(self.forms); }
        Ok(Value::Host(node_handle(base)))
    }
}

#[cfg(test)]
#[path = "script_radio_clone_tests.rs"]
mod radio_clone_tests;
