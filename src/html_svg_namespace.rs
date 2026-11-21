//! Provenance for balanced inline SVG shapes in ordinary HTML flow.
use super::{AttributeNamespace, Namespace, Node, NodeKind, Tag};

#[derive(Default)]
pub(super) struct Island {
    // Nodes stay unknown until the outer SVG end tag has been consumed.
    pub(super) root: Option<usize>,
    // The first still-open HTML ancestor whose insertion mode is unsupported.
    pub(super) blocked_depth: Option<usize>,
}

pub(super) fn flow_context(name: &str) -> bool {
    matches!(name, "html" | "body" | "main" | "article" | "section" | "aside" | "nav" |
        "header" | "footer" | "figure" | "figcaption" | "div" | "span" | "p" |
        "a" | "button" | "label" | "ul" | "ol" | "li")
        || super::valid_custom_element_name(name)
}

pub(super) fn shape_element(name: &str) -> bool {
    matches!(name, "svg" | "defs" | "g" | "mask" | "clippath" | "lineargradient" | "radialgradient" | "stop" | "symbol" | "use"
        | "circle" | "ellipse" | "line" | "path" | "polygon" | "polyline" | "rect")
}

pub(super) fn supported_tag(tag: &Tag) -> bool {
    // A balanced SVG shape belongs to the SVG namespace even when an
    // attribute has no rendering support. Do not use the fragment transfer
    // allowlist as evidence about the initial document's namespace.
    tag.clean_syntax && shape_element(&tag.name)
        && (!matches!(tag.name.as_str(),"use"|"lineargradient"|"radialgradient")
            || tag.attrs.iter().all(|(key,value)|!matches!(key.as_str(),"href"|"xlink:href")||crate::svg_reference::fragment(value).is_some()))
}

fn passive_presentation_attribute(name:&str)->bool{
    // Values remain real attribute data. URI-bearing presentation attributes
    // use separate admission below; this does not expose CSSOM/rendering APIs.
    matches!(name,"alignment-baseline"|"baseline-shift"|"color"|"color-interpolation"|"color-interpolation-filters"|
        "direction"|"dominant-baseline"|"flood-color"|"flood-opacity"|"font-family"|"font-size"|"font-size-adjust"|
        "font-stretch"|"font-style"|"font-variant"|"font-weight"|"glyph-orientation-vertical"|"image-rendering"|
        "letter-spacing"|"lighting-color"|"overflow"|"paint-order"|"pointer-events"|"shape-rendering"|
        "stroke-dasharray"|"stroke-dashoffset"|"stroke-linecap"|"stroke-linejoin"|"stroke-miterlimit"|
        "text-anchor"|"text-decoration"|"text-overflow"|"text-rendering"|"transform-origin"|"unicode-bidi"|
        "vector-effect"|"white-space"|"word-spacing"|"writing-mode")
}

pub(super) fn template_refusal(tag: &Tag) -> Option<(&'static str, &str)> {
    if !shape_element(&tag.name){return Some(("element",&tag.name));}
    if tag.duplicate_attributes{return Some(("duplicate attributes on element",&tag.name));}
    if !tag.clean_syntax{return Some(("malformed element",&tag.name));}
    tag.attrs.iter().find_map(|(key,value)|(!passive_attribute(&tag.name,key,value)).then_some(("attribute",key.as_str())))
}

fn passive_attribute(tag:&str,key:&str,value:&str)->bool{match key {
        "id" | "class" | "style" | "role" | "focusable" | "tabindex" | "viewbox" | "preserveaspectratio" | "pathlength" |
        "maskunits" | "maskcontentunits" | "mask-type" |
        "clippathunits" | "gradientunits" | "gradienttransform" | "spreadmethod" | "offset" | "stop-color" | "stop-opacity" |
        "opacity" | "fill-opacity" | "stroke-opacity" | "fill-rule" | "clip-rule" | "transform" | "display" | "visibility" | "fx" | "fy" | "fr" |
        "width" | "height" | "x" | "y" | "cx" | "cy" | "r" | "rx" | "ry" |
        "x1" | "y1" | "x2" | "y2" | "d" | "points" | "stroke-width" => true,
        "href" | "xlink:href" => matches!(tag,"use"|"lineargradient"|"radialgradient")&&crate::svg_reference::fragment(value).is_some(),
        "mask" | "clip-path" => crate::svg_reference::effect(value).is_ok(),
        "fill" | "stroke" => crate::svg_reference::effect(value).is_ok()||value.strip_prefix('#').map_or_else(
            || !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphabetic()),
            |digits| matches!(digits.len(), 3 | 4 | 6 | 8) && digits.bytes().all(|b| b.is_ascii_hexdigit())),
        "xmlns" => Some(value) == Namespace::Svg.element_uri(),
        "xmlns:xlink" => value=="http://www.w3.org/1999/xlink",
        // These ordinary null-namespace data/accessibility names do not bind
        // event handlers, external resources or graphics/focus APIs.
        _ if key.starts_with("aria-") || key.starts_with("data-") => true,
        _ if passive_presentation_attribute(key) => true,
        _ => false,
    }}

