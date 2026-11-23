//! A small HTML subset parser. It builds an arena of nodes without recursion.
//!
//! This is not a complete HTML tree builder: table recovery, general foreign content,
//! and the full named-character-reference table are not implemented.
//! Common Latin-1 and punctuation references use a hand-selected name table.
//! Numeric references, legacy name matching, and attribute ambiguity follow
//! https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state
//! An unfinished tag consumes the rest of the input. Loaded documents report
//! tree capacity exhaustion before commit. Small internal parses keep a prefix.

use std::borrow::Cow;
use std::collections::HashSet;
use crate::dom_string::DomString;

#[cfg(test)]
#[path = "html_foundation_adapter_tests.rs"]
mod foundation_adapter;

#[path = "html_svg_namespace.rs"]
mod svg_namespace;
#[path="html_doctype.rs"]
mod doctype;
#[path="html_document_mode.rs"]
mod document_mode;
#[path="html_element_names.rs"]
pub(crate) mod element_names;
pub(crate) use element_names::{ExpandedElementName,ElementNamespaceValue};

const MAX_NODES: usize = 20_000;
pub(crate) const MAX_LOADED_DOCUMENT_NODES: usize = 262_144;
const MAX_DEPTH: usize = 128; // Open-element stack, including the synthetic root.

/// A host-only node identity. Vec indices cannot reach usize::MAX, which
/// represents no target. Atomic storage preserves Document's Sync use.
#[derive(Debug)]
pub(crate) struct NativeNode(std::sync::atomic::AtomicUsize);
impl Default for NativeNode{fn default()->Self{Self(std::sync::atomic::AtomicUsize::new(usize::MAX))}}
impl NativeNode{
    pub(crate) fn get(&self)->Option<usize>{let node=self.0.load(std::sync::atomic::Ordering::Relaxed);(node!=usize::MAX).then_some(node)}
    pub(crate) fn set(&self,node:Option<usize>){self.0.store(node.unwrap_or(usize::MAX),std::sync::atomic::Ordering::Relaxed);}
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum DocumentKind {Html,Xml}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum DocumentMode {NoQuirks,LimitedQuirks,Quirks}
impl DocumentMode {
    pub(crate) fn compat_mode(self)->&'static str{if self==Self::Quirks{"BackCompat"}else{"CSS1Compat"}}
}
#[derive(Debug)]
pub(crate) struct DocumentTypeData {
    pub(crate) name:DomString,
    pub(crate) public_id:DomString,
    pub(crate) system_id:DomString,
}
impl DocumentTypeData {
    pub(crate) fn html()->Result<Self,String>{let mut name=String::new();name.try_reserve_exact(4).map_err(|_|"Cannot reserve HTML doctype name.")?;name.push_str("html");Ok(Self{name:name.into(),public_id:DomString::new(),system_id:DomString::new()})}
    pub(crate) fn capacity(&self)->usize{self.name.capacity()+self.public_id.capacity()+self.system_id.capacity()}
    pub(crate) fn used_bytes(&self)->usize{self.name.used_bytes()+self.public_id.used_bytes()+self.system_id.used_bytes()}
    pub(crate) fn try_clone(&self)->Result<Self,String>{Ok(Self{name:self.name.try_clone()?,public_id:self.public_id.try_clone()?,system_id:self.system_id.try_clone()?})}
}

#[derive(Debug)]
pub struct Document {
    pub(crate) kind:DocumentKind,
    pub(crate) mode:DocumentMode,
    /// Host-owned resource policy, never exposed as a document capability.
    pub stylesheet_limits: crate::css::StylesheetLimits,
    /// Host-only screen context; page script cannot supply viewport or input capabilities.
    pub media_environment: Option<crate::css::MediaEnvironment>,
    /// Host control focus; disconnected targets can be retired by native topology rebuilds.
    pub(crate) native_focus: NativeNode,
    pub(crate) native_hover: NativeNode,
    pub title: String,
    /// The committed resource's MIME essence. Synthetic text/image viewers
    /// retain the source type; plain parser callers create HTML documents.
    pub content_type: String,
    pub(crate) encoding: crate::text_codec::Encoding,
    pub nodes: Vec<Node>,
    /// Set at commit after resource policy is known. The current tokenizer still
    /// parses noscript markup; consumers hide/inactivate that subtree when true.
    /// Full HTML scripting-flag tokenization is not implemented yet.
    pub scripting_enabled: bool,
    /// CSP nonceability needs duplicate-attribute evidence even though normal
    /// HTML attribute lookup keeps only the first occurrence.
    pub nonce_unsafe_nodes: HashSet<usize>,
    /// Connected STYLE elements without an associated sheet after their type
    /// or content security policy was checked. Their text remains in the DOM.
    pub(crate) withheld_inline_styles: HashSet<usize>,
}

impl Document {
    /// HTML's body element is the first HTML body or frameset directly below
    /// the document's HTML root. Callers supply the known root element.
    pub(crate) fn body_element(&self, root: usize) -> Option<usize> {
        let element = self.nodes.get(root)?;
        if element.namespace != Namespace::Html
            || !matches!(&element.kind, NodeKind::Element { tag, .. } if tag == "html") {
            return None;
        }
        element.children.iter().copied().find(|&id| self.nodes.get(id).is_some_and(|child| {
            child.namespace == Namespace::Html
                && matches!(&child.kind, NodeKind::Element { tag, .. } if matches!(tag.as_str(), "body" | "frameset"))
        }))
    }

    pub fn attr(&self,node:usize,name:&str)->Option<&str>{self.dom_attr(node,name).map(DomString::render_projection)}

