//! Heading start tags close a paragraph, then only a current HTML heading.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

fn id(doc: &Document, value: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}

fn children(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent].children.iter().map(|&node| match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag.clone(),
        NodeKind::Text(value) => format!("#{value}"),
        _ => panic!("unexpected child kind"),
    }).collect()
}

fn text(doc: &Document) -> String {
    doc.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(value) => Some(value.render_projection()), _ => None,
    }).collect()
}

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 2000, &|| Ok(())).unwrap().0]
}

fn assert_siblings(doc: &Document, parent: usize, old: &str, new: &str) {
    assert_eq!(doc.nodes[parent].children, [id(doc, "old"), id(doc, "new"), id(doc, "tail")], "{old}/{new}");
    assert_eq!(children(doc, id(doc, "old")), ["#A"]);
    assert_eq!(children(doc, id(doc, "new")), ["#B"]);
    assert_eq!(children(doc, id(doc, "tail")), ["#C"]);
    assert_eq!(text(doc), "ABC");
}

#[test]
fn heading_starts_close_each_current_heading_rank_in_loaded_and_inert_documents() {
    for old in HEADINGS { for new in HEADINGS {
        let source = format!("<!doctype html><div id=host><{old} id=old>A<{new} id=new>B</{new}><p id=tail>C</p></div>");
        for doc in documents(&source) {
            assert_siblings(&doc, id(&doc, "host"), old, new);
            assert_eq!(doc.mode, DocumentMode::NoQuirks);
        }
    }}
}

#[test]
fn heading_starts_use_real_checked_fragment_contexts_for_every_rank_pair() {
    for old in HEADINGS { for new in HEADINGS {
        let source = format!("<{old} id=old>A<{new} id=new>B</{new}><p id=tail>C</p>");
        for context in ["div", "template"] { for scripting in [false, true] {
            let doc = parse_fragment(&source, context, scripting).unwrap();
            assert_siblings(&doc, 0, old, new);
            assert_eq!(doc.scripting_enabled, scripting);
        }}
    }}
}

#[test]
fn heading_starts_close_a_paragraph_before_inspecting_the_new_current_node() {
    let source = "<div id=host><h1 id=old><p id=paragraph>A<h2 id=new>B</h2><p id=tail>C</p></div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "old"), id(&doc, "new"), id(&doc, "tail")]);
        assert_eq!(doc.nodes[id(&doc, "old")].children, [id(&doc, "paragraph")]);
        assert_eq!(children(&doc, id(&doc, "paragraph")), ["#A"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
        assert_eq!(children(&doc, id(&doc, "tail")), ["#C"]);
    }
}

#[test]
fn heading_starts_preserve_older_headings_behind_a_current_nonheading() {
    for middle in ["span", "b", "div", "object"] {
        let source = format!("<div id=host><h1 id=old><{middle} id=middle>A<h2 id=new>B</h2>C</{middle}>D</h1><p id=tail>E</p></div>");
        for doc in documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
            assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "old"), id(&doc, "tail")], "{middle}");
            assert_eq!(children(&doc, id(&doc, "old")), [middle, "#D"], "{middle}");
            assert_eq!(children(&doc, id(&doc, "middle")), ["#A", "h2", "#C"], "{middle}");
            assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
            assert_eq!(text(&doc), "ABCDE");
        }
    }
}

#[test]
fn heading_starts_keep_an_older_heading_when_paragraph_closure_exposes_a_barrier() {
    let source = "<div id=host><h1 id=old><object id=boundary><p id=paragraph>A<h2 id=new>B</h2>C</object>D</h1></div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", false).unwrap()]) {
        assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "old")]);
        assert_eq!(children(&doc, id(&doc, "old")), ["object", "#D"]);
        assert_eq!(children(&doc, id(&doc, "boundary")), ["p", "h2", "#C"]);
        assert_eq!(children(&doc, id(&doc, "paragraph")), ["#A"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
    }
}

#[test]
fn heading_starts_preserve_active_formatting_copies_after_both_implicit_closures() {
    let source = "<div id=host><h1 id=old><p id=paragraph><b id=marker data-label=kept>A<h2 id=new>B</h2>C</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let host = id(&doc, "host"); let paragraph = id(&doc, "paragraph"); let new = id(&doc, "new");
        assert_eq!(children(&doc, host), ["h1", "h2", "b"]);
        assert_eq!(doc.nodes[id(&doc, "old")].children, [paragraph]);
        let copies = [doc.nodes[paragraph].children[0], doc.nodes[new].children[0], doc.nodes[host].children[2]];
        assert_ne!(copies[0], copies[1]); assert_ne!(copies[1], copies[2]); assert_ne!(copies[0], copies[2]);
        for (node, value) in copies.into_iter().zip(["#A", "#B", "#C"]) {
            assert_eq!(doc.attr(node, "id"), Some("marker"));
            assert_eq!(doc.attr(node, "data-label"), Some("kept"));
            assert_eq!(children(&doc, node), [value]);
        }
        let mut parents = vec![0; doc.nodes.len()];
        for node in &doc.nodes { for &child in &node.children { parents[child] += 1; } }
        assert_eq!(parents[0], 0); assert!(parents[1..].iter().all(|&count| count == 1));
    }
}

#[test]
fn heading_starts_keep_template_scope_and_never_pop_a_fragment_context_root() {
    let source = "<div id=host><h1 id=outer><template id=boundary><h2 id=old>A<h3 id=new>B</h3>C</template>D</h1></div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "template", true).unwrap()]) {
        assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "outer")]);
        assert_eq!(children(&doc, id(&doc, "outer")), ["template", "#D"]);
        assert_eq!(children(&doc, id(&doc, "boundary")), ["h2", "h3", "#C"]);
        assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
    }
    for context in HEADINGS {
        let doc = parse_fragment("<h2 id=new>A</h2>B", context, true).unwrap();
        assert_eq!(children(&doc, 0), ["h2", "#B"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["#A"]);
    }
}

