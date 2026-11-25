use super::*;

#[test]
fn scoped_end_shared_ordinary_predicate_uses_exact_html_scope_boundaries() {
    let mut doc = parse("<span></span>");
    let node = first(&doc, "span");
    for (name, expected) in [
        ("applet", true), ("caption", true), ("html", true), ("table", true),
        ("td", true), ("th", true), ("marquee", true), ("object", true),
        ("select", true), ("template", true),
        ("button", false), ("ul", false), ("ol", false), ("section", false),
        ("li", false), ("dd", false), ("dt", false), ("h1", false),
        ("div", false), ("p", false), ("dialog", false), ("span", false),
    ] {
        if let NodeKind::Element { tag, .. } = &mut doc.nodes[node].kind { *tag = name.to_owned(); }
        assert_eq!(scope_boundary(&doc, node), expected, "{name}");
    }
}

#[test]
fn scoped_end_shared_ordinary_predicate_keeps_conservative_foreign_and_root_boundaries() {
    let mut doc = parse("<span></span>");
    let node = first(&doc, "span");
    for namespace in [Namespace::None, Namespace::Html, Namespace::Svg, Namespace::UnknownForeign, Namespace::Other] {
        doc.nodes[node].namespace = namespace;
        assert_eq!(scope_boundary(&doc, node), namespace != Namespace::Html);
    }
    assert!(scope_boundary(&doc, 0));
}

#[test]
fn fragment_formatting_stages_repaired_tree_in_preorder_and_preserves_nonce_flags() {
    let doc=parse_fragment("<b>before<div><script nonce=a nonce=b>0</script>inside</b>outside</div><script nonce=c>1</script>","div",true).unwrap();
    assert_eq!(kids(&doc, 0), ["b", "div", "script"]);
    let block = doc.nodes[0].children[1];
    assert_eq!(kids(&doc, block), ["b", "#outside"]);
    let replacement = doc.nodes[block].children[0];
    assert_eq!(kids(&doc, replacement), ["script", "#inside"]);
    let script = doc.nodes[replacement].children[0];
    assert_eq!(doc.nonce_unsafe_nodes.len(), 1);
    assert!(doc.nonce_unsafe_nodes.contains(&script));
    assert!(!doc.nonce_unsafe_nodes.contains(&doc.nodes[0].children[2]));
    let mut stack = vec![0];
    let mut order = Vec::new();
    while let Some(n) = stack.pop() {
        order.push(n);
        stack.extend(doc.nodes[n].children.iter().rev().copied());
    }
    assert!(order.into_iter().eq(0..doc.nodes.len()));
}

#[test]
fn fragment_formatting_reconstructs_after_implied_end_and_keeps_nested_template_scope() {
    let doc = parse_fragment("<p><b>one<div>two</div>three", "div", true).unwrap();
    assert_eq!(kids(&doc, 0), ["p", "div", "b"]);
    let div = doc.nodes[0].children[1];
    assert_eq!(kids(&doc, div), ["b"]);
    assert_eq!(kids(&doc, doc.nodes[div].children[0]), ["#two"]);
    let doc = parse_fragment(
        "<p><b>outer<template><i>held</b></template>end",
        "template",
        true,
    )
    .unwrap();
    let outer = first(&doc, "b");
    assert_eq!(kids(&doc, outer), ["#outer", "template", "#end"]);
    assert!(
        parse_fragment(
            "<b><svg><foreignObject></foreignObject></svg>",
            "template",
            true
        )
        .is_err()
    );
}

#[test]
fn formatting_adoption_inner_loop_retains_original_chain_and_moves_three_replacements() {
    let doc = parse_loaded_document("<div><a><b><u><i><code><div></a>", &|| Ok(())).unwrap();
    let outer = first(&doc, "div");
    assert_eq!(kids(&doc, outer), ["a", "u"]);
    let mut old = doc.nodes[outer].children[0];
    for tag in ["b", "u", "i", "code"] {
        assert_eq!(kids(&doc, old), [tag]);
        old = doc.nodes[old].children[0];
    }
    assert!(doc.nodes[old].children.is_empty());
    let mut new = doc.nodes[outer].children[1];
    for tag in ["i", "code", "div", "a"] {
        assert_eq!(kids(&doc, new), [tag]);
        new = doc.nodes[new].children[0];
    }
    assert!(doc.nodes[new].children.is_empty());
    let mut parents = vec![0; doc.nodes.len()];
    for n in &doc.nodes {
        for &i in &n.children {
            parents[i] += 1;
        }
    }
    assert_eq!(parents[0], 0);
    assert!(parents[1..].iter().all(|&n| n == 1));
}