pub(super) fn dom_name(name:&str)->bool{matches!(name,"svg"|"defs"|"g"|"mask"|"clipPath"|"linearGradient"|"radialGradient"|"stop"|"symbol"|"use"|"circle"|"ellipse"|"line"|"path"|"polygon"|"polyline"|"rect")}
pub(super) fn dom_attribute(tag:&str,name:&super::AttributeName,value:&str)->bool{
    if !dom_name(tag){return false;}
    if let Some(namespace)=&name.namespace{return match namespace.uri.as_str(){
        "http://www.w3.org/1999/xlink"=>namespace.local=="href"&&namespace.prefix.as_deref()==Some("xlink")&&matches!(tag,"use"|"linearGradient"|"radialGradient")&&crate::svg_reference::fragment(value).is_some(),
        "http://www.w3.org/2000/xmlns/"=>(namespace.local=="xmlns"&&namespace.prefix.is_none()&&value=="http://www.w3.org/2000/svg")||(namespace.local=="xlink"&&namespace.prefix.as_deref()==Some("xmlns")&&value=="http://www.w3.org/1999/xlink"),
        _=>false,
    };}
    let key=match name.qualified.as_str(){"viewBox"=>"viewbox","preserveAspectRatio"=>"preserveaspectratio","pathLength"=>"pathlength","maskUnits"=>"maskunits","maskContentUnits"=>"maskcontentunits","clipPathUnits"=>"clippathunits","gradientUnits"=>"gradientunits","gradientTransform"=>"gradienttransform","spreadMethod"=>"spreadmethod",name=>name};
    if key.contains(':'){return false;}
    let tag=match tag{"clipPath"=>"clippath","linearGradient"=>"lineargradient","radialGradient"=>"radialgradient",name=>name};passive_attribute(tag,key,value)
}

