//! Small live DOM surfaces needed by ordinary page initialization. No requests
//! leave this module: image source changes and beacons are reported as declined.
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

pub(super) fn dataset_key(attribute: &str) -> Option<String> {
    let suffix = attribute.strip_prefix("data-")?;
    if suffix.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return None;
    }
    let mut key = String::with_capacity(suffix.len());
    let mut chars = suffix.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '-' && chars.peek().is_some_and(char::is_ascii_lowercase) {
            key.push(chars.next().unwrap().to_ascii_uppercase());
        } else {
            key.push(ch);
        }
    }
    Some(key)
}

pub(super) fn dataset_attribute(name: &str) -> String {
    let mut attribute = String::from("data-");
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            attribute.push('-');
            attribute.push(ch.to_ascii_lowercase());
        } else {
            attribute.push(ch);
        }
    }
    attribute
}

impl Bridge<'_> {
    pub(super) fn dataset_node(&mut self, handle: u32) -> Result<usize, String> {
        if !(DATASET_BASE..EVENT_BASE).contains(&handle) {
            return Err("Invalid dataset handle.".into());
        }
        self.node(node_handle((handle - DATASET_BASE) as usize))
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
        let attribute = dataset_attribute(name);
        self.state.copies(attribute.len())?;
        if let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind {
            self.state.work(attrs.len())?;
        }
        let value = match &self.doc.nodes[node].kind {
            NodeKind::Element { attrs, .. } => attrs
                .iter()
                .find(|(key, _)| *key == attribute)
                .map(|(_, value)| value),
            _ => None,
        };
        if let Some(value) = value {
            if value.len() > MAX_STRING {
                return Err("Dataset value exceeds 32 KiB.".into());
            }
            self.state.copies(value.len())?;
            Ok(Value::String(value.clone()))
        } else {
            Ok(Value::Undefined)
        }
    }

    pub(super) fn create_image(&mut self, args: &[Value]) -> Result<Value, String> {
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
                attrs.push(((*name).into(), dimension.to_string()));
            }
        }
        let bytes = 3 + attrs
            .iter()
            .map(|(name, value): &(String, String)| name.len() + value.len())
            .sum::<usize>();
        self.state.replace_bytes(0, bytes)?;
        let node = self.doc.nodes.len();
        self.doc.nodes.push(Node {
            kind: NodeKind::Element {
                tag: "img".into(),
                id: None,
                href: None,
                attrs,
            },
            children: Vec::new(),
        });
        self.state.detached_images.insert(node);
        self.state.dom.created.insert(node);
        // Detached elements have no tree parent and never enter document.images,
        // ID lookup, layout traversal or the document resource loader.
        self.state.tree.active.push(false);
        self.state.tree.parents.push(None);
        self.state.dom.connected.push(false);
        Ok(Value::Host(node_handle(node)))
    }

    pub(super) fn image_source(&mut self, node: usize) -> Result<Value, String> {
        let Some(source) = self.doc.attr(node, "src") else {
            return Ok(Value::String(String::new()));
        };
        self.state.copies(source.len())?;
        let source = source.to_owned();
        let resolved = self
            .state
            .location
            .as_ref()
            .and_then(|location| crate::url::parse(&location.href).ok())
            .and_then(|(base, _)| base.resolve(&source).ok())
            .map(|(url, fragment)| {
                let mut text = url.display();
                if !fragment.is_empty() {
                    text.push('#');
                    text.push_str(&fragment);
                }
                text
            })
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
            .and_then(|geometry| geometry.image_dimensions(node));
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
                dimensions.is_some() || self.doc.attr(node, "src").is_none_or(str::is_empty),
            ),
            "currentSrc" => {
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
        if name == "src" && !self.state.detached_images.contains(&node) {
            return Err("Changing a connected image source is not implemented; its decoded resource remains unchanged.".into());
        }
        let value = if matches!(name, "width" | "height") {
            (timer_integer(value)? as u32).to_string()
        } else {
            self.text(value)?
        };
        self.attribute(node, name, Some(value))?;
        if name == "src" {
            self.state.declined_images = self.state.declined_images.saturating_add(1);
        }
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