#[test]
fn formatting_adoption_reparents_blocks_and_preserves_original_native_anchor_attributes() {
    let source = "<a href=#end data-label=old>1<p>2</a>3</p>";
    for doc in [
        parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 10, &|| Ok(())).unwrap().0,
    ] {
        let body = first(&doc, "body");
        assert_eq!(kids(&doc, body), ["a", "p"]);
        let original = doc.nodes[body].children[0];
        let p = doc.nodes[body].children[1];
        let replacement = doc.nodes[p].children[0];
        assert_ne!(original, replacement);
        assert_eq!(kids(&doc, original), ["#1"]);
        assert_eq!(kids(&doc, p), ["a", "#3"]);
        assert_eq!(kids(&doc, replacement), ["#2"]);
        assert_eq!(doc.attr(replacement, "href"), Some("#end"));
        assert_eq!(doc.attr(replacement, "data-label"), Some("old"));
    }
    assert!(
        parse_inert_html(source, 8, &|| Ok(()))
            .unwrap_err()
            .contains("node limit")
    );
    let calls = std::cell::Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 4 {
            Err("cancel adoption".into())
        } else {
            Ok(())
        }
    };
    assert!(
        parse_loaded_document(source, &check)
            .unwrap_err()
            .contains("cancel adoption")
    );
}

#[test]
fn formatting_adoption_nested_outer_loops_and_fostered_table_edges_are_in_dom_order() {
    let doc = parse_loaded_document("<a>1<div>2<div>3</a>4</div>5</div>", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["a", "div"]);
    let outer = first(&doc, "div");
    assert_eq!(kids(&doc, outer), ["a", "div", "#5"]);
    let inner = doc.nodes[outer].children[1];
    assert_eq!(kids(&doc, inner), ["a", "#4"]);
    assert_eq!(kids(&doc, doc.nodes[outer].children[0]), ["#2"]);
    assert_eq!(kids(&doc, doc.nodes[inner].children[0]), ["#3"]);
    let doc = parse_loaded_document("<table><a>1<p>2</a>3</p></table>", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["a", "p", "table"]);
    assert_eq!(kids(&doc, first(&doc, "p")), ["a", "#3"]);
    assert!(doc.nodes[first(&doc, "table")].children.is_empty());
}

fn kids(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent]
        .children
        .iter()
        .map(|&n| match &doc.nodes[n].kind {
            NodeKind::Element { tag, .. } => tag.clone(),
            NodeKind::Text(data) => format!("#{data}"),
            NodeKind::Comment(data) => format!("!{data}"),
            _ => panic!("unexpected child"),
        })
        .collect()
}
fn first(doc: &Document, name: &str) -> usize {
    doc.nodes
        .iter()
        .position(|n| matches!(&n.kind,NodeKind::Element{tag,..} if tag==name))
        .unwrap()
}

