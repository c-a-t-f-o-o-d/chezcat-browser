//! Small live DOM surfaces needed by ordinary page initialization. No requests
//! leave this module: image changes use the document resource queue. Fresh
//! detached Image requests and beacons retain their explicit declined boundary.
use super::*;

pub(super) fn navigator_readonly(name: &str) -> bool {
    matches!(
        name,
        "userAgent"
            | "appVersion"
            | "appCodeName"
            | "appName"
            | "product"
            | "vendor"
            | "platform"
            | "language"
            | "cookieEnabled"
            | "webdriver"
            | "pdfViewerEnabled"
            | "plugins"
            | "mimeTypes"
    )
}

pub(super) fn dataset_accessor(method:&str)->Option<bool>{match method{"\0HTMLElement.get.dataset"=>Some(false),"\0SVGElement.get.dataset"=>Some(true),_=>None}}

pub(super) fn dataset_suffix(attribute:&str)->Option<&str>{
    let suffix=attribute.strip_prefix("data-")?;
    (!suffix.bytes().any(|byte|byte.is_ascii_uppercase())).then_some(suffix)
}
pub(super) fn dataset_key_size(suffix:&str)->usize{
    suffix.len()-suffix.as_bytes().windows(2).filter(|pair|pair[0]==b'-'&&pair[1].is_ascii_lowercase()).count()
}
pub(super) fn dataset_key(attribute:&str,available:usize)->Result<Option<String>,String>{
    let Some(suffix)=dataset_suffix(attribute)else{return Ok(None)};
    let length=dataset_key_size(suffix);
    if length>available{return Err("DOM retained dataset key text allowance exhausted.".into());}
    let mut key=String::new();key.try_reserve_exact(length).map_err(|_|"Cannot reserve dataset key text.")?;
    if key.capacity()>available{return Err("DOM retained dataset key allocation exceeds allowance.".into());}
    let mut chars=suffix.chars().peekable();
    while let Some(ch)=chars.next(){
        if ch=='-'&&chars.peek().is_some_and(char::is_ascii_lowercase){key.push(chars.next().unwrap().to_ascii_uppercase());}else{key.push(ch);}
    }
    debug_assert_eq!(key.len(),length);
    Ok(Some(key))
}

pub(super) fn dataset_attribute(name: &str) -> Result<String,String> {
    let length=name.len().checked_add(name.bytes().filter(u8::is_ascii_uppercase).count()).and_then(|n|n.checked_add(5)).ok_or("Dataset attribute name size overflow.")?;
    let mut attribute = String::new();
    attribute.try_reserve_exact(length).map_err(|_|"Cannot reserve dataset attribute name.")?;
    attribute.push_str("data-");
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            attribute.push('-');
            attribute.push(ch.to_ascii_lowercase());
        } else {
            attribute.push(ch);
        }
    }
    Ok(attribute)
}

