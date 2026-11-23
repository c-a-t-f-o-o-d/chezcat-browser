//! Document-level HTML insertion rules. Fragments keep their context-specific parser.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    AfterBody,
    AfterHtml,
}

pub(super) enum Action {
    Normal,
    AtHead(usize),
    Consumed,
}

pub(super) struct Structure {
    mode: Mode,
    html: Option<usize>,
    head: Option<usize>,
    body: Option<usize>,
    html_attrs: Option<HashSet<String>>,
    body_attrs: Option<HashSet<String>>,
}

fn ascii_space(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

#[cfg(test)]
#[path = "html_document_structure_tests.rs"]
mod tests;

fn copy_name(name: &str) -> Result<String, String> {
    let mut copy = String::new();
    copy.try_reserve_exact(name.len())
        .map_err(|_| "Cannot reserve a structural attribute name.")?;
    copy.push_str(name);
    Ok(copy)
}

fn metadata(name: &str) -> bool {
    matches!(
        name,
        "base"
            | "basefont"
            | "bgsound"
            | "link"
            | "meta"
            | "noframes"
            | "script"
            | "style"
            | "template"
            | "title"
    )
}

impl Structure {
    pub(super) fn new() -> Self {
        Self {
            mode: Mode::BeforeHtml,
            html: None,
            head: None,
            body: None,
            html_attrs: None,
            body_attrs: None,
        }
    }

    fn insert(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        name: &str,
        explicit: Option<&mut Tag>,
        limit: usize,
    ) -> Result<usize, String> {
        let parent = if name == "html" {
            0
        } else {
            self.html.ok_or("Missing HTML document element.")?
        };
        if doc.nodes.len() >= limit {
            return Err(format!("HTML document node limit reached ({limit} nodes)."));
        }
        doc.nodes
            .try_reserve(1)
            .map_err(|_| "Cannot reserve HTML document structure.")?;
        doc.nodes[parent]
            .children
            .try_reserve(1)
            .map_err(|_| "Cannot reserve document structure edge.")?;
        stack
            .try_reserve(1)
            .map_err(|_| "Cannot reserve document element stack.")?;
        let (id, href, attrs) = if let Some(tag) = explicit {
            let mut attrs = Vec::new();
            attrs
                .try_reserve_exact(tag.attrs.len())
                .map_err(|_| "Cannot reserve structural element attributes.")?;
            attrs.extend(
                tag.attrs
                    .drain(..)
                    .map(|(k, v)| Attribute::unqualified(k, v)),
            );
            (tag.id.take(), tag.href.take(), attrs)
        } else {
            (None, None, Vec::new())
        };
        let node = doc.nodes.len();
        doc.nodes.push(Node {
            kind: NodeKind::Element {
                expanded_name: Vec::new(),
                tag: name.into(),
                id,
                href,
                attrs,
            },
            namespace: Namespace::Html,
            children: Vec::new(),
        });
        doc.nodes[parent].children.push(node);
        stack.truncate(if parent == 0 { 1 } else { 2 });
        stack.push(node);
        match name {
            "html" => {
                self.html = Some(node);
                self.mode = Mode::BeforeHead;
            }
            "head" => {
                self.head = Some(node);
                self.mode = Mode::InHead;
            }
            _ => {
                self.body = Some(node);
                self.mode = Mode::InBody;
            }
        }
        Ok(node)
    }

    fn close_head(&mut self, stack: &mut Vec<usize>) {
        stack.truncate(2);
        self.mode = Mode::AfterHead;
    }

    fn merge_attrs(
        doc: &mut Document,
        node: usize,
        token: &mut Tag,
        seen: &mut Option<HashSet<String>>,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<(), String> {
        if token.attrs.is_empty() {
            return Ok(());
        }
        let NodeKind::Element {
            id, href, attrs, ..
        } = &mut doc.nodes[node].kind
        else {
            return Err("Invalid HTML document structure.".into());
        };
        // The same html/body tag can recur throughout the input. Build a
        // name index on the first attributed duplicate and reuse it thereafter.
        if seen.is_none() {
            let mut keys = HashSet::new();
            keys.try_reserve(attrs.len())
                .map_err(|_| "Cannot reserve structural attribute index.")?;
            for (index, attr) in attrs.iter().enumerate() {
                if index % 1024 == 0 {
                    check()?;
                }
                keys.insert(copy_name(attr.name.qualified.as_str())?);
            }
            *seen = Some(keys);
        }
        let keys = seen.as_mut().unwrap();
        for (index, (key, value)) in token.attrs.drain(..).enumerate() {
            if index % 1024 == 0 {
                check()?;
            }
            if keys.contains(&key) {
                continue;
            }
            keys.try_reserve(1)
                .map_err(|_| "Cannot grow structural attribute index.")?;
            keys.insert(copy_name(&key)?);
            attrs
                .try_reserve(1)
                .map_err(|_| "Cannot reserve additional structural attributes.")?;
            if key == "id" {
                *id = token.id.take();
            } else if key == "href" {
                *href = token.href.take();
            }
            attrs.push(Attribute::unqualified(key, value));
        }
        Ok(())
    }

    fn nested_metadata(&self, stack: &[usize]) -> bool {
        self.mode == Mode::InHead && stack.last() != self.head.as_ref()
            || self.mode == Mode::AfterHead && stack.last() != self.html.as_ref()
    }

    pub(super) fn tag(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        token: &mut Tag,
        limit: usize,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Action, String> {
        let name = token.name.as_str();
        if matches!(name,"html"|"head"|"body"|"frameset") && stack.iter().skip(1).any(|&n|doc.nodes[n].namespace!=Namespace::Html ||
            matches!(&doc.nodes[n].kind,NodeKind::Element{tag,..} if matches!(tag.as_str(),"template"|"noscript"|"object"|"iframe"))){
            return Ok(Action::Consumed);
        }
        if self.nested_metadata(stack) {
            // A template or other open metadata subtree retains its own children.
            return Ok(if matches!(name, "html" | "head" | "body" | "frameset") {
                Action::Consumed
            } else {
                Action::Normal
            });
        }
        if self.mode != Mode::BeforeHtml && name == "html" && !token.closing {
            Self::merge_attrs(doc, self.html.unwrap(), token, &mut self.html_attrs, check)?;
            return Ok(Action::Consumed);
        }
        loop {
            match self.mode {
                Mode::BeforeHtml => {
                    if name == "html" && !token.closing {
                        self.insert(doc, stack, "html", Some(token), limit)?;
                        return Ok(Action::Consumed);
                    }
                    if token.closing && !matches!(name, "head" | "body" | "html" | "br") {
                        return Ok(Action::Consumed);
                    }
                    self.insert(doc, stack, "html", None, limit)?;
                }
                Mode::BeforeHead => {
                    if name == "head" && !token.closing {
                        self.insert(doc, stack, "head", Some(token), limit)?;
                        return Ok(Action::Consumed);
                    }
                    if token.closing && !matches!(name, "head" | "body" | "html" | "br") {
                        return Ok(Action::Consumed);
                    }
                    self.insert(doc, stack, "head", None, limit)?;
                }
                Mode::InHead => {
                    if !token.closing && (metadata(name) || name == "noscript") {
                        return Ok(Action::Normal);
                    }
                    if token.closing && name == "head" {
                        self.close_head(stack);
                        return Ok(Action::Consumed);
                    }
                    if name == "head" || token.closing && !matches!(name, "body" | "html" | "br") {
                        return Ok(Action::Consumed);
                    }
                    self.close_head(stack);
                }
                Mode::AfterHead => {
                    if !token.closing && name == "body" {
                        self.insert(doc, stack, "body", Some(token), limit)?;
                        return Ok(Action::Consumed);
                    }
                    if !token.closing && name == "frameset" {
                        self.insert(doc, stack, "frameset", Some(token), limit)?;
                        return Ok(Action::Consumed);
                    }
                    if !token.closing && metadata(name) {
                        return Ok(Action::AtHead(self.head.unwrap()));
                    }
                    if name == "head" || token.closing && !matches!(name, "body" | "html" | "br") {
                        return Ok(Action::Consumed);
                    }
                    self.insert(doc, stack, "body", None, limit)?;
                }
                Mode::AfterHtml | Mode::AfterBody => {
                    if self.mode == Mode::AfterBody && token.closing && name == "html" {
                        self.mode = Mode::AfterHtml;
                        return Ok(Action::Consumed);
                    }
                    self.mode = Mode::InBody;
                }
                Mode::InBody => {
                    if !matches!(name, "body" | "html" | "head") {
                        return Ok(Action::Normal);
                    }
                    if !token.closing && name == "body" {
                        if let Some(body)=self.body.filter(|&b|matches!(&doc.nodes[b].kind,NodeKind::Element{tag,..} if tag=="body")){Self::merge_attrs(doc,body,token,&mut self.body_attrs,check)?;}
                    } else if token.closing && matches!(name, "body" | "html") {
                        self.mode = if name == "html" {
                            Mode::AfterHtml
                        } else {
                            Mode::AfterBody
                        };
                    }
                    return Ok(Action::Consumed);
                }
            }
        }
    }

    pub(super) fn comment_parent(&self, current: usize) -> usize {
        match self.mode {
            Mode::AfterHtml => 0,
            Mode::AfterBody => self.html.unwrap(),
            _ => current,
        }
    }

    pub(super) fn text(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        mut text: String,
        limit: usize,
    ) -> Result<(), String> {
        let append = |doc: &mut Document, parent: usize, text: String| -> Result<(), String> {
            if append_text_value(doc, parent, text, limit)? {
                Ok(())
            } else {
                Err(format!("HTML document node limit reached ({limit} nodes)."))
            }
        };
        if self.nested_metadata(stack) {
            return append(doc, *stack.last().unwrap(), text);
        }
        if self.mode == Mode::InBody {
            return append(doc, *stack.last().unwrap(), text);
        }
        let leading = text.bytes().take_while(|&b| ascii_space(b)).count();
        if leading > 0 {
            let retain = !matches!(self.mode, Mode::BeforeHtml | Mode::BeforeHead);
            if leading == text.len() {
                return if retain {
                    append(doc, *stack.last().unwrap(), text)
                } else {
                    Ok(())
                };
            }
            if retain {
                let mut prefix = String::new();
                prefix
                    .try_reserve_exact(leading)
                    .map_err(|_| "Cannot reserve initial whitespace.")?;
                prefix.push_str(&text[..leading]);
                append(doc, *stack.last().unwrap(), prefix)?;
            }
            text.drain(..leading);
        }
        if text.is_empty() {
            return Ok(());
        }
        self.body_for_content(doc, stack, limit)?;
        append(doc, *stack.last().unwrap(), text)
    }

    fn body_for_content(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        limit: usize,
    ) -> Result<(), String> {
        if self.html.is_none() {
            self.insert(doc, stack, "html", None, limit)?;
        }
        if self.head.is_none() {
            self.insert(doc, stack, "head", None, limit)?;
        }
        if self.body.is_none() {
            self.close_head(stack);
            self.insert(doc, stack, "body", None, limit)?;
        }
        self.mode = Mode::InBody;
        Ok(())
    }
    pub(super) fn finish(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        limit: usize,
    ) -> Result<(), String> {
        if self.body.is_none() {
            self.body_for_content(doc, stack, limit)?;
        }
        Ok(())
    }
}
