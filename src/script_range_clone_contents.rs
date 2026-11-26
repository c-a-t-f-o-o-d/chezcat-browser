//! Native range content copying into one unpublished identity bundle.
//! https://dom.spec.whatwg.org/#concept-range-clone
use super::*;
use crate::scripting::script_ranges::{Point,Record};
use std::cmp::Ordering;

struct CopyItem {
    source: Option<u32>,
    owner: u32,
    parent: Option<usize>,
    template: Option<usize>,
    units: Option<(usize,usize)>,
    children: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture()->(Document,Forms,Page){
        let mut d=crate::html::parse("<!doctype html><body><div id=root><b>A😀B</b><span data-note=seed>Middle</span><em>End</em></div></body>");
        d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();
        assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);
        p.begin_job();p.runtime.execute("var root=document.getElementById('root'),r=document.createRange();r.setStart(root.firstChild.firstChild,1);r.setEnd(root.lastChild.firstChild,2);0;",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();
        (d,f,p)
    }
    #[test]
    fn range_clone_contents_exact_retained_storage_for_partial_and_template_forests(){
        for template in [false,true] {
            let(mut d,mut f,mut p)=fixture();
            if template {
                p.begin_job();
                p.runtime.execute("var t=document.createElement('template');t.innerHTML='<span data-note=wide>Saved</span><template><b>Nested</b></template>';root.appendChild(t);var warm=t.content.firstChild,nested=t.content.lastChild.content;r.selectNode(t);",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();
            }
            p.begin_job();let record=p.state.ranges[0];
            let mut b=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};
            let before=(b.state.retained,b.template_import_backing(),b.state.inert_documents.generated.len());
            let Value::Host(copy)=b.clone_range_contents(record).unwrap() else {panic!("native fragment")};
            let mut pending=vec![copy];let mut seen=std::collections::HashSet::new();let mut payload=0;
            while let Some(handle)=pending.pop(){
                assert!(seen.insert(handle),"copied tree must have independent identities");
                let node=b.state.inert_documents.node(handle).unwrap();
                payload+=node.children.capacity()*size_of::<u32>();pending.extend_from_slice(&node.children);
                payload+=match &node.kind {
                    Kind::Element(s)=>s.capacity(),Kind::Text(s)=>s.capacity(),
                    Kind::ProcessingInstruction{target,data}=>target.capacity()+data.capacity(),_=>0,
                };
                if let Some(content)=b.state.inert_documents.template_contents.get(&handle){pending.push(*content);}
            }
            for source in &b.state.inert_documents.generated[before.2..] {
                payload+=node_bytes(&source.node)+size_of::<Node>()+2*size_of::<usize>();
                if let NodeKind::Element{tag,href,..}=&source.node.kind {payload+=tag.capacity()+href.as_ref().map_or(0,String::capacity);}
            }
            assert_eq!(b.state.retained-before.0,payload+b.template_import_backing()-before.1,"template={template}");
        }
    }
    #[test]
    fn range_clone_contents_work_copy_storage_and_cancellation_refuse_atomically(){
        for mode in 0..4 {
            let(mut d,mut f,mut p)=fixture();p.begin_job();let record=p.state.ranges[0];
            let before=(p.state.retained,p.state.inert_documents.entries.len(),p.state.inert_documents.generated.len(),d.nodes.len(),p.state.dom_commits);
            let limit=p.state.retained_limit;
            match mode {0=>p.state.host_work=0,1=>p.state.copy_work=1,2=>p.state.retained_limit=p.state.retained+32,_=>{p.state.host_work=4096;p.state.execution_deadline=Some(Instant::now());}}
            let error=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.clone_range_contents(record).unwrap_err();
            assert!(!error.is_empty());
            assert_eq!((p.state.retained,p.state.inert_documents.entries.len(),p.state.inert_documents.generated.len(),d.nodes.len(),p.state.dom_commits),before,"{error}");
            assert_eq!((p.state.ranges[0].start,p.state.ranges[0].end),(record.start,record.end));
            p.state.retained_limit=limit;p.begin_job();
            p.runtime.execute("var copy=r.cloneContents();if(copy.textContent!=='😀BMiddleEn'||root.textContent!=='A😀BMiddleEnd')throw 'fresh atomic recovery';",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();
        }
    }
}
#[derive(Clone,Copy)]
struct Pending {
    source: u32,
    owner: u32,
    parent: Option<usize>,
    template: Option<usize>,
    select: bool,
}
impl Bridge<'_> {
    pub(in crate::scripting) fn clone_range_contents(&mut self, range: Record) -> Result<Value,String> {
        let mut plan=Vec::new();
        plan.try_reserve_exact(1).map_err(|_|"Cannot reserve Range clone root.")?;
        plan.push(CopyItem{source:None,owner:DOCUMENT,parent:None,template:None,units:None,children:Vec::new()});
        let mut pending=Vec::new();
        if range.start!=range.end {
            let mut common=range.start.node;
            while common!=range.end.node && self.range_ancestor_child(common,range.end.node)?.is_none() {
                common=self.range_parent(common)?.ok_or("Range clone has no common ancestor.")?;
            }
            let index=self.range_node(common)?;
            if self.doc.nodes[index].kind.character_dom_string().is_some() {
                pending.push(Pending{source:common,owner:DOCUMENT,parent:Some(0),template:None,select:true});
            } else {
                let children=&self.doc.nodes[index].children;
                self.state.work(children.len())?;
                pending.try_reserve_exact(children.len()).map_err(|_|"Cannot reserve Range clone roots.")?;
                for &child in children.iter().rev() {
                    pending.push(Pending{source:node_handle(child),owner:DOCUMENT,parent:Some(0),template:None,select:true});
                }
            }
        }
        while let Some(task)=pending.pop() {
            self.state.work(1)?;
            let source=task.source;
            let mut full=!task.select;
            if task.select && source!=range.start.node && source!=range.end.node {
                let (parent,offset)=self.range_index(source)?;
                let before=Point{node:parent,offset};
                let after=Point{node:parent,offset:offset+1};
                if self.range_compare(range.start,after)?!=Some(Ordering::Less)
                    || self.range_compare(before,range.end)?!=Some(Ordering::Less) {continue;}
                full=self.range_compare(range.start,before)?!=Some(Ordering::Greater)
                    && self.range_compare(after,range.end)?!=Some(Ordering::Greater);
            }
            if plan.len()>=self.inert_remaining_entries() {return Err("Range clone identity allowance exhausted.".into());}
            let position=plan.len();
            let units=if task.select && !full {
                let index=self.range_node(source)?;
                self.doc.nodes[index].kind.character_dom_string().map(|text|{
                    let start=if source==range.start.node {range.start.offset}else{0};
                    let end=if source==range.end.node {range.end.offset}else{text.utf16_len()};
                    (start,end-start)
                })
            } else {None};
            plan.try_reserve_exact(1).map_err(|_|"Cannot reserve Range clone nodes.")?;
            plan.push(CopyItem{source:Some(source),owner:task.owner,parent:task.parent,template:task.template,units,children:Vec::new()});
            if let Some(parent)=task.parent {
                plan[parent].children.try_reserve_exact(1).map_err(|_|"Cannot reserve Range clone edge.")?;
                plan[parent].children.push(position);
            }
            let template=self.template_receiver(source);
            if template {
                // Lazy source contents precede assignment of unpublished IDs.
                let Value::Host(content)=self.template_content(source)? else {unreachable!()};
                let owner=self.state.inert_documents.node(content).ok_or("Range template content disappeared.")?.owner;
                pending.try_reserve(1).map_err(|_|"Cannot reserve Range template contents.")?;
                pending.push(Pending{source:content,owner,parent:None,template:Some(position),select:false});
            } else {
                let inert_count=self.state.inert_documents.node(source).map_or(0,|node|node.children.len());
                self.state.work(inert_count)?;
                let children=if let Some(node)=self.state.inert_documents.node(source) {
                    node.children.as_slice()
                } else {
                    let index=self.range_node(source)?;
                    // Main source children are projected below, without an
                    // intermediate serialized or parsed representation.
                    let children=&self.doc.nodes[index].children;
                    self.state.work(children.len())?;
                    pending.try_reserve(children.len()).map_err(|_|"Cannot reserve Range child traversal.")?;
                    for &child in children.iter().rev() {
                        pending.push(Pending{source:node_handle(child),owner:task.owner,parent:Some(position),template:None,select:!full});
                    }
                    &[]
                };
                pending.try_reserve(children.len()).map_err(|_|"Cannot reserve inert Range child traversal.")?;
                for &child in children.iter().rev() {
                    pending.push(Pending{source:child,owner:task.owner,parent:Some(position),template:None,select:false});
                }
            }
            self.state.work(plan.len())?;
            let scratch=plan.capacity().checked_mul(size_of::<CopyItem>())
                .and_then(|bytes|pending.capacity().checked_mul(size_of::<Pending>()).and_then(|other|bytes.checked_add(other)))
                .and_then(|bytes|plan.iter().try_fold(bytes,|sum,item|sum.checked_add(item.children.capacity()*size_of::<usize>())))
                .ok_or("Range clone traversal storage overflow.")?;
            if scratch>self.state.retained_limit.saturating_sub(self.state.retained) {return Err("Range clone temporary storage allowance exhausted.".into());}
        }
        let first=FIRST_ENTRY.checked_add(u32::try_from(self.state.inert_documents.entries.len()).map_err(|_|"Range clone identity overflow.")?)
            .ok_or("Range clone identity overflow.")?;
        let handle=|index:usize|first.checked_add(u32::try_from(index).map_err(|_|"Range clone count overflow.")?).ok_or_else(||"Range clone handle overflow.".to_owned());
        let source_base=self.state.inert_documents.generated.len();
        let mut entries=Vec::new();let mut sources=Vec::new();let mut links=Vec::new();let mut payload=0usize;
        entries.try_reserve_exact(plan.len()).map_err(|_|"Cannot reserve Range clone entries.")?;
        for item in &plan {
            self.state.work(item.children.len()+1)?;
            let parent=item.parent.map(&handle).transpose()?;
            let template=item.template.map(&handle).transpose()?;
            let mut children=Vec::new();children.try_reserve_exact(item.children.len()).map_err(|_|"Cannot reserve Range clone children.")?;
            for &child in &item.children {children.push(handle(child)?);}
            self.state.copies(children.capacity()*size_of::<u32>())?;
            let kind=if let Some(source)=item.source {
                let copy_bytes=self.state.inert_documents.node(source).map(|node|match &node.kind {
                    Kind::Element(name)=>name.len(),Kind::Text(data)=>data.used_bytes(),
                    Kind::ProcessingInstruction{target,data}=>target.len()+data.used_bytes(),_=>0,
                });
                if let Some(copy_bytes)=copy_bytes {
                    self.state.copies(copy_bytes)?;
                    let node=self.state.inert_documents.node(source).unwrap();
                    match &node.kind {
                        Kind::Fragment{..}=>Kind::Fragment{template},
                        Kind::Parsed{source,..}=>Kind::Parsed{source:*source,initial_template:None},
                        Kind::Element(name)=>Kind::Element(script_clone_node::copy_text(name)?),
                        Kind::Text(data)=>Kind::Text(data.try_clone()?),
                        Kind::ProcessingInstruction{target,data}=>{
                            Kind::ProcessingInstruction{target:script_clone_node::copy_text(target)?,data:data.try_clone()?}
                        },
                        _=>return Err("Range clone encountered an unsupported inert node kind.".into()),
                    }
                } else {
                    let index=self.range_node(source)?;
                    self.validate_svg_node(index)?;
                    let original=&self.doc.nodes[index];
                    if self.forms.controls.contains_key(&index) {return Err("Range clone native control state requires its checked copy producer.".into());}
                    if let NodeKind::Element{tag,attrs,..}=&original.kind {
                        if original.namespace==crate::html::Namespace::Html
                            && !(script_clone_node::ordinary_clone_element(tag)||tag=="template"||matches!(tag.as_str(),"area"|"legend"|"param"|"datalist"|"optgroup")) {
                            return Err("Range clone element lifecycle is not implemented.".into());
                        }
                        if attrs.iter().any(|attr|attr.name.namespace.is_empty()
                            && (script_element_properties::event_handler_content_attribute(tag,&attr.name)||attr.name.html_is("is"))) {
                            return Err("Range clone cannot transfer active handlers or customized built-ins.".into());
                        }
                    }
                    let kind=if let Some((offset,count))=item.units {
                        let text=original.kind.character_dom_string().ok_or("Range partial node is not CharacterData.")?;
                        let bytes=count.checked_mul(size_of::<u16>()).ok_or("Range clone unit count overflow.")?;
                        let work=text.used_bytes().checked_mul(3).and_then(|n|n.checked_add(count.checked_mul(3)?)).ok_or("Range clone unit work overflow.")?;
                        self.state.work(work)?;self.state.copies(bytes)?;
                        let mut units=Vec::new();units.try_reserve_exact(count).map_err(|_|"Cannot reserve Range clone units.")?;
                        units.extend(text.code_units().skip(offset).take(count));
                        let (projection,raw)=DomString::projected_size(&units)?;
                        let storage=projection.checked_add(if raw {bytes}else{0}).ok_or("Range clone string storage overflow.")?;
                        self.state.copies(storage)?;
                        let data=DomString::from_units(&units,self.state.retained_limit.saturating_sub(self.state.retained))?;
                        match &original.kind {
                            NodeKind::Text(_)=>NodeKind::Text(data),NodeKind::Cdata(_)=>NodeKind::Cdata(data),
                            NodeKind::Comment(_)=>NodeKind::Comment(data),
                            NodeKind::ProcessingInstruction{target,..}=>{
                                self.state.copies(target.len())?;
                                NodeKind::ProcessingInstruction{target:script_clone_node::copy_text(target)?,data}
                            },
                            _=>return Err("Range CharacterData kind changed during copying.".into()),
                        }
                    } else {
                        self.state.copies(copied_node_bytes(original))?;
                        script_clone_node::copy_node_kind(&original.kind)?
                    };
                    if matches!(kind,NodeKind::DocumentFragment) {Kind::Fragment{template}}
                    else {
                        let source_index=source_base.checked_add(sources.len()).ok_or("Range clone source overflow.")?;
                        let node=Node{kind,namespace:original.namespace,children:Vec::new()};
                        let mut size=node_bytes(&node).checked_add(size_of::<Node>()+2*size_of::<usize>()).ok_or("Range clone payload overflow.")?;
                        if let NodeKind::Element{tag,href,..}=&node.kind {size=size.checked_add(tag.capacity()).and_then(|n|n.checked_add(href.as_ref().map_or(0,String::capacity))).ok_or("Range element payload overflow.")?;}
                        payload=payload.checked_add(size).ok_or("Range clone payload overflow.")?;
                        sources.try_reserve_exact(1).map_err(|_|"Cannot reserve Range clone source snapshots.")?;
                        sources.push(GeneratedNode{node:Rc::new(node),unsafe_nonce:self.doc.nonce_unsafe_nodes.contains(&index)});
                        Kind::Parsed{source:ParsedSource::Generated(source_index),initial_template:None}
                    }
                }
            } else {Kind::Fragment{template:None}};
            let owned=match &kind {
                Kind::Element(name)=>name.capacity(),Kind::Text(data)=>data.capacity(),
                Kind::ProcessingInstruction{target,data}=>target.capacity()+data.capacity(),_=>0,
            };
            payload=payload.checked_add(owned).and_then(|n|n.checked_add(children.capacity()*size_of::<u32>())).ok_or("Range clone owned storage overflow.")?;
            if payload>self.state.retained_limit.saturating_sub(self.state.retained) {return Err("Range clone retained payload allowance exhausted.".into());}
            if let Some(host)=item.template {
                links.try_reserve_exact(1).map_err(|_|"Cannot reserve Range template links.")?;
                links.push((handle(host)?,handle(entries.len())?));
            }
            entries.push(Entry::Node(InertNode{kind,owner:item.owner,parent,children}));
        }
        self.state.work(plan.len())?;
        let temporary=plan.capacity().checked_mul(size_of::<CopyItem>())
            .and_then(|n|n.checked_add(pending.capacity()*size_of::<Pending>()))
            .and_then(|n|plan.iter().try_fold(n,|sum,item|sum.checked_add(item.children.capacity()*size_of::<usize>())))
            .and_then(|n|n.checked_add(entries.capacity()*size_of::<Entry>()))
            .and_then(|n|n.checked_add(sources.capacity()*size_of::<GeneratedNode>()))
            .and_then(|n|n.checked_add(links.capacity()*size_of::<(u32,u32)>()))
            .and_then(|n|n.checked_add(payload)).ok_or("Range clone staging peak overflow.")?;
        if temporary>self.state.retained_limit.saturating_sub(self.state.retained) {return Err("Range clone staging peak allowance exhausted.".into());}
        self.publish_inert_batch(first,entries,sources,payload,links,true)
    }
}