#[test]
fn heading_starts_pop_exactly_one_native_html_current_heading() {
    for namespace in [Namespace::Html, Namespace::Svg, Namespace::UnknownForeign, Namespace::Other] {
        let mut doc = parse("<h1 id=older></h1><h3 id=current></h3>");
        let older = id(&doc, "older"); let current = id(&doc, "current");
        doc.nodes[current].namespace = namespace;
        let mut stack = vec![0, older, current];
        close_implied(&mut stack, &doc, "h2");
        assert_eq!(stack, if namespace == Namespace::Html { vec![0, older] } else { vec![0, older, current] });
        for incoming in ["span", "h7", "x-h1"] {
            let mut stack = vec![0, older]; close_implied(&mut stack, &doc, incoming);
            assert_eq!(stack, [0, older]);
        }
    }
    let doc = parse_fragment("", "h1", true).unwrap();
    let mut stack = vec![0]; close_implied(&mut stack, &doc, "h2"); assert_eq!(stack, [0]);
}

#[test]
fn heading_starts_require_a_complete_token_and_keep_html_self_closing_recovery() {
    for source in ["<h1 id=old>A<h2", "<h1 id=old>A<h2 id='new", "<h1 id=old>A<h2 id=new"] {
        for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
            assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
            assert_eq!(doc.nodes.iter().filter(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "h2")).count(), 0);
        }
    }
    let source = "<div id=host><h1 id=old>A<h2 id='new'/>B</h2>C</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", false).unwrap()]) {
        assert_eq!(children(&doc, id(&doc, "host")), ["h1", "h2", "#C"]);
        assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
    }
}

#[test]
fn heading_starts_keep_rawtext_rcdata_and_other_checked_refusals() {
    for context in ["textarea", "title", "style", "script", "plaintext"] {
        let source = "<h1>&amp;<h2>";
        let doc = parse_fragment(source, context, true).unwrap();
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(text(&doc), if matches!(context, "textarea" | "title") { "<h1>&<h2>" } else { source });
    }
    for source in ["<p>A<listing>B", "<p>A<xmp>B</xmp>", "<h1>A</br>"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    for source in ["<select><h1>A", "<h1>A<table><tr></tr></table>", "<h1>A<svg></svg>"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    assert!(parse_fragment("<h1>A<h2>\0", "div", true).unwrap_err().contains("NULL handling"));
    assert!(parse_fragment("<svg><h1></h1></svg>", "template", true).is_err());
}

#[test]
fn heading_starts_keep_a_bounded_stack_for_a_long_run_of_real_siblings() {
    let sequence: String = (0..300).map(|n| format!("<h{}>A", n % 6 + 1)).collect();
    let source = format!("<div id=host>{sequence}</div>");
    for doc in documents(&source) {
        assert_eq!(doc.nodes.len(), 605);
        assert_eq!(doc.nodes[id(&doc, "host")].children.len(), 300);
        assert!(doc.nodes[id(&doc, "host")].children.iter().all(|&node| doc.nodes[node].namespace == Namespace::Html));
    }
    let doc = parse_fragment(&source, "div", true).unwrap();
    assert_eq!(doc.nodes.len(), 602);
    assert_eq!(doc.nodes[id(&doc, "host")].children.len(), 300);
    assert!(parse_inert_html(&source, 604, &|| Ok(())).unwrap_err().contains("node limit"));
}

#[test]
fn heading_starts_preserve_real_capacity_and_cancellation_without_a_prefix() {
    let source = "<div id=host><h1 id=old>A<h2 id=new>B</h2>C</div>";
    let doc = parse_inert_html(source, 10, &|| Ok(())).unwrap().0;
    assert_eq!(doc.nodes.len(), 10);
    assert_eq!(children(&doc, id(&doc, "host")), ["h1", "h2", "#C"]);
    assert!(parse_inert_html(source, 9, &|| Ok(())).unwrap_err().contains("node limit"));
    let source = "<h1>A".repeat(1024); let calls = std::cell::Cell::new(0);
    let error = parse_loaded_document(&source, &|| { calls.set(calls.get()+1); if calls.get()==8 { Err("heading scan cancelled".into()) } else { Ok(()) } }).unwrap_err();
    assert_eq!(error, "heading scan cancelled"); assert_eq!(calls.get(), 8);
    let doc = parse_fragment("<h1 id=old>A<h2 id=new>B</h2><p id=tail>C</p>", "div", false).unwrap();
    assert_siblings(&doc, 0, "h1", "h2");
}

#[test]
fn heading_starts_keep_real_resource_attributes_and_duplicate_nonce_evidence() {
    let source = "<div id=host><h1 id=old>A<h2 id=new><script id=s nonce=one NONCE=two></script><img id=i src='image.png'>B</h2>C</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let script = id(&doc, "s"); let image = id(&doc, "i"); let new = id(&doc, "new");
        assert_eq!(children(&doc, id(&doc, "host")), ["h1", "h2", "#C"]);
        assert!(doc.nodes[new].children.contains(&script)); assert!(doc.nodes[new].children.contains(&image));
        assert_eq!(doc.attr(script, "nonce"), Some("one"));
        assert_eq!(doc.nonce_unsafe_nodes.len(), 1); assert!(doc.nonce_unsafe_nodes.contains(&script));
        assert_eq!(doc.attr(image, "src"), Some("image.png")); assert_eq!(text(&doc), "ABC");
    }
}
