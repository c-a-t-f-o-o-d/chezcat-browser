//! A checked HTML fragment slice. Replacement stages every node/resource plan
//! before publishing children; old arena nodes and their native identities live
//! until document teardown. Unsupported contexts never commit a parsed prefix.
use super::*;

#[path = "script_markup_output.rs"]
mod output;
pub(super) use output::Markup;

pub(super) fn raw_text(name: &str, scripting: bool) -> bool {
    matches!(name, "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext") || name == "noscript" && scripting
}

pub(super) fn serializes_as_void(name: &str) -> bool {
    crate::html::is_void(name) || matches!(name, "basefont" | "bgsound" | "frame" | "keygen")
}

impl Bridge<'_> {
    // https://html.spec.whatwg.org/multipage/dynamic-markup-insertion.html#dom-element-outerhtml
    pub(super) fn serialize_html(&mut self, node: usize, include_element: bool) -> Result<Value, String> {
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("HTML serialization requires known HTML namespace provenance.".into()); }
        if tag(self.doc, node) == "template" { return Err("Template content serialization is not implemented.".into()); }
        if !include_element && serializes_as_void(tag(self.doc, node)) { return Ok(Value::String(String::new())); }
        let mut stack: Vec<_> = if include_element {
            vec![(node, false, false)]
        } else {
            let children = &self.doc.nodes[node].children;
            self.state.work(children.len())?;
            let raw = raw_text(tag(self.doc, node), self.doc.scripting_enabled);
            children.iter().rev().map(|&child| (child, false, raw)).collect()
        };
        let mut out = Markup::new();
        while let Some((node, end, raw)) = stack.pop() {
            self.state.work(1)?;
            let item = &self.doc.nodes[node];
            match &item.kind {
                NodeKind::DocumentType(data)=>{out.wrapped(self.state, "<!DOCTYPE ", &data.name, ">")?;},
                NodeKind::DocumentFragment => {
                    self.state.work(item.children.len())?;
                    stack.extend(item.children.iter().rev().map(|&child| (child, false, raw)));
                }
                NodeKind::Text(text) | NodeKind::Cdata(text) => out.escaped(self.state, text.view(), false, raw)?,
                NodeKind::Comment(data) => {
                    out.wrapped(self.state, "<!--", data, "-->")?;
                }
                NodeKind::ProcessingInstruction { target, data } => out.processing_instruction(self.state, target, data)?,
                NodeKind::Element { tag, attrs, expanded_name, .. } => {
                    if item.namespace != crate::html::Namespace::Html || tag == "template" { return Err("HTML serialization of foreign/template content is not implemented.".into()); }
                    let local = crate::html::element_names::local(item.namespace, tag, expanded_name)?;
                    if end { out.close_tag(self.state, local)?; continue; }
                    out.open_tag(self.state, local, attrs)?;
                    if !serializes_as_void(tag) {
                        self.state.work(item.children.len())?;
                        stack.push((node, true, false));
                        let raw = raw_text(tag, self.doc.scripting_enabled);
                        stack.extend(item.children.iter().rev().map(|&child| (child, false, raw)));
                    }
                }
            }
        }
        out.finish(self.state)
    }

    pub(super) fn set_inner_html(&mut self, node: usize, source: &str) -> Result<(), String> {
        self.state.rebuild(self.doc)?;
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("innerHTML replacement requires known HTML namespace provenance.".into()); }
        if matches!(tag(self.doc, node), "script" | "style") { return Err("innerHTML replacement of script/style resource contexts is not implemented.".into()); }
        let passive_noscript = self.doc.scripting_enabled && self.movable_noscript_host(node)?;
        if passive_noscript {
            if let Some(parent) = self.state.tree.parents[node] { self.mutation_ancestors(parent)?; }
        } else {
            self.mutation_ancestors(node)?;
        }
        self.state.work(source.len())?;
        self.state.copies(source.len().saturating_mul(2))?;
        if source.len() > self.state.limits.host.string_bytes { return Err("HTML fragment source exceeds the host string limit.".into()); }
        let mut staged = crate::html::parse_fragment(source, tag(self.doc, node), self.doc.scripting_enabled)?;
        let count = staged.nodes.len() - 1;
        if self.doc.nodes.len().checked_add(count).and_then(|n| n.checked_add(self.state.blank_frame_nodes)).is_none_or(|n| n > self.state.node_limit) { return Err("DOM appended-node limit reached during HTML replacement.".into()); }
        let base = self.doc.nodes.len();
        let connected = self.state.dom.connected[node];
        let mut old = Vec::new();
        for child in self.doc.nodes[node].children.clone() { old.extend(self.mutation_subtree_for_fragment(child, true)?.0); }
        for &old in &old {
            if tag(self.doc, old) == "iframe" || tag(self.doc, old) == "link" && !self.removable_fragment_link(old)? {
                return Err("HTML replacement of stylesheet/frame resource contexts is not implemented.".into());
            }
        }
        let mut bytes = 0usize;
        let mut scripts = Vec::new();
        let mut images = Vec::new();
        let mut parents = vec![None; count];
        let mut depth = vec![0usize; staged.nodes.len()];
        let mut parent_depth = 0usize;
        let mut ancestor = Some(node);
        while let Some(parent) = ancestor { self.state.work(1)?; parent_depth += 1; ancestor = self.state.tree.parents[parent]; }
        for local in 0..staged.nodes.len() {
            self.state.work(1 + staged.nodes[local].children.len())?;
            for &child in &staged.nodes[local].children {
                depth[child] = depth[local] + 1;
                if depth[child].saturating_add(parent_depth) > 128 { return Err("DOM tree depth limit reached during HTML replacement.".into()); }
                parents[child - 1] = Some(if local == 0 { node } else { base + local - 1 });
            }
            if local == 0 { continue; }
            let item = &staged.nodes[local];
            bytes = bytes.checked_add(node_bytes(item)).ok_or("HTML fragment retained size overflow.")?;
            bytes = bytes.checked_add(child_link_bytes(&item.children)).ok_or("HTML fragment child storage overflow.")?;
            self.state.copies(item.children.len().checked_mul(size_of::<usize>()).ok_or("HTML fragment child copy work overflow.")?)?;
            if let NodeKind::Element { tag: name, attrs, .. } = &item.kind {
                let href = match &item.kind { NodeKind::Element {href, ..} => href.as_ref().map_or(0, String::len), _ => 0 };
                bytes = bytes.checked_add(name.len()).and_then(|bytes| bytes.checked_add(href)).ok_or("HTML fragment retained size overflow.")?;
                if item.namespace != crate::html::Namespace::Html
                    || forms::kind(&staged, local).is_some() && name != "select" && !forms::fragment_text_input(&staged, local) && !forms::fragment_checkbox_input(&staged, local) && forms::textarea_content(&staged, local).is_none()
                    || name == "link" && !script_stylesheets::bare_fragment_link(&staged, local)
                    || matches!(name.as_str(), "form" | "fieldset" | "legend" | "style" | "base" | "meta" | "title" | "iframe" | "object" | "embed" | "source" | "audio" | "video") {
                    return Err("HTML fragment replacement of native controls/forms or unsupported resources is not implemented.".into());
                }
                self.state.work(attrs.len())?;
                if let Some(text) = forms::textarea_content(&staged, local) {
                    // Native admission collects and normalizes the RCDATA default
                    // before copying it into the editor. Charge temporary work
                    // here; the shared commit charges the retained control below.
                    self.state.work(text.len().saturating_mul(3))?;
                    self.state.copies(text.len().saturating_mul(2))?;
                }
                if attrs.iter().any(|attr| script_element_properties::event_handler_content_attribute(name,&attr.name)) { return Err("HTML fragment inline event-handler registration is not implemented.".into()); }
                if let Some(style) = staged.attr(local, "style") { validate_style(style, self.state.limits.host.string_bytes)?; }
                if let Some(id) = staged.attr(local, "id").filter(|id| !id.is_empty()) {
                    self.state.work(self.forms.controls.len())?;
                    if self.forms.controls.keys().any(|&control| self.doc.attr(control, "form") == Some(id)) { return Err("HTML fragment id changes an explicit native form association.".into()); }
                }
                if name == "script" { scripts.push(base + local - 1); }
                if name == "img" {
                    if staged.attr(local, "srcset").is_some() { return Err("Dynamic responsive image selection is not implemented.".into()); }
                    if connected { images.push((base + local - 1, staged.attr(local, "src").map(str::to_owned))); }
                }
            }
        }
        self.state.copies(bytes)?;
        self.reserve_image_changes(images.len(), images.iter().filter(|(_, source)| source.is_some()).count())?;
        let mut image_plans = Vec::new();
        image_plans.try_reserve(images.len()).map_err(|_| "HTML image-plan reservation failed.")?;
        for (image, source) in images { image_plans.push((image, self.prepare_image_change(image, source.as_deref())?)); }
        self.doc.nonce_unsafe_nodes.try_reserve(staged.nonce_unsafe_nodes.len()).map_err(|_| "HTML nonce metadata reservation failed.")?;
        let mut children = Vec::new();
        self.state.copies(staged.nodes[0].children.len().checked_mul(size_of::<usize>()).ok_or("HTML root child copy work overflow.")?)?;
        children.try_reserve_exact(staged.nodes[0].children.len()).map_err(|_| "HTML children reservation failed.")?;
        children.extend(staged.nodes[0].children.iter().map(|&child| base + child - 1));
        let mut form_roots = Vec::new();
        form_roots.try_reserve(self.doc.nodes[node].children.len().saturating_add(children.len())).map_err(|_| "Select fragment roots reservation failed.")?;
        form_roots.extend(self.doc.nodes[node].children.iter().map(|&root| (root, None)));
        form_roots.extend(children.iter().map(|&root| (root, Some(node))));
        let form_plan = self.prepare_select_form_change(Some((&staged, base)), &form_roots)?;
        self.state.copies(form_plan.retained_bytes)?;
        let bytes = bytes.checked_add(form_plan.retained_bytes).and_then(|bytes|bytes.checked_add(child_link_bytes(&children)))
            .ok_or("HTML/native select payload overflow.")?;
        // Detached old nodes and their own buffers stay retained; only this
        // replaced root vector is dropped. Child record copies were prepaid.
        let released = child_link_bytes(&self.doc.nodes[node].children);
        let available = self.state.retained.checked_sub(released)
            .and_then(|retained|self.state.retained_limit.checked_sub(retained))
            .and_then(|available|available.checked_sub(bytes)).ok_or("HTML node/payload backing allowance exhausted.")?;
        let (node_growth, node_bytes) = self.prepare_main_nodes(count, available)?;
        self.state.prepare_range_changes(1,old.len())?;
        self.state.replace_storage(released, bytes.checked_add(node_bytes).ok_or("HTML node/payload storage overflow.")?, bytes)?;
        self.state.ranges_remove(node,0,self.doc.nodes[node].children.len(),&old);
        self.commit_main_nodes(node_growth);
        // No fallible operation follows publication of the new child list.
        for &root in &self.doc.nodes[node].children { self.state.tree.parents[root] = None; }
        for old in old { self.state.detached_nodes.insert(old); self.state.dom.connected[old] = false; self.state.tree.active[old] = false; }
        self.doc.nodes[node].children = children;
        for (offset, mut item) in staged.nodes.drain(1..).enumerate() {
            let id = base + offset;
            for child in &mut item.children { *child += base - 1; }
            self.doc.nodes.push(item);
            self.state.dom.created.insert(id);
            self.state.dom.connected.push(connected);
            self.state.tree.parents.push(parents[offset]);
            self.state.tree.active.push(false);
            if !connected { self.state.detached_nodes.insert(id); }
        }
        self.state.dom.fragment_scripts.extend(scripts);
        self.doc.nonce_unsafe_nodes.extend(staged.nonce_unsafe_nodes.into_iter().map(|local| base + local - 1));
        form_plan.commit(self.forms);
        self.commit_image_connections(image_plans);
        self.state.dom_committed();
        self.state.dirty = true;
        if connected { self.state.layout_changed(); }
        Ok(())
    }
}