    pub(crate) fn dom_attr(&self, node: usize, name: &str) -> Option<&DomString> {
        // HTML consumers accept a case-insensitive query, but only the exact
        // lower-case null-namespace attribute drives native HTML behavior.
        let name = if name.bytes().any(|c| c.is_ascii_uppercase()) {
            Cow::Owned(name.to_ascii_lowercase())
        } else { Cow::Borrowed(name) };
        match &self.nodes.get(node)?.kind {
            NodeKind::Element { attrs, .. } => attrs
                .iter()
                .find(|attr| attr.name.html_is(name.as_ref()))
                .map(|attr| &attr.value),
            NodeKind::Text(_) | NodeKind::Cdata(_) | NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. }
            | NodeKind::DocumentFragment | NodeKind::DocumentType(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<usize>,
    /// Persistent provenance from document parsing or a supported DOM creation path.
    pub namespace: Namespace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace {
    None,
    Html,
    Svg,
    UnknownForeign,
    // Known non-HTML/SVG Element URI (including null), owned in expanded_name.
    Other,
}

impl Namespace {
    pub(crate) fn element_uri(self) -> Option<&'static str> {
        match self {
            Self::Html => Some("http://www.w3.org/1999/xhtml"),
            Self::Svg => Some("http://www.w3.org/2000/svg"),
            Self::None | Self::UnknownForeign | Self::Other => None,
        }
    }

    pub(crate) fn is_foreign(self) -> bool {
        matches!(self, Self::Svg | Self::UnknownForeign | Self::Other)
    }
}

#[derive(Debug)]
pub struct AttributeName {
    pub qualified: DomString,
    /// Zero or one owned record; null-namespace names allocate no metadata.
    pub namespace: Vec<AttributeNamespace>,
}

#[derive(Debug)]
pub struct AttributeNamespace {
    pub uri: DomString,
    pub prefix: Option<DomString>,
    pub local: DomString,
}

impl AttributeName {
    pub(crate) fn namespace_storage(namespace:Option<AttributeNamespace>)->Result<Vec<AttributeNamespace>,String>{let mut records=Vec::new();if let Some(namespace)=namespace{records.try_reserve_exact(1).map_err(|_|"Cannot reserve attribute namespace record.")?;records.push(namespace);}Ok(records)}
    pub fn unqualified(qualified: String) -> Self { Self { qualified:qualified.into(), namespace: Vec::new() } }
    pub(crate) fn unqualified_dom(qualified:DomString)->Self{Self{qualified,namespace:Vec::new()}}
    pub fn namespace_uri(&self) -> Option<&DomString> { self.namespace.first().map(|ns| &ns.uri) }
    pub fn prefix(&self) -> Option<&DomString> { self.namespace.first().and_then(|ns| ns.prefix.as_ref()) }
    pub fn local(&self) -> &DomString { self.namespace.first().map_or(&self.qualified, |ns| &ns.local) }
    pub fn html_is(&self, name: &str) -> bool { self.namespace.is_empty() && self.qualified == name }
    pub fn matches(&self, namespace: Option<&str>, local: &str) -> bool {
        self.matches_dom(namespace.map(crate::dom_string::DomStringRef::Scalar),local)
    }
    pub(crate) fn matches_dom(&self,namespace:Option<crate::dom_string::DomStringRef<'_>>,local:&str)->bool{self.matches_names(namespace,crate::dom_string::DomStringRef::Scalar(local))}
    pub(crate) fn matches_names(&self,namespace:Option<crate::dom_string::DomStringRef<'_>>,local:crate::dom_string::DomStringRef<'_>)->bool{
        let equal=match(self.namespace_uri(),namespace){(None,None)=>true,(Some(a),Some(b))=>a.view().same_units(b),_=>false};equal&&self.local().view().same_units(local)
    }
    pub fn same_expanded_name(&self, other: &Self) -> bool { self.matches_names(other.namespace_uri().map(DomString::view), other.local().view()) }
    pub fn bytes(&self) -> usize {
        self.namespace.len()*std::mem::size_of::<AttributeNamespace>() + self.qualified.used_bytes() + self.namespace.first().map_or(0, |ns| ns.uri.used_bytes() + ns.local.used_bytes() + ns.prefix.as_ref().map_or(0, DomString::used_bytes))
    }
    pub(crate) fn owned_bytes(&self)->usize {
        self.namespace.capacity()*std::mem::size_of::<AttributeNamespace>()+self.qualified.capacity()+self.namespace.first().map_or(0,|ns|ns.uri.capacity()+ns.local.capacity()+ns.prefix.as_ref().map_or(0,DomString::capacity))
    }
    // Callers charge bytes before cloning retained native metadata.
    pub fn copy(&self) -> Result<Self,String> {
        Ok(Self { qualified: self.qualified.try_clone()?, namespace: Self::namespace_storage(self.namespace.first().map(|ns|
            Ok::<_,String>(AttributeNamespace { uri: ns.uri.try_clone()?, prefix: ns.prefix.as_ref().map(DomString::try_clone).transpose()?, local: ns.local.try_clone()? })).transpose()?)? })
    }
}

#[derive(Debug)]
pub struct Attribute {
    pub name: AttributeName,
    pub value: DomString,
}

impl Attribute {
    pub fn unqualified(name: String, value: String) -> Self { Self { name: AttributeName::unqualified(name), value:value.into() } }
    pub fn bytes(&self) -> usize { self.name.bytes() + self.value.used_bytes() }
    pub(crate) fn owned_bytes(&self)->usize{self.name.owned_bytes()+self.value.capacity()}
}

#[derive(Debug)]
pub enum NodeKind {
    Element {
        tag: String,
        expanded_name: Vec<ExpandedElementName>,
        id: Option<String>,
        href: Option<String>,
        attrs: Vec<Attribute>,
    },
    Text(DomString),
    Cdata(DomString),
    // Non-rendering DOM Comment data retained with its real parent and order.
    Comment(DomString),
    ProcessingInstruction { target: String, data: DomString },
    // A genuine detached Node container; insertion splices its children.
    DocumentFragment,
    DocumentType(DocumentTypeData),
}

impl NodeKind {
    pub(crate) fn character_dom_string(&self)->Option<&DomString>{
        match self {Self::Text(data)|Self::Cdata(data)|Self::Comment(data)|Self::ProcessingInstruction {data,..}=>Some(data),_=>None}
    }
    pub(crate) fn character_data(&self) -> Option<&str> {
        match self {
            Self::Text(data) | Self::Cdata(data) | Self::Comment(data) | Self::ProcessingInstruction { data, .. } => Some(data),
            Self::Element { .. } | Self::DocumentFragment | Self::DocumentType(_) => None,
        }
    }

    pub(crate) fn character_data_capacity(&self) -> Option<usize> {
        match self {
            Self::Text(data) | Self::Cdata(data) | Self::Comment(data) | Self::ProcessingInstruction { data, .. } => Some(data.capacity()),
            Self::Element { .. } | Self::DocumentFragment | Self::DocumentType(_) => None,
        }
    }

    pub(crate) fn is_nonrendered_node(&self) -> bool {
        matches!(self, Self::Comment(_) | Self::ProcessingInstruction { .. } | Self::DocumentType(_))
    }
}

pub fn parse(input: &str) -> Document {
    parse_internal(input, None, false, None, None).expect("Document parsing retains its bounded prefix behavior.")
}

#[derive(Clone, Copy)]
struct LoadedTree<'a> {
    nodes: usize,
    check_cancel: &'a dyn Fn() -> Result<(), String>,
}

pub(crate) fn parse_loaded_document(
    input: &str,
    check_cancel: &dyn Fn() -> Result<(), String>,
) -> Result<Document, String> {
    parse_internal(input, None, false, Some(LoadedTree {
        nodes: MAX_LOADED_DOCUMENT_NODES,
        check_cancel,
    }), None)
}

/// Inert documents reuse the tokenizer with cancellation and strict capacity
/// admission. The returned index identifies the actual retained doctype node.
pub(crate) fn parse_inert_html(
    input: &str, nodes: usize, check_cancel: &dyn Fn() -> Result<(), String>,
) -> Result<(Document, Option<usize>), String> {
    let mut doctype = None;
    let parsed = parse_internal(
        input, None, false, Some(LoadedTree { nodes, check_cancel }), Some(&mut doctype),
    )?;
    Ok((parsed, doctype))
}

/// Checked fragment subset. The context sets the tokenizer state directly;
/// callers never create a source wrapper that markup could escape. Unsupported
/// tree-builder recovery and declaration forms reject explicitly.
pub(crate) fn parse_fragment(input: &str, context: &str, scripting: bool) -> Result<Document, String> {
    if !matches!(context, "div" | "span" | "p" | "a" | "section" | "article" | "main" | "header" | "footer" | "aside" | "nav" | "li" | "ul" | "ol" | "dl" | "dt" | "dd" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "em" | "b" | "strong" | "i" | "u" | "small" | "label" | "pre" | "blockquote" | "body" | "title" | "textarea" | "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext" | "select" | "optgroup" | "option" | "template") {
        return Err("HTML fragment parsing for this element context is not implemented.".into());
    }
    parse_internal(input, Some(context), scripting, None, None)
}

pub(crate) fn svg_dom_name(name:&str)->bool{svg_namespace::dom_name(name)}
pub(crate) fn svg_dom_attribute(tag:&str,name:&AttributeName,value:&str)->bool{svg_namespace::dom_attribute(tag,name,value)}
pub(crate) fn svg_dom_transfer(node:&Node)->bool{node.namespace==Namespace::Svg&&match &node.kind{NodeKind::Element{tag,attrs,..}=>svg_dom_name(tag)&&attrs.iter().all(|attr|svg_dom_attribute(tag,&attr.name,&attr.value)),_=>false}}

/// HTML's autonomous custom-element names start with an ASCII lowercase
/// letter and use the DOM local-name grammar.
/// https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name
pub(crate) fn valid_custom_element_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name.contains('-')
        && !name.bytes().any(|b| matches!(b, b'A'..=b'Z' | 0 | b'\t' | b'\n' | 0x0c | b'\r' | b' ' | b'/' | b'>'))
        && !matches!(name, "annotation-xml" | "color-profile" | "font-face" | "font-face-src"
            | "font-face-uri" | "font-face-format" | "font-face-name" | "missing-glyph")
}

fn parse_internal(
    input: &str,
    context: Option<&str>,
    scripting: bool,
    loaded: Option<LoadedTree<'_>>,
    mut inert_doctype: Option<&mut Option<usize>>,
) -> Result<Document, String> {
    let checked = context.is_some();
    let template_svg = context == Some("template");
    let mut initial=context.is_none();
    if let Some(loaded) = loaded { (loaded.check_cancel)()?; }
    let node_limit = loaded.map_or(MAX_NODES, |loaded| loaded.nodes);
    let fail_on_capacity = checked || loaded.is_some();
    let node_limit_error = || if checked {
        "HTML fragment node limit reached.".into()
    } else {
        format!("HTML document node limit reached ({node_limit} nodes).")
    };
    // HTML input preprocessing happens before tokenization, so an LF produced
    // by a numeric reference remains distinct from a literal source CR.
    let normalized = if input.contains('\r') {
        Cow::Owned(input.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(input)
    };
    let input = normalized.as_ref();
    if checked && input.contains('\0') && !context.is_some_and(|name| matches!(name, "title" | "textarea" | "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext")) {
        return Err("HTML fragment NULL handling outside text-only contexts is not implemented.".into());
    }
    let mut document = Document {
        kind:DocumentKind::Html,mode:if context.is_none(){DocumentMode::Quirks}else{DocumentMode::NoQuirks},
        stylesheet_limits: crate::css::StylesheetLimits::default(),
        media_environment: None,
        title: String::new(),
        content_type: "text/html".into(),
        encoding: crate::text_codec::Encoding::UTF8,
        scripting_enabled: false,
        nonce_unsafe_nodes: HashSet::new(),
        withheld_inline_styles: HashSet::new(),
            native_focus: crate::html::NativeNode::default(),
            native_hover: crate::html::NativeNode::default(),
        nodes: vec![Node {
            kind: NodeKind::Element {expanded_name:Vec::new(),
                tag: context.unwrap_or("#document").into(),
                id: None,
                href: None,
                attrs: Vec::new(),
            },
            children: Vec::new(),
            namespace: Namespace::None,
        }],
    };
    if context.is_some_and(|name| matches!(name, "title" | "textarea" | "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext")) {
        let text = if matches!(context, Some("title" | "textarea")) {
            decode_entities(input).replace('\0', "\u{fffd}")
        } else {
            html_raw_text(input, None)?
        };
        if !append_text_value(&mut document, 0, text, node_limit)? { return Err(node_limit_error()); }
        document.scripting_enabled = scripting;
        return Ok(document);
    }
    let mut stack = vec![0];
    let mut position = 0;
    let mut title_seen = false;
    // If unsupported foreign recovery escapes through an unrelated ancestor,
    // later elements cannot safely be classified from this subset's stack.
    let mut uncertain_foreign_recovery = false;
    let mut svg = svg_namespace::Island::default();
    let mut ignore_fragment_lf = None;
    let mut tokens = 0u8;

    while position < input.len() {
        if tokens == 0 {
            if let Some(loaded) = loaded { (loaded.check_cancel)()?; }
        }
        tokens = tokens.wrapping_add(1);
        let parent = *stack.last().unwrap();
        let rest = &input[position..];
        if !rest.starts_with('<') {
            let end = position + rest.find('<').unwrap_or(rest.len());
            let before_root=context.is_none()&&parent==0&&!document.nodes[0].children.iter().any(|&node|matches!(document.nodes[node].kind,NodeKind::Element{..}));
            if before_root {
                let source=&input[position..end];let trimmed=source.trim_start_matches(|ch:char|ch.is_ascii()&&tag_space(ch as u8,true));
                position+=source.len()-trimmed.len();if position==end{continue;}
            }
            if initial {initial=false;}

            if svg.root.is_some() && !input[position..end].bytes().all(|b| tag_space(b, true)) {
                if template_svg { return Err("Template SVG only admits whitespace between supported shapes.".into()); }
                svg.root = None;
            }
            let appended = if checked {
                let mut text = decode_entities(&input[position..end]);
                if ignore_fragment_lf.take() == Some(parent) && text.starts_with('\n') { text.remove(0); }
                append_text_value(&mut document, parent, text, node_limit)?
            } else { append_text(&mut document, parent, &input[position..end], node_limit)? };
            if !appended {
                if fail_on_capacity { return Err(node_limit_error()); }
                break;
            }
            position = end;
            continue;
        }
        ignore_fragment_lf = None;

        if let Some(comment) = rest.strip_prefix("<!--") {
            // A comment neither leaves the current tree context nor terminates
            // a checked SVG island. Its data never reenters markup scanning.
            let (data,consumed)=comments::token(comment);
            if document.nodes.len()>=node_limit {
                if fail_on_capacity{return Err(node_limit_error());}break;
            }
            let data=comments::copy_data(data)?;
            document.nodes.try_reserve(1).map_err(|_|"Could not reserve HTML comment node.")?;
            document.nodes[parent].children.try_reserve(1).map_err(|_|"Could not reserve HTML comment child link.")?;
            let node=document.nodes.len();
            document.nodes.push(Node{kind:NodeKind::Comment(data.into()),namespace:Namespace::None,children:Vec::new()});
            document.nodes[parent].children.push(node);
            position+=4+consumed;continue;
        }
        if !checked&&rest.get(..9).is_some_and(|prefix|prefix.eq_ignore_ascii_case("<!doctype")) {
            let token=doctype::read(&rest[9..],loaded.map(|loaded|loaded.check_cancel))?;
            if initial {
                if document.nodes.len()==node_limit {if fail_on_capacity{return Err(node_limit_error());}break;}
                document.nodes.try_reserve(1).map_err(|_|"Cannot reserve native doctype node.")?;
                document.nodes[0].children.try_reserve(1).map_err(|_|"Cannot reserve native doctype child link.")?;
                let node=document.nodes.len();document.mode=token.mode;
                document.nodes.push(Node{kind:NodeKind::DocumentType(token.data),namespace:Namespace::None,children:Vec::new()});
                document.nodes[0].children.push(node);
                if let Some(doctype)=inert_doctype.as_deref_mut(){*doctype=Some(node);}
                initial=false;
            }
            position+=9+token.consumed;continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            svg.root=None;
            if checked{return Err("HTML fragment declaration/comment tokens are not implemented.".into());}
            if inert_doctype.is_some(){return Err("HTML bogus-comment recovery is outside the inert parser subset.".into());}
            position+=rest.find('>').map_or(rest.len(),|end|end+1);continue;
        }

        let tag = match read_tag_mode(input, position, checked) {
            TagToken::Complete(tag) => tag,
            TagToken::Incomplete => break,
            TagToken::Unsupported(error) => return Err(error.into()),
            TagToken::NotATag => {
                if template_svg && svg.root.is_some() {
                    return Err("Template SVG contains a malformed tag.".into());
                }
                svg.root = None;
                if checked && rest.starts_with("</") { return Err("HTML fragment bogus end-tag/comment recovery is not implemented.".into()); }
                if !append_text(&mut document, parent, "<", node_limit)? {
                    if fail_on_capacity { return Err(node_limit_error()); }
                    break;
                }
                position += 1;
                continue;
            }
        };
        if initial {initial=false;}
        position = tag.end;

        let empty_table = checked && tag.name == "table";
        let starts_template_svg = template_svg && tag.name == "svg" && !tag.closing;
        let inside_template_svg = template_svg && svg.root.is_some();
        if checked {
            // Template contents form a fresh HTML scope. End tags inside one
            // cannot close an ancestor in the surrounding fragment.
            let scope_start = stack.iter().rposition(|&node| matches!(&document.nodes[node].kind,NodeKind::Element{tag,..} if tag=="template")).unwrap_or(0);
            let scope = &stack[scope_start..];
            checked_select_tag(scope, &document, &tag.name, tag.closing)?;
            if template_svg && tag.name=="template" && svg.root.is_none() && tag.closing {
                if scope_start>0 {stack.truncate(scope_start);}continue;
            }
            if tag.name == "textarea" && (tag.closing || tag.self_closing || !tag.clean_syntax) {
                return Err("HTML fragment TEXTAREA requires an ordinary start tag and a matching end tag.".into());
            }
            if empty_table {
                position = checked_empty_table_end(input, &tag, &stack, &document)?;
            } else if matches!(tag.name.as_str(), "html" | "head" | "body" | "frameset" | "frame" | "caption" | "colgroup" | "col" | "tbody" | "thead" | "tfoot" | "tr" | "td" | "th" | "math" | "picture" | "image" | "xmp" | "noembed" | "noframes" | "plaintext" | "listing")
                || tag.name == "template" && (!template_svg || svg.root.is_some())
                || tag.name == "svg" && !(starts_template_svg || inside_template_svg)
                || tag.name == "noscript" && scripting {
                return Err(format!("HTML fragment structural, table/select or foreign-content recovery is not implemented (tag {:.32}, closing {}, context {:.32}).",tag.name,tag.closing,context.unwrap_or("#document")));
            }
            if (starts_template_svg || inside_template_svg) && !tag.closing {
                if let Some((kind,name))=svg_namespace::template_refusal(&tag){
                    return Err(format!("Template SVG contains an unsupported {kind}: {name:.64} (element {:.64}).",tag.name));
                }
            }
            if !tag.closing {
                let heading = |name: &str| matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
                if scope.iter().skip(1).any(|&node| matches!(&document.nodes[node].kind, NodeKind::Element {tag: open, ..} if (matches!(tag.name.as_str(), "dt" | "dd") && matches!(open.as_str(), "dt" | "dd")) || (heading(&tag.name) && heading(open)) || (open == "p" && matches!(tag.name.as_str(), "address" | "aside" | "details" | "dialog" | "dl" | "dt" | "dd" | "figcaption" | "figure" | "hgroup" | "listing" | "menu" | "nav" | "search" | "summary")))) {
                    return Err("HTML fragment implied definition/heading/block recovery is not implemented.".into());
                }
            }
            if tag.closing {
                if tag.name == "br" || tag.name == "p" && !scope.iter().skip(1).any(|&node| matches!(&document.nodes[node].kind, NodeKind::Element {tag, ..} if tag == "p")) { return Err("HTML fragment special end-tag recovery is not implemented.".into()); }
                if let Some(position) = scope.iter().rposition(|&node| matches!(&document.nodes[node].kind, NodeKind::Element {tag: name, ..} if name == &tag.name)) {
                    if position > 0 && (fragment_formatting(&tag.name) && position + 1 != scope.len() || scope[position+1..].iter().any(|&node| matches!(&document.nodes[node].kind, NodeKind::Element {tag, ..} if fragment_formatting(tag)))) {
                        return Err("HTML fragment formatting-element reconstruction is not implemented.".into());
                    }
                }
            } else if fragment_formatting(&tag.name) && scope.iter().skip(1).any(|&node| matches!(&document.nodes[node].kind, NodeKind::Element {tag: name, ..} if name == &tag.name)) {
                return Err("HTML fragment nested formatting recovery is not implemented.".into());
            }
        }

        if tag.closing {
            if let Some(root) = svg.root {
                let exact = tag.clean_syntax && tag.attrs.is_empty() && !tag.self_closing &&
                    matches!(&document.nodes[parent].kind, NodeKind::Element { tag: open, .. } if open == &tag.name);
                if !exact {
                    if template_svg { return Err("Template SVG requires matching end tags.".into()); }
                    svg.root = None;
                } else if parent == root {
                    svg_namespace::publish(&mut document.nodes[root..]);
                    svg.root = None;
                }
            }
            let floor=if template_svg {stack.iter().rposition(|&node|matches!(&document.nodes[node].kind,NodeKind::Element{tag,..} if tag=="template")).unwrap_or(0)}else{0};
            if let Some(index) = stack.iter().enumerate().rfind(|(index,node)| *index>floor && matches!(&document.nodes[**node].kind,NodeKind::Element{tag:name,..} if name==&tag.name)).map(|(index,_)|index) {
                if index > 0 {
                    if document.nodes[*stack.last().unwrap()].namespace == Namespace::UnknownForeign
                        && document.nodes[stack[index]].namespace != Namespace::UnknownForeign
                    {
                        uncertain_foreign_recovery = true;
                    }
                    stack.truncate(index);
                }
            }
            continue;
        }

        let was_foreign = document.nodes[*stack.last().unwrap()].namespace == Namespace::UnknownForeign;
        let previous_stack = checked.then(|| stack.clone());
        close_implied(&mut stack, &document, &tag.name);
        if previous_stack.as_ref().is_some_and(|previous| previous[stack.len()..].iter().any(|&node| matches!(&document.nodes[node].kind, NodeKind::Element {tag, ..} if fragment_formatting(tag)))) {
            return Err("HTML fragment implied-close formatting reconstruction is not implemented.".into());
        }
        let parent = *stack.last().unwrap();
        if was_foreign && document.nodes[parent].namespace != Namespace::UnknownForeign {
            uncertain_foreign_recovery = true;
        }
        if svg.blocked_depth.is_some_and(|depth| stack.len() < depth) {
            svg.blocked_depth = None;
        }
        let node = document.nodes.len();
        let enters_svg = tag.name == "svg" && (!checked || template_svg) && !uncertain_foreign_recovery &&
            svg.blocked_depth.is_none() && (parent == 0 || document.nodes[parent].namespace == Namespace::Html);
        let svg_empty = tag.self_closing && (enters_svg || svg.root.is_some()) &&
            svg_namespace::shape_element(&tag.name);
        if enters_svg {
            svg.root = Some(node);
        }
        if !template_svg && svg.root.is_some() && !svg_namespace::supported_tag(&tag) {
            svg.root = None;
        }
        let html_raw = matches!(tag.name.as_str(), "iframe" | "xmp" | "noembed" | "noframes")
            && !uncertain_foreign_recovery && svg.root.is_none()
            && matches!(document.nodes[parent].namespace, Namespace::None | Namespace::Html);
        let raw = html_raw || matches!(tag.name.as_str(), "script" | "style" | "title" | "textarea");
        let void = is_void(&tag.name);
        let nesting_full = !raw && !void && !svg_empty && stack.len() == MAX_DEPTH;
        if document.nodes.len() == node_limit || nesting_full {
            if checked { return Err("HTML fragment node/depth limit reached.".into()); }
            if fail_on_capacity {
                return Err(if nesting_full {
                    format!("HTML document nesting limit reached ({MAX_DEPTH} open nodes).")
                } else {
                    node_limit_error()
                });
            }
            break;
        }
        if matches!(tag.name.as_str(), "script" | "style" | "link") && tag.duplicate_attributes {
            document.nonce_unsafe_nodes.insert(node);
        }
        document.nodes[parent].children.push(node);
        document.nodes.push(Node {
            namespace: if uncertain_foreign_recovery
                || matches!(tag.name.as_str(), "svg" | "math")
                || document.nodes[parent].namespace == Namespace::UnknownForeign
            { Namespace::UnknownForeign } else { Namespace::Html },
            kind: NodeKind::Element {expanded_name:Vec::new(),
                tag: tag.name.clone(),
                id: tag.id,
                href: tag.href,
                attrs: tag.attrs.into_iter().map(|(name, value)| Attribute::unqualified(name, value)).collect(),
            },
            children: Vec::new(),
        });

        if raw {
            let check = loaded.map(|tree| tree.check_cancel);
            let (content_end, after_end_tag) = raw_text_end_mode(input, position, &tag.name, checked, check)?;
            if checked && tag.name == "script" && input[position..content_end].contains("<!--") { return Err("HTML fragment escaped script-data tokenization is not implemented.".into()); }
            if matches!(tag.name.as_str(), "title" | "textarea") {
                // RCDATA recognizes only its own end tag as markup. Decode
                // entities after finding that tag so '&lt;/textarea&gt;' stays
                // text rather than prematurely closing the element.
                let mut content =
                    decode_entities(&input[position..content_end]).replace('\0', "\u{fffd}");
                if tag.name == "textarea" && content.starts_with('\n') {
                    content.remove(0);
                }
                if tag.name == "title" && !title_seen {
                    title_seen = true;
                    // Keep a useful window title even when the source spans lines.
                    document.title = content
                        .split_ascii_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                }
                if !append_text_value(&mut document, node, content, node_limit)? {
                    if fail_on_capacity { return Err(node_limit_error()); }
                    break;
                }
            } else if html_raw {
                let content = html_raw_text(&input[position..content_end], check)?;
                if !append_text_value(&mut document, node, content, node_limit)? {
                    if fail_on_capacity { return Err(node_limit_error()); }
                    break;
                }
            } else if matches!(tag.name.as_str(), "style" | "script")
                && !append_text_value(&mut document, node, input[position..content_end].to_owned(), node_limit)?
            {
                if fail_on_capacity { return Err(node_limit_error()); }
                break;
            }
            position = after_end_tag;
        } else if svg_empty {
            if svg.root == Some(node) {
                svg_namespace::publish(&mut document.nodes[node..]);
                svg.root = None;
            }
        } else if !void && !empty_table {
            // A trailing slash does not close a non-void HTML element.
            stack.push(node);
            if !checked && document.nodes[node].namespace == Namespace::Html &&
                svg.blocked_depth.is_none() && !svg_namespace::flow_context(&tag.name) {
                svg.blocked_depth = Some(stack.len());
            }
            if checked && tag.name == "pre" { ignore_fragment_lf = Some(node); }
        }
    }

    if template_svg && svg.root.is_some() { return Err("Template SVG requires an explicit close.".into()); }
    if checked { document.scripting_enabled = scripting; }
    if let Some(loaded) = loaded { (loaded.check_cancel)()?; }
    Ok(document)
}

// An immediately closed table has no table-mode children to insert. Consume
// its real end token once, leaving both surrounding text runs in their parent.
fn checked_empty_table_end(input: &str, open: &Tag, stack: &[usize], document: &Document) -> Result<usize, String> {
    let flow = stack.iter().all(|&node| matches!(&document.nodes[node].kind,
        NodeKind::Element { tag, .. } if matches!(tag.as_str(),
            "body" | "main" | "div" | "section" | "article" | "header" | "footer" | "aside" | "nav" |
            "blockquote" | "figure" | "figcaption" | "ul" | "ol" | "li" | "dl" | "dt" | "dd")));
    if flow && open.clean_syntax && !open.closing && !open.self_closing && input[open.end..].starts_with("</") {
        if let TagToken::Complete(end) = read_tag_mode(input, open.end, true) {
            if end.name == "table" && end.clean_syntax && !end.self_closing && end.attrs.is_empty() {
                return Ok(end.end);
            }
        }
    }
    Err("HTML fragment tables require ordinary flow and an immediately matching empty-table end tag.".into())
}

// This finite conventional select slice follows the current in-body select,
// option and optgroup rules where the existing stack is sufficient. It does
// not claim customizable-select content or unsupported parser recovery.
// Validate before the generic stack can pop an unrelated ancestor or publish
// any result. Unchecked full-document parsing deliberately keeps its policy.
fn checked_select_tag(stack: &[usize], document: &Document, incoming: &str, closing: bool) -> Result<(), String> {
    let name = |node: usize| match &document.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag.as_str(),
        _ => "",
    };
    let context = name(0);
    let select = stack.iter().position(|&node| name(node) == "select");
    let active = select.is_some() || matches!(context, "optgroup" | "option");
    let unsupported = || "HTML fragment nonconventional select recovery is not implemented.".to_owned();
    if !active {
        return if (incoming == "select" && !closing) || !matches!(incoming, "select" | "option" | "optgroup") {
            Ok(())
        } else { Err(unsupported()) };
    }
    // A conventional option fragment can contain character data only in this
    // slice. Rich option content remains explicitly unsupported.
    if context == "option" { return Err(unsupported()); }
    let current = *stack.last().unwrap();
    match (incoming, closing) {
        ("select", false) => Err(unsupported()), // Nested select needs recovery.
        ("select", true) if select.is_some_and(|position| position > 0) => Ok(()),
        ("option", false) => Ok(()), // close_implied pops the prior option.
        ("option", true) if current != 0 && name(current) == "option" => Ok(()),
        ("optgroup", false) if context != "optgroup" => Ok(()),
        ("optgroup", true) if stack.iter().skip(1).any(|&node| name(node) == "optgroup") => Ok(()),
        _ => Err(unsupported()),
    }
}

fn fragment_formatting(name: &str) -> bool {
    matches!(name, "a" | "b" | "i" | "em" | "strong" | "u" | "small" | "big" | "code" | "font" | "nobr" | "s" | "strike" | "tt")
}

fn append_text(document: &mut Document, parent: usize, source: &str, node_limit: usize) -> Result<bool,String> {
    append_text_value(document, parent, decode_entities(source), node_limit)
}

fn append_text_value(document: &mut Document, parent: usize, text: String, node_limit: usize) -> Result<bool,String> {
    if text.is_empty() {
        return Ok(true);
    }
    if let Some(&last) = document.nodes[parent].children.last() {
        if let NodeKind::Text(previous) = &mut document.nodes[last].kind {
            previous.append_parser_scalar(&text)?;
            return Ok(true);
        }
    }
    if document.nodes.len() == node_limit {
        return Ok(false);
    }
    let node = document.nodes.len();
    document.nodes[parent].children.push(node);
    document.nodes.push(Node {
        kind: NodeKind::Text(text.into()),
        children: Vec::new(),
        namespace: Namespace::None,
    });
    Ok(true)
}

struct Tag {
    name: String,
    id: Option<String>,
    href: Option<String>,
    attrs: Vec<(String, String)>,
    closing: bool,
    self_closing: bool,
    clean_syntax: bool,
    end: usize,
    duplicate_attributes: bool,
}

enum TagToken {
    Complete(Tag),
    Incomplete,
    NotATag,
    Unsupported(&'static str),
}

fn tag_space(byte: u8, checked: bool) -> bool {
    if checked { matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c') } else { byte.is_ascii_whitespace() }
}

fn read_tag_mode(input: &str, start: usize, checked: bool) -> TagToken {
    let bytes = input.as_bytes();
    let mut position = start + 1;
    let closing = bytes.get(position) == Some(&b'/');
    if closing {
        position += 1;
    }
    if !bytes.get(position).is_some_and(u8::is_ascii_alphabetic) {
        return TagToken::NotATag;
    }
    let name_start = position;
    while position < bytes.len()
        && (bytes[position].is_ascii_alphanumeric()
            || matches!(bytes[position], b'-' | b'_' | b':'))
    {
        position += 1;
    }
    let name = input[name_start..position].to_ascii_lowercase();
    // Only explicitly supported SVG elements and these checked fragments use
    // strict token evidence. Other elements retain the normal tokenizer rules.
    let checked_native_fragment = checked && matches!(name.as_str(), "table" | "textarea");
    let mut clean_syntax = (svg_namespace::shape_element(&name) || checked_native_fragment)
        && bytes.get(position).is_some_and(|b| tag_space(*b, true) || matches!(b, b'/' | b'>'));
    let mut self_closing = false;
    if checked && bytes.get(position).is_some_and(|byte| !tag_space(*byte, true) && !matches!(*byte, b'/' | b'>')) { return TagToken::Unsupported("HTML fragment tag-name tokenization is not implemented for this boundary."); }
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    let mut duplicate_attributes = false;

    while position < bytes.len() {
        while position < bytes.len() && tag_space(bytes[position], checked) {
            position += 1;
        }
        if position == bytes.len() {
            break;
        }
        if bytes[position] == b'>' {
            return TagToken::Complete(Tag {
                name,
                id: attrs
                    .iter()
                    .find(|(name, _)| name == "id")
                    .map(|(_, value)| value.clone()),
                href: attrs
                    .iter()
                    .find(|(name, _)| name == "href")
                    .map(|(_, value)| value.clone()),
                attrs,
                closing,
                self_closing,
                clean_syntax: clean_syntax && !duplicate_attributes && !input[start..position].contains(['\0', '\x0b']),
                end: position + 1,
                duplicate_attributes,
            });
        }
        if bytes[position] == b'/' {
            self_closing = bytes.get(position + 1) == Some(&b'>');
            clean_syntax &= self_closing;
            position += 1;
            continue;
        }

        let attribute_start = position;
        while position < bytes.len()
            && !tag_space(bytes[position], checked)
            && !matches!(bytes[position], b'=' | b'/' | b'>')
        {
            position += 1;
        }
        if position == attribute_start {
            if checked { return TagToken::Unsupported("HTML fragment attribute-name recovery is not implemented."); }
            // A stray '=' must still advance the parser.
            clean_syntax = false;
            position += 1;
            continue;
        }
        let attribute = &input[attribute_start..position];
        if checked_native_fragment && attribute.contains(['"', '\'', '<']) {
            clean_syntax = false;
        }
        while position < bytes.len() && tag_space(bytes[position], checked) {
            position += 1;
        }

        let mut value = "";
        if bytes.get(position) == Some(&b'=') {
            position += 1;
            while position < bytes.len() && tag_space(bytes[position], checked) {
                position += 1;
            }
            if matches!(bytes.get(position), Some(b'\'' | b'"')) {
                let quote = bytes[position];
                position += 1;
                let value_start = position;
                while position < bytes.len() && bytes[position] != quote {
                    position += 1;
                }
                value = &input[value_start..position];
                if position < bytes.len() {
                    position += 1;
                    clean_syntax &= bytes.get(position).is_some_and(|b| tag_space(*b, true) || matches!(b, b'/' | b'>'));
                }
            } else {
                let value_start = position;
                while position < bytes.len()
                    && !tag_space(bytes[position], checked)
                    && bytes[position] != b'>'
                {
                    position += 1;
                }
                value = &input[value_start..position];
                clean_syntax = clean_syntax && !value.is_empty() && !value.contains(['"', '\'', '<', '=', '`']);
            }
        }

        // Hashing avoids quadratic scans when a tag contains many attributes.
        let attribute = attribute.to_ascii_lowercase();
        if seen.insert(attribute.clone()) {
            attrs.push((attribute, decode_entities_in(value, true)));
        } else {
            duplicate_attributes = true;
        }
    }

    if checked_native_fragment {
        TagToken::Unsupported(if name == "table" { "Incomplete HTML fragment table tag." } else { "Incomplete HTML fragment TEXTAREA tag." })
    } else {
        TagToken::Incomplete
    }
}

fn close_implied(stack: &mut Vec<usize>, document: &Document, incoming: &str) {
    if matches!(
        incoming,
        "p" | "div"
            | "center"
            | "table"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "li"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "main"
            | "blockquote"
            | "pre"
            | "hr"
    ) {
        close_in_scope(
            stack,
            document,
            "p",
            &["html", "table", "td", "th", "button"],
        );
    }
    if incoming == "li" {
        close_in_scope(stack, document, "li", &["ul", "ol", "html"]);
    }
    if matches!(incoming, "option" | "optgroup") {
        close_in_scope(
            stack,
            document,
            "option",
            &["select", "datalist", "optgroup", "html"],
        );
    }
    if incoming == "optgroup" {
        close_in_scope(stack, document, "optgroup", &["select", "datalist", "html"]);
    }
    if matches!(incoming, "td" | "th" | "tr" | "thead" | "tbody" | "tfoot") {
        close_in_scope(stack, document, "td", &["tr", "table", "html"]);
        close_in_scope(stack, document, "th", &["tr", "table", "html"]);
    }
    if matches!(incoming, "tr" | "thead" | "tbody" | "tfoot") {
        close_in_scope(stack, document, "tr", &["table", "html"]);
    }
    if matches!(incoming, "thead" | "tbody" | "tfoot") {
        for tag in ["thead", "tbody", "tfoot"] {
            close_in_scope(stack, document, tag, &["table", "html"]);
        }
    }
}

fn close_in_scope(stack: &mut Vec<usize>, document: &Document, target: &str, barriers: &[&str]) {
    for position in (1..stack.len()).rev() {
        if let NodeKind::Element { tag, .. } = &document.nodes[stack[position]].kind {
            if tag == target {
                stack.truncate(position);
                return;
            }
            if tag == "template" && document.nodes[stack[position]].namespace == Namespace::Html || barriers.contains(&tag.as_str()) {
                return;
            }
        }
    }
}

const RAW_TEXT_CHECK_BYTES: usize = 16 * 1024;

fn html_raw_text(input: &str, check: Option<&dyn Fn() -> Result<(), String>>) -> Result<String, String> {
    let mut bytes = input.len();
    for chunk in input.as_bytes().chunks(RAW_TEXT_CHECK_BYTES) {
        if let Some(check) = check { check()?; }
        let zeros = chunk.iter().filter(|&&byte| byte == 0).count();
        bytes = bytes.checked_add(2 * zeros).ok_or("HTML raw text size overflow.")?;
    }
    let mut output = String::new();
    output.try_reserve_exact(bytes).map_err(|_| "Cannot reserve HTML raw text.")?;
    let mut remaining = input;
    while !remaining.is_empty() {
        if let Some(check) = check { check()?; }
        let end = remaining.floor_char_boundary(remaining.len().min(RAW_TEXT_CHECK_BYTES));
        let (chunk, rest) = remaining.split_at(end);
        for (i, part) in chunk.split('\0').enumerate() {
            if i != 0 { output.push('\u{fffd}'); }
            output.push_str(part);
        }
        remaining = rest;
    }
    Ok(output)
}

fn raw_text_end_mode(input: &str, start: usize, name: &str, checked: bool,
    check: Option<&dyn Fn() -> Result<(), String>>) -> Result<(usize, usize), String> {
    let bytes = input.as_bytes();
    let mut position = start;
    let mut next_check = position;
    while position < bytes.len() {
        if position >= next_check {
            if let Some(check) = check { check()?; }
            next_check = position.saturating_add(RAW_TEXT_CHECK_BYTES);
        }
        let end = next_check.min(bytes.len());
        let Some(offset) = bytes[position..end].iter().position(|&b| b == b'<') else {
            position = end;
            continue;
        };
        let opening = position + offset;
        let name_start = opening + 2;
        let name_end = name_start + name.len();
        if bytes.get(opening + 1) == Some(&b'/')
            && bytes
                .get(name_start..name_end)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name.as_bytes()))
            && bytes
                .get(name_end)
                .is_some_and(|byte| tag_space(*byte, checked) || matches!(byte, b'/' | b'>'))
        {
            return match read_tag_mode(input, opening, checked) {
                TagToken::Complete(tag) if checked && name == "textarea"
                    && (!tag.clean_syntax || tag.self_closing || !tag.attrs.is_empty()) =>
                    Err("HTML fragment TEXTAREA end-tag recovery is not implemented.".into()),
                TagToken::Complete(tag) => Ok((opening, tag.end)),
                _ if checked => Err("HTML fragment raw-text end-tag recovery is not implemented.".into()),
                _ => Ok((input.len(), input.len())),
            };
        }
        position = opening + 1;
    }
    if checked && name == "textarea" {
        return Err("HTML fragment TEXTAREA requires a complete matching end tag.".into());
    }
    Ok((input.len(), input.len()))
}

pub(crate) fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn decode_entities(input: &str) -> String {
    decode_entities_in(input, false)
}

fn decode_entities_in(input: &str, attribute: bool) -> String {
    let mut output = String::with_capacity(input.len());
    let mut position = 0;
    while let Some(offset) = input[position..].find('&') {
        let opening = position + offset;
        output.push_str(&input[position..opening]);
        let rest = &input[opening + 1..];
        if let Some((character, consumed)) = character_reference(rest, attribute) {
            output.push(character);
            position = opening + 1 + consumed;
            continue;
        }
        output.push('&');
        position = opening + 1;
    }
    output.push_str(&input[position..]);
    output
}

fn character_reference(input: &str, attribute: bool) -> Option<(char, usize)> {
    let bytes = input.as_bytes();
    if bytes.first() == Some(&b'#') {
        let mut position = 1;
        let radix = if matches!(bytes.get(position), Some(b'x' | b'X')) {
            position += 1;
            16
        } else {
            10
        };
        let first_digit = position;
        let mut number = 0u32;
        while let Some(digit) = bytes
            .get(position)
            .and_then(|byte| char::from(*byte).to_digit(radix))
        {
            // Continue consuming a long numeric reference once, even after it
            // overflows; saturation guarantees the replacement character.
            number = number.saturating_mul(radix).saturating_add(digit);
            position += 1;
        }
        if position == first_digit {
            return None;
        }
        if bytes.get(position) == Some(&b';') {
            position += 1;
        }
        return Some((numeric_character(number), position));
    }

    // A bounded longest match among our supported names. Names are case
    // sensitive. Only the legacy names below can omit their semicolon.
    let mut matched = None;
    for (index, byte) in bytes.iter().take(32).enumerate() {
        if !byte.is_ascii_alphanumeric() {
            break;
        }
        let end = index + 1;
        if let Some((character, legacy)) = named_character(&input[..end]) {
            if bytes.get(end) == Some(&b';') {
                return Some((character, end + 1));
            }
            if legacy {
                matched = Some((character, end));
            }
        }
    }
    let (character, consumed) = matched?;
    if attribute
        && bytes
            .get(consumed)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'=')
    {
        return None;
    }
    Some((character, consumed))
}

fn numeric_character(number: u32) -> char {
    // These are HTML's numeric-reference corrections, not byte decoding. The
    // undefined Windows-1252 positions (81, 8D, 8F, 90, 9D) remain controls.
    let corrected = match number {
        0 => 0xfffd,
        0x80 => 0x20ac,
        0x82 => 0x201a,
        0x83 => 0x0192,
        0x84 => 0x201e,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02c6,
        0x89 => 0x2030,
        0x8a => 0x0160,
        0x8b => 0x2039,
        0x8c => 0x0152,
        0x8e => 0x017d,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201c,
        0x94 => 0x201d,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02dc,
        0x99 => 0x2122,
        0x9a => 0x0161,
        0x9b => 0x203a,
        0x9c => 0x0153,
        0x9e => 0x017e,
        0x9f => 0x0178,
        _ => number,
    };
    char::from_u32(corrected).unwrap_or('\u{fffd}')
}

fn named_character(name: &str) -> Option<(char, bool)> {
    // Hand-selected from the standard's name/code-point facts, not an imported
    // entity database. All printable Latin-1 names and common web punctuation
    // are included. Unlisted names are unsupported; a legacy prefix can still
    // match, as it does in HTML's longest-match algorithm.
    let legacy = match name {
        "amp" | "AMP" => '&',
        "lt" | "LT" => '<',
        "gt" | "GT" => '>',
        "quot" | "QUOT" => '"',
        "nbsp" => '\u{00a0}',
        "iexcl" => '¡',
        "cent" => '¢',
        "pound" => '£',
        "curren" => '¤',
        "yen" => '¥',
        "brvbar" => '¦',
        "sect" => '§',
        "uml" => '¨',
        "copy" | "COPY" => '©',
        "ordf" => 'ª',
        "laquo" => '«',
        "not" => '¬',
        "shy" => '\u{00ad}',
        "reg" | "REG" => '®',
        "macr" => '¯',
        "deg" => '°',
        "plusmn" => '±',
        "sup2" => '²',
        "sup3" => '³',
        "acute" => '´',
        "micro" => 'µ',
        "para" => '¶',
        "middot" => '·',
        "cedil" => '¸',
        "sup1" => '¹',
        "ordm" => 'º',
        "raquo" => '»',
        "frac14" => '¼',
        "frac12" => '½',
        "frac34" => '¾',
        "iquest" => '¿',
        "Agrave" => 'À',
        "Aacute" => 'Á',
        "Acirc" => 'Â',
        "Atilde" => 'Ã',
        "Auml" => 'Ä',
        "Aring" => 'Å',
        "AElig" => 'Æ',
        "Ccedil" => 'Ç',
        "Egrave" => 'È',
        "Eacute" => 'É',
        "Ecirc" => 'Ê',
        "Euml" => 'Ë',
        "Igrave" => 'Ì',
        "Iacute" => 'Í',
        "Icirc" => 'Î',
        "Iuml" => 'Ï',
        "ETH" => 'Ð',
        "Ntilde" => 'Ñ',
        "Ograve" => 'Ò',
        "Oacute" => 'Ó',
        "Ocirc" => 'Ô',
        "Otilde" => 'Õ',
        "Ouml" => 'Ö',
        "times" => '×',
        "Oslash" => 'Ø',
        "Ugrave" => 'Ù',
        "Uacute" => 'Ú',
        "Ucirc" => 'Û',
        "Uuml" => 'Ü',
        "Yacute" => 'Ý',
        "THORN" => 'Þ',
        "szlig" => 'ß',
        "agrave" => 'à',
        "aacute" => 'á',
        "acirc" => 'â',
        "atilde" => 'ã',
        "auml" => 'ä',
        "aring" => 'å',
        "aelig" => 'æ',
        "ccedil" => 'ç',
        "egrave" => 'è',
        "eacute" => 'é',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "igrave" => 'ì',
        "iacute" => 'í',
        "icirc" => 'î',
        "iuml" => 'ï',
        "eth" => 'ð',
        "ntilde" => 'ñ',
        "ograve" => 'ò',
        "oacute" => 'ó',
        "ocirc" => 'ô',
        "otilde" => 'õ',
        "ouml" => 'ö',
        "divide" => '÷',
        "oslash" => 'ø',
        "ugrave" => 'ù',
        "uacute" => 'ú',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "yacute" => 'ý',
        "thorn" => 'þ',
        "yuml" => 'ÿ',
        _ => {
            let character = match name {
                "apos" => '\'',
                "euro" => '€',
                "trade" | "TRADE" => '™',
                "ndash" => '–',
                "mdash" => '—',
                "lsquo" | "OpenCurlyQuote" => '‘',
                "rsquo" | "CloseCurlyQuote" => '’',
                "sbquo" => '‚',
                "ldquo" | "OpenCurlyDoubleQuote" => '“',
                "rdquo" | "CloseCurlyDoubleQuote" => '”',
                "bdquo" => '„',
                "hellip" => '…',
                "bull" | "bullet" => '•',
                "dagger" => '†',
                "Dagger" => '‡',
                "permil" => '‰',
                "lsaquo" => '‹',
                "rsaquo" => '›',
                "OElig" => 'Œ',
                "oelig" => 'œ',
                "Scaron" => 'Š',
                "scaron" => 'š',
                "Zcaron" => 'Ž',
                "zcaron" => 'ž',
                "Yuml" => 'Ÿ',
                "circ" => 'ˆ',
                "tilde" => '˜',
                "fnof" => 'ƒ',
                "ensp" => '\u{2002}',
                "emsp" => '\u{2003}',
                "thinsp" => '\u{2009}',
                "NonBreakingSpace" => '\u{00a0}',
                "NewLine" => '\n',
                "Tab" => '\t',
                "minus" => '−',
                "ne" => '≠',
                "le" => '≤',
                "ge" => '≥',
                "notin" | "notinva" => '∉',
                "larr" => '←',
                "rarr" => '→',
                "uarr" => '↑',
                "darr" => '↓',
                "harr" => '↔',
                _ => return None,
            };
            return Some((character, false));
        }
    };
    Some((legacy, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_raw_text_elements_keep_one_literal_child_until_their_own_end_tag() {
        for tag in ["iframe", "xmp", "noembed", "noframes"] {
            let body = format!("<b data-q='x'>é&amp;</b></{tag}more>\r\n\0tail");
            let source = format!("<{tag}>{body}</{} ><p>after</p>", tag.to_ascii_uppercase());
            let doc = parse_loaded_document(&source, &|| Ok(())).unwrap();
            let root = doc.nodes[0].children[0];
            let [child] = doc.nodes[root].children.as_slice() else { panic!("expected one {tag} text child") };
            let expected = body.replace("\r\n", "\n").replace('\0', "\u{fffd}");
            assert!(matches!(&doc.nodes[*child].kind, NodeKind::Text(data) if data == &expected));
            let tail = doc.nodes[0].children[1];
            assert!(matches!(&doc.nodes[tail].kind, NodeKind::Element { tag, .. } if tag == "p"));
            assert!(parse(&format!("<{tag}></{tag}>")).nodes[1].children.is_empty());
            let unfinished = parse(&format!("<{tag}>literal <i> and &amp;"));
            assert!(matches!(&unfinished.nodes[unfinished.nodes[1].children[0]].kind,
                NodeKind::Text(data) if data == "literal <i> and &amp;"));
        }
        let foreign = parse("<math><iframe><b>foreign</b></iframe></math>");
        assert!(foreign.nodes.iter().any(|n| n.namespace == Namespace::UnknownForeign
            && matches!(&n.kind, NodeKind::Element { tag, .. } if tag == "b")));
    }

    #[test]
    fn html_raw_text_limits_and_cancellation_return_before_document_commit() {
        let text = format!("<iframe>{}</iframe>", "x".repeat(100_000));
        let calls = std::cell::Cell::new(0);
        let error = parse_loaded_document(&text, &|| {
            calls.set(calls.get() + 1);
            if calls.get() >= 4 { Err("cancel raw text".into()) } else { Ok(()) }
        }).unwrap_err();
        assert!(error.contains("cancel raw text"));
        let limit = LoadedTree { nodes: 2, check_cancel: &|| Ok(()) };
        assert!(parse_internal("<iframe>x</iframe>", None, false, Some(limit), None).is_err());
        let fresh = parse_loaded_document("<iframe>x</iframe>", &|| Ok(())).unwrap();
        assert_eq!(fresh.nodes.len(), 3);
    }

    #[test]
    fn default_namespace_parser_provenance_is_conservative_across_foreign_recovery() {
        for markup in [
            "<html><svg><g id=x></html><p id=after>unknown</p>",
            "<p><svg><g id=x><div id=after>implied escape</div>",
        ] {
            let doc = parse(markup);
            for id in ["x", "after"] {
                let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
                assert_eq!(doc.nodes[node].namespace, Namespace::UnknownForeign, "{markup}");
            }
        }
        let doc = parse("<div id=html xmlns='urn:other'></div><math><mi id=m>x</mi></math><p id=after>known</p>");
        for (id, namespace) in [("html", Namespace::Html), ("m", Namespace::UnknownForeign), ("after", Namespace::Html)] {
            let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
            assert_eq!(doc.nodes[node].namespace, namespace);
        }
        assert!(doc.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Text(_))).all(|n| n.namespace == Namespace::None));
    }

    fn text(document: &Document) -> String {
        document
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                NodeKind::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn duplicate_attributes_keep_script_and_stylesheet_nonce_evidence() {
        let doc = parse("<link id=l rel=stylesheet nonce=first NONCE=second href=x.css><style id=s nonce=first nonce=second>p{color:red}</style><script id=j nonce=first nonce=second></script><link id=good rel=stylesheet nonce=only href=y.css>");
        for id in ["l", "s", "j", "good"] {
            let node = doc
                .nodes
                .iter()
                .enumerate()
                .find_map(|(node, _)| (doc.attr(node, "id") == Some(id)).then_some(node))
                .unwrap();
            assert_eq!(
                doc.attr(node, "nonce"),
                Some(if id == "good" { "only" } else { "first" })
            );
            assert_eq!(doc.nonce_unsafe_nodes.contains(&node), id != "good");
        }
    }

    #[test]
    fn omitted_table_cell_and_row_end_tags_keep_sibling_cells() {
        let doc = parse("<table><tr><td>A<th>B<tr><td>C<td><table><tr><td>N<td>Z</table><tbody><tr><td>D<td>E</table>");
        let rows: Vec<_> = doc
            .nodes
            .iter()
            .filter(|node| matches!(&node.kind, NodeKind::Element {tag,..} if tag == "tr"))
            .collect();
        assert_eq!(rows.len(), 4);
        for row in rows {
            let cells = row.children.iter().filter(|&&child| matches!(&doc.nodes[child].kind, NodeKind::Element {tag,..} if tag == "td" || tag == "th")).count();
            assert_eq!(cells, 2);
        }
        assert_eq!(text(&doc), "ABCNZDE");
    }

    #[test]
    fn keeps_structure_attributes_title_and_unicode() {
        let document = parse(
            "<html><head><title>  Fish &amp; Chips\n</title></head><body>\
             <H1 ID='hello' id='ignored'>Hello</H1>\
             <p><a HREF='next.html?a=1&amp;b=2'>café &#x1F642;</a><br>tail</p>\
             </body></html>",
        );
        assert_eq!(document.title, "Fish & Chips");
        let link = document
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    &node.kind, NodeKind::Element { tag, .. } if tag == "a"
                )
            })
            .unwrap();
        assert!(
            matches!(&link.kind, NodeKind::Element { href: Some(href), .. }
            if href == "next.html?a=1&b=2")
        );
        assert!(matches!(&document.nodes[link.children[0]].kind,
            NodeKind::Text(value) if value == "café 🙂"));
        assert!(document.nodes.iter().any(|node| matches!(
            &node.kind, NodeKind::Element { tag, id: Some(id), .. }
                if tag == "h1" && id == "hello"
        )));
    }

    #[test]
    fn retains_raw_css_and_scripts_without_parsing_markup_or_entities_inside_them() {
        let document = parse(
            "before<!-- hidden --><script>hidden </scripted><b>also hidden</b>\
             </ScRiPt><style>p::before { content: '&amp;'; }</style>after\
             <script>unterminated hidden",
        );
        assert_eq!(
            text(&document),
            "beforehidden </scripted><b>also hidden</b>p::before { content: '&amp;'; }afterunterminated hidden"
        );
    }

    #[test]
    fn retains_attributes_and_closes_paragraphs_and_list_items() {
        let document =
            parse("<p CLASS='one&#32;two' class='ignored' data-X='ok'>a<p>b<ul><li>c<li>d</ul>");
        let first = document.nodes[0].children[0];
        assert_eq!(document.attr(first, "class"), Some("one two"));
        assert_eq!(document.attr(first, "DATA-X"), Some("ok"));
        assert_eq!(document.attr(usize::MAX, "class"), None);
        assert_eq!(document.nodes[0].children.len(), 3);
        let list = document.nodes[0].children[2];
        assert_eq!(document.nodes[list].children.len(), 2);
    }

    #[test]
    fn omitted_option_and_group_end_tags_keep_select_choices_as_siblings() {
        let doc = parse("<select><option value=a>Alpha<option value=b>Beta<optgroup label=First><option>One<option>Two<optgroup label=Second disabled><option>Three</select><p>After</p>");
        let select = doc.nodes[0].children[0];
        let children = &doc.nodes[select].children;
        assert_eq!(children.len(), 4);
        assert_eq!(doc.attr(children[0], "value"), Some("a"));
        assert_eq!(doc.attr(children[1], "value"), Some("b"));
        assert_eq!(doc.attr(children[2], "label"), Some("First"));
        assert_eq!(doc.attr(children[3], "label"), Some("Second"));
        assert_eq!(doc.nodes[children[2]].children.len(), 2);
        assert_eq!(doc.nodes[children[3]].children.len(), 1);
        assert_eq!(doc.nodes[0].children.len(), 2);
        assert_eq!(text(&doc), "AlphaBetaOneTwoThreeAfter");

        let doc = parse("<select id=outer><option>Outer<select id=inner><option>Inner<option>Next</select></option></select>");
        let outer = doc.nodes[0].children[0];
        let option = doc.nodes[outer].children[0];
        let inner = doc.nodes[option].children[1];
        assert_eq!(doc.attr(inner, "id"), Some("inner"));
        assert_eq!(doc.nodes[inner].children.len(), 2);
    }

    #[test]
    fn recovers_predictably_and_decodes_only_supported_entities() {
        let document =
            parse("<p>2 < 3 &bogus; &#65; &#0; &#xD800; &lt;/p&gt;</wrong></p><div id='");
        assert_eq!(text(&document), "2 < 3 &bogus; A � � </p>");
        assert!(document.nodes.iter().all(|node| !matches!(
            &node.kind, NodeKind::Element { tag, .. } if tag == "div"
        )));
    }

    #[test]
    fn common_named_entities_keep_accents_punctuation_and_nonbreaking_spaces() {
        assert_eq!(
            decode_entities(
                "Im&aacute;genes &bull; B&uacute;squeda &copy; 2026 &reg; &trade; &euro;"
            ),
            "Imágenes • Búsqueda © 2026 ® ™ €"
        );
        assert_eq!(
            decode_entities(
                "&Aacute;&aacute; &Ccedil;&ccedil; &Ntilde;&ntilde; &OElig;&oelig; &Yuml;&yuml;"
            ),
            "Áá Çç Ññ Œœ Ÿÿ"
        );
        assert_eq!(
            decode_entities(
                "&ldquo;Caf&eacute;&rdquo;&nbsp;&ndash;&nbsp;&lsquo;ni&ntilde;o&rsquo;&hellip;"
            ),
            "“Café”\u{00a0}–\u{00a0}‘niño’…"
        );
        assert_eq!(
            decode_entities("&amp;lt; &unknown; &eAcute;"),
            "&lt; &unknown; &eAcute;"
        );
    }

    #[test]
    fn legacy_names_use_longest_matching_and_attribute_ambiguity_rules() {
        assert_eq!(
            decode_entities("&copycat &notit; &notin; &amp=1 &apos &trade"),
            "©cat ¬it; ∉ &=1 &apos &trade"
        );
        let document =
            parse("<a href='?a=1&copy=2&reg3&copy;=4&copy!' title='&AMP &quot;'>Link</a>");
        let link = document.nodes[0].children[0];
        assert_eq!(document.attr(link, "href"), Some("?a=1&copy=2&reg3©=4©!"));
        assert_eq!(document.attr(link, "title"), Some("& \""));
    }

    #[test]
    fn numeric_references_replace_invalid_scalars_and_apply_html_c1_corrections() {
        assert_eq!(
            decode_entities("&#0; &#xD800; &#x110000; &#128; &#x91; &#x92; &#x9f;"),
            "� � � € ‘ ’ Ÿ"
        );
        assert_eq!(
            decode_entities("&#x81; &#xFFFF; &#x1F642;"),
            "\u{81} \u{ffff} 🙂"
        );
        assert_eq!(
            decode_entities("&#65tail &#x41; &#x; &#; &#-1;"),
            "Atail A &#x; &#; &#-1;"
        );
        assert_eq!(
            decode_entities(&format!("&#{};end", "9".repeat(8_000))),
            "�end"
        );
        assert_eq!(decode_entities("&#13;&#10;"), "\r\n");
    }

    #[test]
    fn textarea_is_rcdata_and_ignores_only_its_first_line_feed() {
        let document = parse("<title> T &copy;\r\n one </title><textarea name='q'>\r\nIm&aacute;genes\r\n<b>literal</b> &lt;/textarea&gt; </textareaX>\0</TEXTAREA><p>after</p>");
        assert_eq!(document.title, "T © one");
        let textarea = document
            .nodes
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "textarea"))
            .unwrap();
        assert_eq!(textarea.children.len(), 1);
        assert!(
            matches!(&document.nodes[textarea.children[0]].kind, NodeKind::Text(value) if value == "Imágenes\n<b>literal</b> </textarea> </textareaX>�")
        );
        assert!(!document
            .nodes
            .iter()
            .any(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "b")));
        assert!(text(&document).ends_with("after"));
        let leading_reference = parse("<textarea>&#10;\nsecond line</textarea>");
        assert_eq!(text(&leading_reference), "\nsecond line");
    }

    #[test]
    fn limits_tree_size_and_nesting_without_recursion() {
        let deep = parse(&"<div>".repeat(MAX_DEPTH * 2));
        assert_eq!(deep.nodes.len(), MAX_DEPTH);
        let wide = parse(&"<br>".repeat(MAX_NODES * 2));
        assert_eq!(wide.nodes.len(), MAX_NODES);
        assert_eq!(wide.nodes[0].children.len(), MAX_NODES - 1);
    }

    #[test]
    fn loaded_document_keeps_the_tail_beyond_the_internal_node_bound() {
        let source = format!("{}<p id=tail>wide document</p>", "<br>".repeat(MAX_NODES));
        let doc = parse_loaded_document(&source, &|| Ok(())).unwrap();
        assert_eq!(doc.nodes.len(), MAX_NODES + 3);
        assert_eq!(doc.attr(MAX_NODES + 1, "id"), Some("tail"));
    }

    #[test]
    fn loaded_document_reports_limits_and_cancellation_instead_of_returning_a_prefix() {
        let loaded = Some(LoadedTree { nodes: 3, check_cancel: &|| Ok(()) });
        assert_eq!(parse_internal("<br><br>", None, false, loaded, None).unwrap().nodes.len(), 3);
        for source in ["<br><br><br>", "<br><br>tail", "<br><textarea>tail</textarea>"] {
            assert_eq!(
                parse_internal(source, None, false, loaded, None).unwrap_err(),
                "HTML document node limit reached (3 nodes)."
            );
        }
        let deep = "<div>".repeat(MAX_DEPTH);
        assert!(parse_loaded_document(&deep, &|| Ok(())).unwrap_err()
            .starts_with("HTML document nesting limit reached"));

        let checks = std::cell::Cell::new(0);
        let wide = "<br>".repeat(512);
        let error = parse_loaded_document(&wide, &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 3 { Err("Cancelled".into()) } else { Ok(()) }
        }).unwrap_err();
        assert_eq!(error, "Cancelled");
    }
}

#[cfg(test)]
#[path = "html_select_fragment_tests.rs"]
mod select_fragment_tests;

#[path = "html_comments.rs"]
mod comments;
#[cfg(test)]
#[path = "html_comment_tests.rs"]
mod comment_tests;