pub(super) fn url_copy_cost(
    raw: &str,
    base: Option<&crate::navigation::Location>,
) -> Result<usize, String> {
    let mut bytes = raw.len();
    if let Some(base) = base {
        for count in [
            base.fragment.len(),
            base.path.as_ref().map_or(0, |p| p.as_os_str().len()),
            base.url.as_ref().map_or(0, |u| u.host.len()),
            base.url.as_ref().map_or(0, |u| u.path.len()),
            base.url
                .as_ref()
                .and_then(|u| u.query.as_ref())
                .map_or(0, String::len),
        ] {
            bytes = bytes
                .checked_add(count)
                .ok_or("URL copy accounting overflow.")?;
        }
    }
    // Covers resolution temporaries, file percent encoding and both retained
    // URI copies; no allocation depends on a claimed resource size.
    bytes
        .checked_mul(20)
        .and_then(|n| n.checked_add(1024))
        .ok_or_else(|| "URL copy accounting overflow.".into())
}
impl Bridge<'_> {
    /// Genuine HTML string reflections, not a general expando/native alias.
    pub(super) fn html_text_reflection(&self,node:usize,name:&str)->Option<&'static str> {
        if !self.doc.nodes.get(node).is_some_and(|entry|entry.namespace==crate::html::Namespace::Html && matches!(entry.kind,NodeKind::Element{..})) {return None;}
        match name {
            "dir"=>Some("dir"),"lang"=>Some("lang"),
            "name" if tag(self.doc, node) == "slot" => Some("name"),
            "useMap" if matches!(tag(self.doc,node),"img"|"input"|"object")=>Some("usemap"),
            "align" if matches!(tag(self.doc,node),"div"|"p"|"h1"|"h2"|"h3"|"h4"|"h5"|"h6"|"hr"|"img"|"input"|"iframe"|"object"|"embed"|"table"|"caption"|"thead"|"tbody"|"tfoot"|"tr"|"td"|"th"|"col"|"colgroup")=>Some("align"),
            _=>None,
        }
    }
    pub(super) fn html_text_reflection_get(&mut self,node:usize,name:&str)->Result<Value,String> {
        if self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign {return Err("HTML reflection requires a known namespace context.".into());}
        let Some(attribute)=self.html_text_reflection(node,name) else {return Ok(Value::Undefined);};
        let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else {unreachable!()};self.state.work(attrs.len())?;
        let value=self.doc.attr(node,attribute).unwrap_or("");
        let value=if name=="dir" {["ltr","rtl","auto"].into_iter().find(|known|value.eq_ignore_ascii_case(known)).unwrap_or("")}else{value};
        if value.len()>self.state.limits.host.string_bytes {return Err("Reflected HTML value exceeds the DOM string allowance.".into());}
        self.state.copies(value.len())?;Ok(Value::String(value.to_owned()))
    }
    pub(super) fn html_image_map_property(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|item| item.namespace == crate::html::Namespace::Html)
            && tag(self.doc, node) == "img"
    }

    pub(super) fn html_image_map_get(&mut self, node: usize) -> Result<Value, String> {
        if self.doc.nodes[node].namespace == crate::html::Namespace::UnknownForeign {
            return Err("Image-map reflection requires a known namespace context.".into());
        }
        if !self.html_image_map_property(node) { return Ok(Value::Undefined); }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        self.state.work(attrs.len())?;
        Ok(Value::Bool(self.doc.attr(node, "ismap").is_some()))
    }

    pub(super) fn html_border_property(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|item| item.namespace == crate::html::Namespace::Html)
            && matches!(tag(self.doc, node), "img" | "object" | "table")
    }

    pub(super) fn html_border_get(&mut self, node: usize) -> Result<Value, String> {
        if self.doc.nodes[node].namespace == crate::html::Namespace::UnknownForeign {
            return Err("Border lookup requires a known namespace context.".into());
        }
        if !self.html_border_property(node) { return Ok(Value::Undefined); }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        self.state.work(attrs.len())?;
        let value = self.doc.attr(node, "border").unwrap_or("");
        if value.len() > self.state.limits.host.string_bytes { return Err("Border value exceeds the DOM string limit.".into()); }
        self.state.copies(value.len())?;
        Ok(Value::String(value.to_owned()))
    }

    pub(super) fn html_spacing_property(&self, node: usize, name: &str) -> bool {
        matches!(name, "vspace" | "hspace")
            && self.doc.nodes.get(node).is_some_and(|item| item.namespace == crate::html::Namespace::Html)
            && matches!(tag(self.doc, node), "img" | "input" | "object" | "embed" | "marquee")
    }

    pub(super) fn html_spacing_get(&mut self, node: usize, name: &str) -> Result<Value, String> {
        if self.doc.nodes[node].namespace == crate::html::Namespace::UnknownForeign {
            return Err("Spacing lookup requires a known namespace context.".into());
        }
        if !self.html_spacing_property(node, name) { return Ok(Value::Undefined); }
        if let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind {
            self.state.work(attrs.len())?;
        }
        let value = self.doc.attr(node, name).unwrap_or("");
        self.state.work(value.len().saturating_add(1))?;
        Ok(Value::Number(f64::from(crate::forms::parse_tab_index(value).filter(|n| *n >= 0).unwrap_or(0))))
    }

    pub(super) fn html_spacing_set(&mut self, node: usize, name: &str, value: &Value) -> Result<(), String> {
        let value = timer_integer(value)? as u32;
        let value = if value > i32::MAX as u32 { 0 } else { value };
        self.attribute(node, name, Some(value.to_string()))
    }

    pub(super) fn dataset_accessor_receiver(&self,object:u32,svg:bool)->bool{
        if svg{self.style_interface_receiver(object,true)||self.frame_node_interface(object)==Some("SVGElement")}
        else{self.html_reflection_receiver(object)}
    }
    pub(super) fn dataset_accessor_member(&self,object:u32,property:&str)->bool{
        property=="dataset"&&!self.form_named_control(object,property)
            &&(self.dataset_accessor_receiver(object,false)||self.dataset_accessor_receiver(object,true))
    }
    pub(super) fn validate_dataset_accessor(&self,object:u32,svg:bool)->Result<(),(&'static str,String)>{
        if !self.dataset_accessor_receiver(object,svg){return Err(("TypeError","Dataset accessor requires its genuine HTML or SVG interface receiver.".into()));}
        self.native_node_access(object)?;
        if self.is_frame_handle(object)||self.inert_node_type(object).is_some(){return Err(("NotSupportedError","Dataset backing on child or inert document elements is unavailable.".into()));}
        Ok(())
    }
    pub(super) fn dataset_accessor_get(&mut self,object:u32,svg:bool)->Result<Value,String>{
        self.validate_dataset_accessor(object,svg).map_err(|(_,message)|message)?;
        self.state.work(1)?;
        let node=self.node(object)?;
        Ok(Value::Host(DATASET_BASE+node as u32))
    }

    pub(super) fn dataset_node(&mut self, handle: u32) -> Result<usize, String> {
        if !(DATASET_BASE..EVENT_BASE).contains(&handle) {
            return Err("Invalid dataset handle.".into());
        }
        self.node(node_handle((handle - DATASET_BASE) as usize))
    }

    pub(super) fn dataset_keys(&mut self,handle:u32)->Result<Vec<String>,String>{
        let node=self.dataset_node(handle)?;
        let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{return Err("Dataset requires an Element.".into())};
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&attrs.len()>self.state.limits.host.element_attributes{return Err(format!("Dataset enumeration limit reached ({} attributes).",self.state.limits.host.element_attributes));}
        self.state.work(attrs.len())?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let(mut count,mut payload)=(0usize,0usize);
        for attr in attrs.iter().filter(|attr|attr.name.namespace.is_empty()){
            let Some(raw)=attr.name.qualified.strip_prefix("data-")else{continue};
            attr.name.qualified.scalar()?;
            self.state.work(raw.len().checked_mul(2).ok_or("Dataset key validation work overflow.")?)?;
            let Some(suffix)=dataset_suffix(&attr.name.qualified)else{continue};
            // Both validation walks are prepaid even for excluded names.
            // Eligible names also pay the length and final conversion walks.
            self.state.work(suffix.len().checked_mul(5).ok_or("Dataset key work overflow.")?)?;
            count=count.checked_add(1).ok_or("Dataset key count overflow.")?;
            payload=payload.checked_add(dataset_key_size(suffix)).ok_or("Dataset key payload overflow.")?;
            let minimum=count.checked_mul(size_of::<String>()).and_then(|bytes|bytes.checked_add(payload)).ok_or("Dataset key storage overflow.")?;
            if minimum>available{return Err("DOM retained dataset key allowance exhausted.".into());}
        }
        let work=count.checked_mul(size_of::<String>()).and_then(|bytes|bytes.checked_add(payload)).ok_or("Dataset key work overflow.")?;
        self.state.copies(work)?;
        let mut keys=Vec::new();keys.try_reserve_exact(count).map_err(|_|"Cannot reserve dataset key records.")?;
        let mut bytes=keys.capacity().checked_mul(size_of::<String>()).filter(|bytes|bytes.checked_add(payload).is_some_and(|total|total<=available)).ok_or("DOM retained dataset key record allowance exhausted.")?;
        let mut remaining=payload;
        for attr in attrs.iter().filter(|attr|attr.name.namespace.is_empty()){
            let Some(suffix)=dataset_suffix(&attr.name.qualified)else{continue};
            remaining-=dataset_key_size(suffix);
            let spare=available.checked_sub(bytes).and_then(|bytes|bytes.checked_sub(remaining)).ok_or("DOM retained dataset key allowance exhausted.")?;
            let key=dataset_key(&attr.name.qualified,spare)?.ok_or("Dataset key validation changed during enumeration.")?;
            bytes=bytes.checked_add(key.capacity()).ok_or("Dataset key allocation overflow.")?;
            keys.push(key);
        }
        Ok(keys)
    }

    pub(super) fn dataset_units_assignment(&mut self,handle:u32,key:&[u16])->Result<crate::javascript::NativeNamedAssignment,String>{
        use crate::javascript::NativeNamedAssignment;
        script_text_content::deadline(self.state)?;
        self.state.work(key.len().checked_mul(4).and_then(|n|n.checked_add(1)).ok_or("Dataset raw key work overflow.")?)?;
        let node=self.dataset_node(handle)?;
        self.native_node_access(node_handle(node)).map_err(|(_,message)|message)?;
        if !matches!(&self.doc.nodes[node].kind,NodeKind::Element{..}){return Err("Dataset requires an Element.".into());}
        if key.windows(2).any(|pair|pair[0]==45&&(97..=122).contains(&pair[1])){return Ok(NativeNamedAssignment::SyntaxError);}
        // The data- prefix makes the result nonempty. ASCII upper alpha expansion
        // introduces only valid name units, so validate the borrowed suffix.
        if !super::script_attribute_names::valid_attribute_name_dom(crate::dom_string::DomStringRef::Raw(key))&&!key.is_empty(){return Ok(NativeNamedAssignment::InvalidCharacterError);}
        let length=key.len().checked_add(key.iter().filter(|unit|(65..=90).contains(*unit)).count()).and_then(|n|n.checked_add(5)).ok_or("Dataset raw attribute size overflow.")?;
        let bytes=length.checked_mul(size_of::<u16>()).ok_or("Dataset raw attribute bytes overflow.")?;
        if bytes>self.state.limits.host.string_bytes{return Err("Dataset attribute exceeds the host string allowance.".into());}
        self.state.copies(bytes)?;
        let mut name=Vec::new();name.try_reserve_exact(length).map_err(|_|"Cannot reserve dataset raw attribute name.")?;
        name.extend_from_slice(&[100,97,116,97,45]);
        for &unit in key{if (65..=90).contains(&unit){name.push(45);name.push(unit+32);}else{name.push(unit);}}
        script_text_content::deadline(self.state)?;
        Ok(NativeNamedAssignment::Attribute{element:node_handle(node),name:Value::StringUnits(name)})
    }

    /// Borrow the original name; display projections cannot identify a key.
    pub(super) fn dataset_units_attribute(&mut self,handle:u32,key:&[u16])->Result<Option<(usize,usize)>,String>{
        script_text_content::deadline(self.state)?;
        self.state.work(1)?;
        let node=self.dataset_node(handle)?;
        self.native_node_access(node_handle(node)).map_err(|(_,message)|message)?;
        let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{return Err("Dataset requires an Element.".into())};
        self.state.work(attrs.len())?;
        for (index,attr) in attrs.iter().enumerate().filter(|(_,a)|a.name.namespace.is_empty()){
            script_text_content::deadline(self.state)?;
            let name=attr.name.qualified.view();
            self.state.work(name.logical_bytes().checked_mul(4).ok_or("Dataset raw name work overflow.")?)?;
            let mut prefix=name.code_units();
            if ![100,97,116,97,45].into_iter().all(|unit|prefix.next()==Some(unit)){continue;}
            if name.code_units().skip(5).any(|unit|(65..=90).contains(&unit)){continue;}
            let mut suffix=name.code_units().skip(5).peekable();
            let converted=std::iter::from_fn(||{
                let unit=suffix.next()?;
                if unit==45&&suffix.peek().is_some_and(|next|(97..=122).contains(next)){
                    Some(suffix.next().unwrap()-32)
                }else{Some(unit)}
            });
            if !converted.eq(key.iter().copied()){continue;}
            return Ok(Some((node,index)));
        }
        script_text_content::deadline(self.state)?;
        Ok(None)
    }

    pub(super) fn dataset_units_get(&mut self,handle:u32,key:&[u16])->Result<Value,String>{
        let Some((node,index))=self.dataset_units_attribute(handle,key)? else{return Ok(Value::Undefined)};
        let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{return Err("Dataset requires an Element.".into())};
        let value=&attrs[index].value;
        if value.logical_bytes()>self.state.limits.host.string_bytes{return Err("Dataset value exceeds 32 KiB.".into());}
        Self::dom_string_value(self.state,value)
    }

    pub(super) fn dataset_get(&mut self, handle: u32, name: &str) -> Result<Value, String> {
        let node = self.dataset_node(handle)?;
        // A literal hyphen followed by a lowercase ASCII letter cannot be a
        // supported DOMStringMap key. Do not alias it to the camel-case key.
        if name
            .as_bytes()
            .windows(2)
            .any(|pair| pair[0] == b'-' && pair[1].is_ascii_lowercase())
        {
            return Ok(Value::Undefined);
        }
        let attribute = dataset_attribute(name)?;
        self.state.copies(attribute.len())?;
        if let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind {
            self.state.work(attrs.len())?;
        }
        let value = match &self.doc.nodes[node].kind {
            NodeKind::Element { attrs, .. } => attrs
                .iter()
                .find(|attr| attr.name.html_is(&attribute))
                .map(|attr| &attr.value),
            _ => None,
        };
        if let Some(value) = value {
            if value.logical_bytes() > self.state.limits.host.string_bytes {
                return Err("Dataset value exceeds 32 KiB.".into());
            }
            Self::dom_string_value(self.state,value)
        } else {
            Ok(Value::Undefined)
        }
    }

    pub(super) fn create_image(&mut self, args: &[Value]) -> Result<Value, String> {
        self.create_image_named(args,Vec::new())
    }
    pub(super) fn create_image_named(&mut self,args:&[Value],expanded_name:Vec<crate::html::ExpandedElementName>)->Result<Value,String>{
        self.state.rebuild(self.doc)?;
        if self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes) >= self.state.node_limit {
            return Err("DOM appended-node limit reached (256 per document).".into());
        }
        let mut attrs = Vec::new();
        for (index, name) in ["width", "height"].iter().enumerate() {
            if let Some(value) = args.get(index) {
                // Web IDL unsigned long wraps after truncation; it is not a
                // pointer size or a raster allocation request.
                let dimension = timer_integer(value)? as u32;
                attrs.push(crate::html::Attribute::unqualified((*name).into(), dimension.to_string()));
            }
        }
        let bytes = 3 + crate::html::element_names::owned_bytes(&expanded_name) + attrs.capacity()*size_of::<crate::html::Attribute>() + attrs
            .iter()
            .map(crate::html::Attribute::owned_bytes)
            .sum::<usize>();
        let available = self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(bytes)
            .ok_or("Image node/payload backing allowance exhausted.")?;
        let (node_growth, node_bytes) = self.prepare_main_nodes(1, available)?;
        self.state.replace_storage(0, bytes.checked_add(node_bytes).ok_or("Image node/payload storage overflow.")?, bytes)?;
        self.commit_main_nodes(node_growth);
        let node = self.doc.nodes.len();
        self.doc.nodes.push(Node {
            namespace: crate::html::Namespace::Html,
            kind: NodeKind::Element {expanded_name,
                tag: "img".into(),
                id: None,
                href: None,
                attrs,
            },
            children: Vec::new(),
        });
        self.state.detached_images.insert(node);
        self.state.dom.created.insert(node);
        self.state.detached_nodes.insert(node);
        // Detached elements have no tree parent and never enter document.images,
        // ID lookup, layout traversal or the document resource loader.
        self.state.tree.active.push(false);
        self.state.tree.parents.push(None);
        self.state.dom.connected.push(false);
        Ok(Value::Host(node_handle(node)))
    }

    /// Reflect a URL content attribute without admitting or opening a resource.
    pub(super) fn reflected_url(&mut self,node:usize,attribute:&str)->Result<Value,String>{
        let Some(raw)=self.doc.attr(node,attribute) else{return self.string("");};
        if raw.len()>self.state.limits.host.string_bytes{return Err("Reflected URL exceeds the configured string allowance.".into());}
        let base=self.state.dynamic.base.as_ref().or_else(||self.state.location.as_ref().map(|location|&location.source));
        let cost=url_copy_cost(raw,base)?;
        self.state.copies(cost)?;
        let base=self.state.dynamic.base.as_ref().or_else(||self.state.location.as_ref().map(|location|&location.source));
        let resolved=base.and_then(|base|crate::navigation::selection_url(raw,base).ok()).unwrap_or_else(||raw.to_owned());
        self.string(&resolved)
    }

    pub(super) fn image_source(&mut self, node: usize) -> Result<Value, String> {
        let Some(source) = self.doc.attr(node, "src") else {
            return Ok(Value::String(String::new()));
        };
        self.state.copies(source.len())?;
        let source = source.to_owned();
        let resolved = self.state.dynamic.base.as_ref()
            .or_else(|| self.state.location.as_ref().map(|location| &location.source))
            .and_then(|base| crate::navigation::resolve(&source, base).ok())
            .map(|target| target.display())
            .unwrap_or(source);
        self.string(&resolved)
    }

    pub(super) fn image_get(&mut self, node: usize, name: &str) -> Result<Option<Value>, String> {
        if tag(self.doc, node) != "img" {
            return Ok(None);
        }
        let dimensions = self
            .state
            .geometry
            .as_ref()
            .and_then(|geometry| geometry.image_dimensions(node))
            .filter(|_| self.state.images.entries.get(&node).is_none_or(|entry| entry.available));
        Ok(Some(match name {
            "src" => return self.image_source(node).map(Some),
            "alt" => {
                let value = self.doc.attr(node, "alt").unwrap_or("").to_owned();
                return self.string(&value).map(Some);
            }
            "width" | "height" => Value::Number(
                self.doc
                    .attr(node, name)
                    .and_then(|text| text.trim().parse::<u32>().ok())
                    .map(f64::from)
                    .or_else(|| {
                        dimensions.map(|(w, h)| if name == "width" { w as f64 } else { h as f64 })
                    })
                    .unwrap_or(0.0),
            ),
            "naturalWidth" => Value::Number(dimensions.map_or(0, |size| size.0) as f64),
            "naturalHeight" => Value::Number(dimensions.map_or(0, |size| size.1) as f64),
            "complete" => Value::Bool(
                self.state.images.entries.get(&node).map_or_else(
                    || dimensions.is_some() || self.doc.attr(node, "src").is_none_or(str::is_empty),
                    |entry| !entry.pending || self.doc.attr(node, "src").is_none_or(str::is_empty)),
            ),
            "currentSrc" => {
                if let Some(entry) = self.state.images.entries.get(&node) {
                    let current = entry.current.clone();
                    return self.string(&current).map(Some);
                }
                if dimensions.is_some() {
                    return self.image_source(node).map(Some);
                } else {
                    Value::String(String::new())
                }
            }
            _ => return Ok(None),
        }))
    }

    pub(super) fn image_set(
        &mut self,
        node: usize,
        name: &str,
        value: &Value,
    ) -> Result<bool, String> {
        if tag(self.doc, node) != "img" || !matches!(name, "src" | "alt" | "width" | "height") {
            return Ok(false);
        }
        let value = if matches!(name, "width" | "height") {
            (timer_integer(value)? as u32).to_string()
        } else {
            self.text(value)?
        };
        if name == "src" {
            self.image_source_change(node, Some(value))?;
        } else { self.attribute(node, name, Some(value))?; }
        Ok(true)
    }

    pub(super) fn send_beacon(&mut self, args: &[Value]) -> Result<Value, String> {
        let value = args.first().ok_or("sendBeacon requires a URL.")?;
        let source = self.text(value)?;
        // Validation uses the same URL parser as navigation. This is inspection
        // only; no DNS, socket, cookie, credential or background queue exists.
        if !self.beacon_url_valid(&source) {
            return Err("sendBeacon requires a valid HTTP or HTTPS URL.".into());
        }
        if let Some(Value::String(body)) = args.get(1) {
            self.state.copies(body.len())?;
        }
        self.state.declined_beacons = self.state.declined_beacons.saturating_add(1);
        Ok(Value::Bool(false))
    }

    pub(super) fn beacon_url_valid(&self, source: &str) -> bool {
        self.state
            .location
            .as_ref()
            .and_then(|location| crate::url::parse(&location.href).ok())
            .map(|(base, _)| base.resolve(source).is_ok())
            .unwrap_or_else(|| crate::url::parse(source).is_ok())
    }
}

#[cfg(test)]
#[path="script_dataset_storage_tests.rs"]
mod dataset_storage_tests;
