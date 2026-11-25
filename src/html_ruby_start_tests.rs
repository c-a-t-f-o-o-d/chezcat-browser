//! Original native ruby annotation start-token witnesses.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
//! https://html.spec.whatwg.org/multipage/parsing.html#generate-implied-end-tags
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100_000, &|| Ok(())).unwrap().0]
}

fn id(doc: &Document, value: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}

fn element(doc: &Document, name: &str) -> usize {
    doc.nodes.iter().position(|node| matches!(&node.kind,
        NodeKind::Element { tag, .. } if tag == name)).unwrap()
}

fn children(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent].children.iter().map(|&child| match &doc.nodes[child].kind {
        NodeKind::Element { tag, .. } => tag.clone(),
        NodeKind::Text(value) => format!("#{value}"),
        NodeKind::Comment(value) => format!("!{value}"),
        other => panic!("unexpected child {other:?}"),
    }).collect()
}

fn contents(doc: &Document, parent: usize) -> String {
    doc.nodes[parent].children.iter().map(|&child| tree(doc, child)).collect()
}

fn tree(doc: &Document, node: usize) -> String {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", contents(doc, node)),
        NodeKind::Text(value) => value.as_str().to_owned(),
        _ => panic!("unexpected node in original ruby tree"),
    }
}

fn unique_links(doc: &Document) {
    let mut counts = vec![0; doc.nodes.len()];
    for node in &doc.nodes { for &child in &node.children { counts[child] += 1; } }
    assert_eq!(counts[0], 0);
    assert!(counts[1..].iter().all(|&count| count == 1));
}

fn checked_trees(source: &str, expected: &str) {
    for doc in documents(source) {
        assert_eq!(tree(&doc, id(&doc, "host")), expected, "{source}");
        unique_links(&doc);
    }
    for context in ["div", "template"] { for scripting in [false, true] {
        let doc = parse_fragment(source, context, scripting).unwrap();
        assert_eq!(tree(&doc, id(&doc, "host")), expected, "{context}/{source}");
        assert_eq!(doc.scripting_enabled, scripting);
        unique_links(&doc);
    }}
}

#[test]
fn ruby_rb_and_rtc_starts_close_each_current_implied_end_category() {
    for incoming in ["rb", "rtc"] {
        for old in ["dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc"] {
            let source = format!("<ruby id=host><{old} id=old>A<{incoming} id=new>B");
            for doc in documents(&source) {
                assert_eq!(children(&doc, id(&doc, "host")), [old, incoming], "{old}/{incoming}");
                assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
                assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
                assert_ne!(id(&doc, "old"), id(&doc, "new"));
                unique_links(&doc);
            }
        }
    }
}

#[test]
fn ruby_rp_and_rt_starts_close_current_implied_categories_except_rtc() {
    for incoming in ["rp", "rt"] {
        for old in ["dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt"] {
            let source = format!("<ruby id=host><{old} id=old>A<{incoming} id=new>B");
            for doc in documents(&source) {
                assert_eq!(children(&doc, id(&doc, "host")), [old, incoming], "{old}/{incoming}");
                assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
                assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
                unique_links(&doc);
            }
        }
    }
}

#[test]
fn ruby_rp_and_rt_retain_rtc_and_close_only_its_current_annotation() {
    for incoming in ["rp", "rt"] {
        checked_trees(&format!("<ruby id=host><rtc>A<{incoming}>B"),
            &format!("<ruby><rtc>A<{incoming}>B</{incoming}></rtc></ruby>"));
        for old in ["rp", "rt"] {
            checked_trees(&format!("<ruby id=host><rtc><{old}>A<{incoming}>B"),
                &format!("<ruby><rtc><{old}>A</{old}><{incoming}>B</{incoming}></rtc></ruby>"));
        }
    }
}

