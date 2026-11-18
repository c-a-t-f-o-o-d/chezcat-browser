//! Original strict negation lists, actual HTML state and shared-work controls.
use super::*;
fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}

#[test]
fn selector_negation_strict_lists_depth_and_atomic_unsupported_members() {
    for selector in ["input:not(:checked)","button:not(.x,#absent)","button:not(div > .x,span + button)","button:not(:not(.selected))",":not(*)",":not([data-key='x,y)'])",":NoT(.selected)"] {
        assert!(valid_query_selector(selector),"{selector}");
    }
    for selector in [":not",":not()",":not( )",":not(,a)",":not(a,)",":not(a,,b)",":not(>a)",":not(a >)",":not(::before)",":not(a::after)",":not(.missing,:hover)",":not(:not(.missing,:hover))",":not(:is(.x))",":not(.x))",":not(.x",":disabled()",":disabled(yes)"] {
        assert!(!valid_query_selector(selector),"{selector}");
        let list=format!(".safe,{selector}");
        let mut rules=Vec::new();parse_sheet(&format!("{list}{{color:red}}"),&mut rules);
        assert!(rules.is_empty(),"invalid member broadened whole style list: {selector}");
        let doc=crate::html::parse("<p id=target class=safe>Target</p>");let mut budget=100_000;
        assert!(query_all(&doc,&list,&[id(&doc,"target")],None,&mut budget).is_err(),"{selector}");
        assert_eq!(budget,100_000,"invalid query published or charged a partial match: {selector}");
    }
    let nested=|depth:usize|format!("{}*{}",":not(".repeat(depth),")".repeat(depth));
    assert!(valid_query_selector(&nested(MAX_SELECTOR_NESTING)));
    assert!(!valid_query_selector(&nested(MAX_SELECTOR_NESTING+1)));
}

#[test]
fn selector_negation_complex_nested_checked_and_max_argument_specificity() {
    let doc=crate::html::parse("<div class=blocked><input id=a type=checkbox checked data-key='x,y)'></div><input id=b type=checkbox class=selected><input id=c type=checkbox>");
    let(a,b,c)=(id(&doc,"a"),id(&doc,"b"),id(&doc,"c"));let candidates=[a,b,c];
    for (source,expected) in [
        ("input:not(.selected,#a)",vec![c]),
        ("input:not(.blocked > input)",vec![b,c]),
        ("input:not(:not(.selected))",vec![b]),
        ("input:not([data-key='x,y)'])",vec![b,c]),
        ("input:not(:checked)",vec![b,c]),
    ] {let mut budget=100_000;assert_eq!(query_all(&doc,source,&candidates,None,&mut budget).unwrap(),expected,"{source}");}
    let checked=|node|Some(node==b);let mut budget=100_000;
    assert_eq!(query_all(&doc,"input:not(:checked)",&candidates,Some(&checked),&mut budget).unwrap(),vec![a,c],"live checkedness must reach nested selectors");
    let doc=crate::html::parse(concat!(
        "<style>.candidate:not(#none,#other){background-color:red}",
        "#target.candidate{background-color:blue}",
        ".candidate:not(#none,.missing){color:#112233}",
        ".candidate.extra{color:#445566}</style>",
        "<p id=target class='candidate extra'>Specificity</p>"
    ));
    let styles=compute(&doc,&HashMap::new());let target=id(&doc,"target");
    assert_eq!(styles[target].background,Some(0x0000ff),"argument list maximum, not sum, ties the later ID/class rule");
    assert_eq!(styles[target].color,0x112233,"highest argument specificity is used even though neither argument matches");
}

#[test]
fn selector_negation_disabled_options_groups_selects_and_nested_first_legends() {
    let doc=crate::html::parse(concat!(
        "<fieldset disabled id=outer><legend><select id=legend><optgroup id=legend-group><option id=legend-option>Allowed</option></optgroup></select></legend>",
        "<select id=inherited><optgroup id=inherited-group><option id=inherited-option>Blocked</option></optgroup></select>",
        "<fieldset id=nested><legend><input id=nested-input></legend></fieldset>",
        "<datalist><option id=data-option>Ordinary</option></datalist></fieldset>",
        "<select id=enabled><option id=normal>Normal</option><optgroup id=group disabled><option id=group-option>Group</option></optgroup><option id=own-option disabled>Own</option></select>",
        "<select disabled id=own-select><optgroup id=own-group><option id=own-select-option>Own select</option></optgroup></select>",
        "<div id=ordinary disabled>Ordinary attribute</div><svg id=foreign disabled></svg>"
    ));
    let forms=crate::forms::Forms::new(&doc);
    assert!(!forms.controls[&id(&doc,"legend")].disabled);
    assert!(forms.controls[&id(&doc,"inherited")].disabled);
    assert!(forms.controls[&id(&doc,"nested-input")].disabled);
    for (name,expected) in [("outer",true),("legend",false),("legend-group",false),("legend-option",false),("inherited",true),("inherited-group",true),("inherited-option",true),("nested",true),("nested-input",true),("data-option",false),("enabled",false),("normal",false),("group",true),("group-option",true),("own-option",true),("own-select",true),("own-group",true),("own-select-option",true),("ordinary",false),("foreign",false)] {
        let node=id(&doc,name);let mut budget=100_000;
        assert_eq!(query_first(&doc,":disabled",&[node],None,&mut budget).unwrap().is_some(),expected,"{name}");
        let mut budget=100_000;assert_eq!(query_first(&doc,":not(:disabled)",&[node],None,&mut budget).unwrap().is_some(),!expected,"negated {name}");
    }
}

#[test]
fn selector_negation_exhaustion_is_never_inverted_into_a_match() {
    let doc=crate::html::parse("<fieldset disabled><div><button id=target class=target>Target</button></div></fieldset>");let target=id(&doc,"target");
    for source in [":not(*)",":not(:disabled)"] {
        let mut budget=doc.nodes.len()*3+1;
        let error=query_first(&doc,source,&[target],None,&mut budget).unwrap_err();
        assert!(error.contains("work limit"),"{source}: {error}");assert_eq!(budget,0);
    }
    let parents=vec![None;doc.nodes.len()];let previous=vec![None;doc.nodes.len()];
    let context=MatchContext{doc:&doc,parents:&parents,previous_elements:&previous,checked:None,charge_text:false};
    let mut budget=1;assert!(matches_selector(&parse_selector(".target").unwrap(),target,&context,&mut budget));assert_eq!(budget,0,"old stylesheet boundary is retained");
    let mut budget=1;assert!(!matches_selector(&parse_selector(":not(*)").unwrap(),target,&context,&mut budget));assert_eq!(budget,0);
}