pub(super) fn publish(nodes: &mut [Node]) {
    // Arena allocation follows source order. Each completed island owns a
    // disjoint slice, including its elements and non-rendering text/comment nodes.
    for node in nodes {
        if let NodeKind::Element { tag, attrs, .. } = &mut node.kind {
            node.namespace = Namespace::Svg;
            // Same-length ASCII tag adjustment retains the admitted backing.
            let upper=match tag.as_str(){"clippath"=>Some(4),"lineargradient"|"radialgradient"=>Some(6),_=>None};
            if let Some(index)=upper{tag.get_mut(index..index+1).expect("ASCII SVG tag adjustment").make_ascii_uppercase();}
            for attr in attrs {
                // The shape subset supports these adjusted, case-sensitive
                // attribute names without implementing their graphics APIs.
                let adjusted = match attr.name.qualified.as_str() {
                    "viewbox" => Some("viewBox"),
                    "preserveaspectratio" => Some("preserveAspectRatio"),
                    "pathlength" => Some("pathLength"),
                    "maskunits" => Some("maskUnits"),
                    "maskcontentunits" => Some("maskContentUnits"),
                    "clippathunits" => Some("clipPathUnits"),
                    "gradientunits" => Some("gradientUnits"),
                    "gradienttransform" => Some("gradientTransform"),
                    "spreadmethod" => Some("spreadMethod"),
                    _ => None,
                };
                if let Some(name) = adjusted { attr.name.qualified = name.into(); }
                if attr.name.html_is("xmlns") {
                    attr.name.namespace = Some(AttributeNamespace {
                        uri: "http://www.w3.org/2000/xmlns/".into(), prefix: None, local: "xmlns".into(),
                    });
                }else if attr.name.html_is("xmlns:xlink"){
                    attr.name.namespace=Some(AttributeNamespace{uri:"http://www.w3.org/2000/xmlns/".into(),prefix:Some("xmlns".into()),local:"xlink".into()});
                }else if attr.name.html_is("xlink:href"){
                    attr.name.namespace=Some(AttributeNamespace{uri:"http://www.w3.org/1999/xlink".into(),prefix:Some("xlink".into()),local:"href".into()});
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::{parse, parse_fragment, Document, MAX_DEPTH, MAX_NODES};

    fn node<'a>(doc: &'a Document, id: &str) -> &'a Node {
        &doc.nodes[(0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).expect(id)]
    }

    #[test]
    fn svg_namespace_balanced_islands_publish_only_scoped_elements() {
        let doc = parse("<html><body><circle id=outside></circle><svg id=root xmlns='http://www.w3.org/2000/svg'>\n\
            <g id=group><circle id='a'/><rect id='b'/><svg id=nested><path id=p d='M1 2'/></svg></g><defs><mask id=mask maskUnits=userSpaceOnUse><rect id='mask-rect'/></mask><clipPath id=clip clipPathUnits=userSpaceOnUse><path id='clip-path'/></clipPath></defs><use id='linked-use' xlink:href='#group'/><use id='empty-use'/></svg>\
            <svg id='empty'/><div id=after xmlns='http://www.w3.org/2000/svg'></div></body></html>");
        for id in ["root", "group", "a", "b", "nested", "p", "empty", "mask", "mask-rect", "clip", "clip-path", "linked-use", "empty-use"] {
            assert_eq!(node(&doc, id).namespace, Namespace::Svg, "{id}");
        }
        for id in ["outside", "after"] { assert_eq!(node(&doc, id).namespace, Namespace::Html); }
        let group = node(&doc, "group");
        assert_eq!(group.children.len(), 3, "self-closing shapes must be siblings");
        let NodeKind::Element {attrs,..} = &node(&doc, "root").kind else { panic!("SVG root") };
        assert!(attrs.iter().any(|a| a.name.matches(Some("http://www.w3.org/2000/xmlns/"), "xmlns")));
        let NodeKind::Element{attrs,..}=&node(&doc,"mask").kind else{panic!("SVG mask")};
        assert!(attrs.iter().any(|attr|attr.name.html_is("maskUnits")&&attr.value=="userSpaceOnUse"));
        let NodeKind::Element{tag,attrs,..}=&node(&doc,"clip").kind else{panic!("SVG clipPath")};
        assert_eq!(tag,"clipPath");assert!(attrs.iter().any(|attr|attr.name.html_is("clipPathUnits")&&attr.value=="userSpaceOnUse"));
        let NodeKind::Element{attrs,..}=&node(&doc,"linked-use").kind else{panic!("SVG use")};
        assert!(attrs.iter().any(|attr|attr.name.matches(Some("http://www.w3.org/1999/xlink"),"href")&&attr.value=="#group"));
        assert!(doc.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Text(_))).all(|n| n.namespace == Namespace::None));
        assert_eq!(Namespace::Svg.element_uri(), Some("http://www.w3.org/2000/svg"));
        assert!(Namespace::Svg.is_foreign() && Namespace::UnknownForeign.is_foreign());
        assert!(!Namespace::Html.is_foreign() && !Namespace::None.is_foreign());
    }

    #[test]
    fn svg_namespace_uncertain_content_cannot_publish_an_earlier_valid_prefix() {
        for bad in ["<foreignObject></foreignObject>", "<use href='/guard'></use>", "<use xlink:href='/guard'></use>", "<linearGradient href='/guard'></linearGradient>", "<script></script>", "<math></math>",
            "<g><circle></g>",
            "<g id='a'id='b'></g>", "<g//>", "<g></g/>", "</missing>", "<![CDATA[x]]>"] {
            let doc = parse(&format!("<body><svg id=root><circle id='first'/>{bad}</svg><p id=next></p></body>"));
            assert_eq!(node(&doc, "root").namespace, Namespace::UnknownForeign, "{bad}");
            assert_eq!(node(&doc, "first").namespace, Namespace::UnknownForeign, "{bad}");
            assert_eq!(node(&doc, "next").namespace, Namespace::Html, "{bad}");
        }
        for markup in ["<svg id=root><g/>", "<svg id=root><g", "<svg id=root><circle id=unquoted/></svg>",
                       "<select><svg id=root></svg></select>", "<template><div><svg id=root></svg></div></template>",
                       "<math><svg id=root></svg></math>"] {
            assert_eq!(node(&parse(markup), "root").namespace, Namespace::UnknownForeign, "{markup}");
        }
        let doc = parse("<select><option></option></select><svg id='s'/>");
        assert_eq!(node(&doc, "s").namespace, Namespace::Svg);
    }

    #[test]
    fn svg_namespace_node_and_stack_limits_preserve_unknown_prefix_and_fragment_refusal() {
        let depth = format!("<svg id=root>{}{}", "<g>".repeat(MAX_DEPTH), "</g>".repeat(MAX_DEPTH));
        let doc = parse(&depth);
        assert_eq!(doc.nodes.len(), MAX_DEPTH);
        assert_eq!(node(&doc, "root").namespace, Namespace::UnknownForeign);
        let many = format!("<svg id=root>{}</svg>", "<circle/>".repeat(MAX_NODES));
        let doc = parse(&many);
        assert_eq!(doc.nodes.len(), MAX_NODES);
        assert!(doc.nodes[1..].iter().all(|n| n.namespace == Namespace::UnknownForeign));
        assert!(parse_fragment("<svg><circle/></svg>", "div", true).is_err());
        assert!(parse_fragment("<circle/>", "svg", false).is_err());
    }
}