#[test]
fn ruby_starts_pop_a_current_implied_chain_and_stop_at_the_rtc_exception() {
    for incoming in ["rb", "rtc", "rp", "rt"] {
        for doc in documents(&format!("<ruby id=host><dd><li><option>A<{incoming}>B")) {
            assert_eq!(tree(&doc, id(&doc, "host")),
                format!("<ruby><dd><li><option>A</option></li></dd><{incoming}>B</{incoming}></ruby>"));
            unique_links(&doc);
        }
        let expected = if matches!(incoming, "rp" | "rt") {
            format!("<ruby><rtc><p>A</p><{incoming}>B</{incoming}></rtc></ruby>")
        } else {
            format!("<ruby><rtc><p>A</p></rtc><{incoming}>B</{incoming}></ruby>")
        };
        checked_trees(&format!("<ruby id=host><rtc><p>A<{incoming}>B"), &expected);
    }
}

#[test]
fn ruby_starts_do_not_close_implied_elements_without_a_ruby_in_scope() {
    for incoming in ["rb", "rtc", "rp", "rt"] {
        for old in ["p", "rb", "rtc", "rp", "rt"] {
            checked_trees(&format!("<div id=host><{old}>A<{incoming}>B"),
                &format!("<div><{old}>A<{incoming}>B</{incoming}></{old}></div>"));
        }
    }
}

#[test]
fn ruby_starts_stop_at_a_nonimplied_current_node_even_when_ruby_is_in_scope() {
    for incoming in ["rb", "rtc", "rp", "rt"] {
        for blocker in ["span", "b", "section", "button", "dialog"] {
            checked_trees(&format!("<ruby id=host><rt>A<{blocker}>B<{incoming}>C"),
                &format!("<ruby><rt>A<{blocker}>B<{incoming}>C</{incoming}></{blocker}></rt></ruby>"));
        }
    }
}

#[test]
fn ruby_starts_use_the_nearest_nested_ruby_before_its_outer_annotation() {
    checked_trees("<ruby id=host><rb>A<ruby><rt>B<rb>C</ruby><rt>D",
        "<ruby><rb>A<ruby><rt>B</rt><rb>C</rb></ruby></rb><rt>D</rt></ruby>");
}

#[test]
fn ruby_lookup_respects_ordinary_scope_barriers_before_any_popping() {
    for boundary in ["object", "applet", "marquee", "select", "template"] {
        for incoming in ["rb", "rtc", "rp", "rt"] {
            let source = format!("<ruby id=host><{boundary} id=gate><p id=old>A<{incoming} id=new>B");
            for doc in documents(&source) {
                assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "gate")]);
                assert_eq!(doc.nodes[id(&doc, "gate")].children, [id(&doc, "old")]);
                assert_eq!(children(&doc, id(&doc, "old")), ["#A", incoming], "{boundary}/{incoming}");
                assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
            }
        }
    }
}

#[test]
fn ruby_lookup_crosses_button_and_ordinary_special_nodes_without_popping_them() {
    for boundary in ["button", "div", "section", "ul"] {
        for incoming in ["rb", "rtc", "rp", "rt"] {
            checked_trees(&format!("<ruby id=host><{boundary}><p>A<{incoming}>B"),
                &format!("<ruby><{boundary}><p>A</p><{incoming}>B</{incoming}></{boundary}></ruby>"));
        }
    }
}

#[test]
fn ruby_lookup_finds_an_inner_ruby_before_a_template_barrier() {
    let source = "<ruby id=host><template id=gate><ruby id=inner><rt id=old>A<rb id=new>B</template>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "template", true).unwrap()]) {
        assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "gate")]);
        assert_eq!(doc.nodes[id(&doc, "gate")].children, [id(&doc, "inner")]);
        assert_eq!(children(&doc, id(&doc, "inner")), ["rt", "rb"]);
        assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
        unique_links(&doc);
    }
}

#[test]
fn ruby_annotation_changes_preserve_fostered_ruby_ownership_and_table_order() {
    let source = "<body><table id=table><ruby id=host><rb>A<rt>B</ruby>C</table>D";
    for doc in documents(source) {
        assert_eq!(children(&doc, element(&doc, "body")), ["ruby", "#C", "table", "#D"]);
        assert_eq!(tree(&doc, id(&doc, "host")), "<ruby><rb>A</rb><rt>B</rt></ruby>");
        assert!(doc.nodes[id(&doc, "table")].children.is_empty());
        unique_links(&doc);
    }
}