#[test]
fn active_formatting_reconstructs_original_attributes_and_distinct_native_siblings() {
    let source = "<p><b id=marker data-a=original><i>A</p><!--leave-->B</b>C</i>D";
    for doc in [
        parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100, &|| Ok(())).unwrap().0,
    ] {
        let body = first(&doc, "body");
        assert_eq!(kids(&doc, body), ["p", "!leave", "b", "i", "#D"]);
        let old = first(&doc, "b");
        let new = doc.nodes[body].children[2];
        assert_ne!(old, new);
        assert_eq!(doc.attr(new, "data-a"), Some("original"));
        assert_eq!(doc.attr(new, "id"), Some("marker"));
        assert_eq!(kids(&doc, old), ["i"]);
        assert_eq!(kids(&doc, new), ["i"]);
        assert_eq!(kids(&doc, doc.nodes[old].children[0]), ["#A"]);
        assert_eq!(kids(&doc, doc.nodes[new].children[0]), ["#B"]);
        assert_eq!(kids(&doc, doc.nodes[body].children[3]), ["#C"]);
    }
}
#[test]
fn active_formatting_isolated_markers_and_foster_placement() {
    let doc =
        parse_loaded_document("<table><b>A<tr><td>X<i>Y<td>Z</table>after", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["b", "table", "b"]);
    assert_eq!(kids(&doc, doc.nodes[body].children[0]), ["#A"]);
    assert_eq!(kids(&doc, doc.nodes[body].children[2]), ["#after"]);
    let row = first(&doc, "tr");
    assert_eq!(kids(&doc, row), ["td", "td"]);
    assert_eq!(kids(&doc, doc.nodes[row].children[0]), ["#X", "i"]);
    assert_eq!(kids(&doc, doc.nodes[row].children[1]), ["#Z"]);
    let doc = parse_loaded_document("<table><b>A<tr>B<!--here--></table>", &|| Ok(())).unwrap();
    assert_eq!(kids(&doc, first(&doc, "body")), ["b", "b", "table"]);
    let b = doc.nodes[first(&doc, "body")].children[1];
    assert_eq!(kids(&doc, b), ["#B", "!here"]);
}
#[test]
fn active_formatting_limits_equivalent_entries_with_unordered_attributes() {
    let doc = parse_loaded_document(
        "<p><b x=1 y=2><b y=2 x=1><b x=1 y=2><b y=2 x=1><b x=1 y=3>A</p>B",
        &|| Ok(()),
    )
    .unwrap();
    let mut chain = doc.nodes[first(&doc, "body")].children[1];
    let mut values = Vec::new();
    while let NodeKind::Element { .. } = &doc.nodes[chain].kind {
        values.push(doc.attr(chain, "y").unwrap());
        chain = doc.nodes[chain].children[0];
    }
    assert_eq!(values, ["2", "2", "2", "3"]);
    assert!(matches!(&doc.nodes[chain].kind,NodeKind::Text(t) if t=="B"));
}
#[test]
fn active_formatting_charges_clones_and_preserves_checked_fragment_limits() {
    let source = "<p><b>X</p>Y";
    let doc = parse_inert_html(source, 9, &|| Ok(())).unwrap().0;
    assert_eq!(doc.nodes.len(), 9);
    assert!(
        parse_inert_html(source, 8, &|| Ok(()))
            .unwrap_err()
            .contains("node limit")
    );
    let at_eof = parse_fragment("<b><i></b>", "div", false).unwrap();
    assert_eq!(kids(&at_eof, 0), ["b"]);
    assert_eq!(kids(&at_eof, at_eof.nodes[0].children[0]), ["i"]);
    let fragment = parse_fragment("<b><i></b>X", "div", false).unwrap();
    assert_eq!(kids(&fragment, 0), ["b", "i"]);
    assert_eq!(kids(&fragment, fragment.nodes[0].children[1]), ["#X"]);
    assert!(parse_fragment("<table><tr><td>X", "div", false).is_err());
    let mut doc = parse_loaded_document("<p><b data-x=1234>A", &|| Ok(())).unwrap();
    let b = first(&doc, "b");
    let body = first(&doc, "body");
    let mut fmt = Formatting::default();
    fmt.opened(&doc, &[0, body, b], b).unwrap();
    fmt.copied = crate::javascript::limits::BROWSER.host.retained_bytes;
    let before = doc.nodes.len();
    let mut stack = vec![0, body];
    assert!(
        fmt.reconstruct(&mut doc, &mut stack, 100, None)
            .unwrap_err()
            .contains("storage limit")
    );
    assert_eq!(doc.nodes.len(), before);
    assert_eq!(stack, [0, body]);
}

#[test]
fn active_formatting_marker_survives_unrelated_table_pop_until_explicit_clear() {
    let doc = parse_loaded_document("<a><table><marquee></table><a>", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["a"]);
    let outer = doc.nodes[body].children[0];
    assert_eq!(kids(&doc, outer), ["marquee", "table", "a"]);
    let doc = parse_loaded_document("<a><marquee><i>held</marquee><a>outside", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["a", "a"]);
    assert_eq!(kids(&doc, doc.nodes[body].children[1]), ["#outside"]);
}

#[test]
fn active_formatting_nested_nobr_uses_open_element_scope_after_reconstruction() {
    let doc = parse_loaded_document("<nobr><table><marquee></table><nobr>", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["nobr", "nobr"]);
    assert_eq!(
        kids(&doc, doc.nodes[body].children[0]),
        ["marquee", "table"]
    );
    let doc = parse_loaded_document("<p><nobr>old</p><nobr>new", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["p", "nobr", "nobr"]);
    assert_eq!(
        kids(&doc, doc.nodes[body].children[1]),
        Vec::<String>::new()
    );
    assert_eq!(kids(&doc, doc.nodes[body].children[2]), ["#new"]);
}

#[test]
fn formatting_adoption_select_scope_keeps_the_owning_formatting_element_open() {
    let source = "<font><select><option>a</option></font><option>b</select>after</font>end";
    for doc in [
        parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 32, &|| Ok(())).unwrap().0,
    ] {
        let body = first(&doc, "body");
        assert_eq!(kids(&doc, body), ["font", "#end"]);
        assert_eq!(kids(&doc, first(&doc, "font")), ["select", "#after"]);
        let select = first(&doc, "select");
        assert_eq!(kids(&doc, select), ["option", "option"]);
        assert_eq!(kids(&doc, doc.nodes[select].children[0]), ["#a"]);
        assert_eq!(kids(&doc, doc.nodes[select].children[1]), ["#b"]);
    }
}