#[cfg(test)]
#[path = "script_inner_html_tests.rs"]
mod tests;

#[cfg(test)]
mod noscript_fragment_context_tests {
 use super::*;
 fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document_with_scripting("<body><div id=root><b id=old>Old</b></div><p id=out>Real</p>",true,&||Ok(())).unwrap();let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://fragment-noscript.test/",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
 fn run(d:&mut Document,f:&mut Forms,p:&mut Page,source:&str){p.runtime.execute(source,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
 #[test]
 fn noscript_fragment_context_native_markup_observation_raw_reads_and_serialization(){
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var root=document.getElementById('root'),old=root.firstChild,watch=new MutationObserver(function(){});watch.observe(root,{childList:true});var raw='<img id=fake src=missing><script>window.bad=1;</script><style>p{color:red}</style>&amp;';root.innerHTML='<noscript id=fallback>'+raw+'</noscript><p id=after>After</p>';var fallback=document.getElementById('fallback'),records=watch.takeRecords();if(!fallback||fallback.firstChild.nodeType!==3||fallback.firstChild.data!==raw||fallback.textContent!==raw||fallback.innerHTML!==raw||fallback.outerHTML!=='<noscript id="fallback">'+raw+'</noscript>'||document.querySelectorAll('noscript').length!==1||document.getElementById('fake')!==null||document.querySelectorAll('script').length||old.parentNode!==null||records.length!==1||records[0].target!==root||records[0].removedNodes[0]!==old||records[0].addedNodes.length!==2||records[0].addedNodes[0]!==fallback||records[0].addedNodes[1]!==document.getElementById('after')||window.bad!==undefined)throw 'native fragment data and real records';"#);
  assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());assert!(!d.nodes.iter().any(|n|matches!(&n.kind,NodeKind::Element{tag,..}if tag=="img"||tag=="script"||tag=="style")));
 }
 #[test]
 fn noscript_fragment_context_native_resource_refusal_is_atomic_and_recovers(){
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var root=document.getElementById('root'),old=root.firstChild,markup='<noscript id=fallback><img id=fake src=missing>&amp;</noscript>',watch=new MutationObserver(function(){});watch.observe(root,{childList:true});"#);p.state.rebuild(&d).unwrap();let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let limit=p.state.retained_limit;
  for mode in 0..3{p.begin_job();match mode{0=>p.state.copy_work=0,1=>p.state.host_work=0,_=>p.state.retained_limit=p.state.retained};let error=p.runtime.execute("root.innerHTML=markup;",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();let category=error.to_ascii_lowercase();assert!(match mode{0=>category.contains("dom string work limit"),1=>category.contains("dom work limit"),_=>category.contains("storage")||category.contains("allowance")||category.contains("retained")},"{mode}: {error}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);p.state.retained_limit=limit;p.begin_job();run(&mut d,&mut f,&mut p,"if(root.firstChild!==old||watch.takeRecords().length||document.getElementById('fallback')!==null)throw 'atomic fragment refusal';");}
  p.begin_job();run(&mut d,&mut f,&mut p,"root.innerHTML=markup;var fallback=document.getElementById('fallback');if(old.parentNode!==null||fallback.firstChild.nodeType!==3||fallback.textContent!=='<img id=fake src=missing>&amp;'||document.getElementById('fake')!==null||watch.takeRecords().length!==1)throw 'fragment recovery';");assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
 }
}

#[cfg(test)]
#[path="script_noscript_inner_html_tests.rs"]
mod noscript_inner_html_tests;