#[test]
fn ruby_annotation_changes_stay_in_cells_and_captions_and_respect_outer_scope() {
    for (prefix, suffix) in [("<table><tr><td id=cell>", "</td></tr></table>"),
        ("<table><caption id=cell>", "</caption></table>")] {
        for doc in documents(&format!("{prefix}<ruby id=host><rt>A<rb>B</ruby><span id=tail>C</span>{suffix}")) {
            assert_eq!(children(&doc, id(&doc, "cell")), ["ruby", "span"]);
            assert_eq!(tree(&doc, id(&doc, "host")), "<ruby><rt>A</rt><rb>B</rb></ruby>");
            assert_eq!(children(&doc, id(&doc, "tail")), ["#C"]);
            unique_links(&doc);
        }
        for doc in documents(&format!("<ruby id=host>{prefix}<p id=old>A<rt id=new>B{suffix}")) {
            assert_eq!(children(&doc, id(&doc, "cell")), ["p"]);
            assert_eq!(children(&doc, id(&doc, "old")), ["#A", "rt"]);
            assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
        }
    }
}

#[test]
fn ruby_starts_keep_existing_formatting_identity_and_do_not_reconstruct_on_insertion() {
    let source = "<ruby id=host><b id=mark><p id=old>A<rt id=annotation></rt>C";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", false).unwrap()]) {
        assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "mark")]);
        assert_eq!(children(&doc, id(&doc, "mark")), ["p", "rt", "#C"]);
        assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
        assert!(doc.nodes[id(&doc, "annotation")].children.is_empty());
        assert_eq!(doc.nodes.iter().filter(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "b")).count(), 1);
        assert_eq!(doc.nodes.len(), if doc.nodes[0].namespace == Namespace::None && doc.nodes[0].children.iter().any(|&node| matches!(&doc.nodes[node].kind, NodeKind::Element { tag, .. } if tag == "html")) { 10 } else { 7 });
        unique_links(&doc);
    }
    let source = "<ruby id=host><p><b id=mark>A</p><rt id=annotation></rt>B";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", false).unwrap()]) {
        let ruby = id(&doc, "host");
        assert_eq!(children(&doc, ruby), ["p", "rt", "b"]);
        assert!(doc.nodes[id(&doc, "annotation")].children.is_empty());
        let original = doc.nodes[doc.nodes[ruby].children[0]].children[0];
        let copy = doc.nodes[ruby].children[2];
        assert_ne!(original, copy);
        assert_eq!(children(&doc, original), ["#A"]);
        assert_eq!(children(&doc, copy), ["#B"]);
        assert_eq!(doc.attr(copy, "id"), Some("mark"));
        unique_links(&doc);
    }
}

#[test]
fn ruby_start_tokens_preserve_folded_names_nonvoid_slashes_attributes_and_nonce_evidence() {
    let source = "<ruby id=host><RB id=old ID=discarded data-x='A&amp;B' />A<RT id=new /><script id=s nonce=one NONCE=two>noop()</script><img id=i src='image.png'>B";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(children(&doc, id(&doc, "host")), ["rb", "rt"]);
        assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["script", "img", "#B"]);
        assert_eq!(doc.attr(id(&doc, "old"), "data-x"), Some("A&B"));
        assert_eq!(doc.attr(id(&doc, "s"), "nonce"), Some("one"));
        assert!(doc.nonce_unsafe_nodes.contains(&id(&doc, "s")));
        assert_eq!(doc.nonce_unsafe_nodes.len(), 1);
        assert_eq!(doc.attr(id(&doc, "i"), "src"), Some("image.png"));
        unique_links(&doc);
    }
}

#[test]
fn ruby_implied_pops_count_only_real_nodes_and_avoid_false_nesting_refusal() {
    let source = "<ruby id=host><rb><rtc><rt><rp>";
    let (doc, _) = parse_inert_html(source, 9, &|| Ok(())).unwrap();
    assert_eq!(tree(&doc, id(&doc, "host")), "<ruby><rb></rb><rtc><rt></rt><rp></rp></rtc></ruby>");
    assert_eq!(doc.nodes.len(), 9);
    assert!(parse_inert_html(source, 8, &|| Ok(())).unwrap_err().contains("node limit"));
    let source = format!("<ruby id=host>{}", "<rt>".repeat(MAX_DEPTH * 2));
    for doc in documents(&source) {
        assert_eq!(doc.nodes[id(&doc, "host")].children.len(), MAX_DEPTH * 2);
        assert!(doc.nodes[id(&doc, "host")].children.iter().all(|&node| doc.nodes[node].children.is_empty()));
        unique_links(&doc);
    }
    let doc = parse_fragment(&source, "div", false).unwrap();
    assert_eq!(doc.nodes[id(&doc, "host")].children.len(), MAX_DEPTH * 2);
    assert_eq!(doc.nodes.len(), MAX_DEPTH * 2 + 2);
    unique_links(&doc);
    let too_deep = format!("<ruby>{}<rt>", "<span>".repeat(MAX_DEPTH));
    assert!(parse_loaded_document(&too_deep, &|| Ok(())).unwrap_err().contains("nesting limit"));
    assert!(parse_fragment(&too_deep, "div", false).unwrap_err().contains("node/depth limit"));
}

#[test]
fn ruby_start_recovery_remains_cancelable_and_does_not_keep_state_after_refusal() {
    let source = format!("<ruby>{}", "<rt>x".repeat(1024));
    for inert in [false, true] {
        let calls = std::cell::Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            if calls.get() == 32 { Err("original ruby start cancellation".into()) } else { Ok(()) }
        };
        let error = if inert { parse_inert_html(&source, 5000, &cancel).unwrap_err() }
            else { parse_loaded_document(&source, &cancel).unwrap_err() };
        assert_eq!(error, "original ruby start cancellation");
        assert_eq!(calls.get(), 32);
    }
    checked_trees("<ruby id=host><rt>A<rb>B", "<ruby><rt>A</rt><rb>B</rb></ruby>");
}

#[test]
fn ruby_starts_preserve_unknown_foreign_uncertain_and_flat_subset_behavior() {
    for foreign in ["svg", "math"] {
        for doc in documents(&format!("<ruby id=host><{foreign}><rt id=old>A<rb id=new>B")) {
            assert_eq!(doc.nodes[id(&doc, "old")].namespace, Namespace::UnknownForeign);
            assert_eq!(doc.nodes[id(&doc, "new")].namespace, Namespace::UnknownForeign);
            assert_eq!(children(&doc, id(&doc, "old")), ["#A", "rb"]);
        }
    }
    for doc in documents("<div><svg><span></div><ruby id=host><rt id=old>A<rb id=new>B") {
        assert_eq!(doc.nodes[id(&doc, "host")].namespace, Namespace::UnknownForeign);
        assert_eq!(children(&doc, id(&doc, "old")), ["#A", "rb"]);
    }
    let doc = parse("<ruby id=host><rt>A<rb>B");
    assert_eq!(tree(&doc, id(&doc, "host")), "<ruby><rt>A<rb>B</rb></rt></ruby>");
}

#[test]
fn ruby_start_slice_retains_checked_context_select_table_and_foreign_refusals() {
    for context in ["ruby", "rb", "rtc", "rp", "rt"] {
        assert!(parse_fragment("<rt>A<rb>B", context, false).unwrap_err().contains("element context"));
    }
    for context in ["select", "option", "optgroup"] {
        assert!(parse_fragment("<ruby><rt>A<rb>B", context, true).is_err());
    }
    for source in ["<ruby><select><option>A<rt>B", "<ruby><table><rt>B", "<ruby><rt>\0<rb>B"] {
        assert!(parse_fragment(source, "div", false).is_err(), "{source}");
    }
    assert!(parse_fragment("<svg><ruby><rt>A<rb>B</ruby></svg>", "template", false).is_err());
}
