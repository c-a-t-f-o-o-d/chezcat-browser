//! Original cascade controls for argument specificity and disabled negation.
use super::*;

fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}

#[test]
fn selector_negation_original_cascade_uses_argument_specificity_without_extra_pseudo_weight() {
    let markup=concat!(
        "<p id=tie class='candidate extra'>Equal class specificity</p>",
        "<p id=specific class=candidate>ID argument specificity</p>",
        "<button id=active class=action data-action=choose>Active</button>",
        "<button id=classed class='action disabled' data-action=choose>Classed</button>",
        "<button id=off class=action data-action=choose disabled>Disabled</button>"
    );
    let base=concat!(
        ".candidate{color:#123456}.candidate.extra{color:#445566}",
        "#specific{background-color:#334455}.action[data-action=choose]{color:#aa1122}"
    );
    let literal=crate::html::parse(&format!("<!doctype html><style>{base}</style>{markup}"));
    let positive=compute(&literal,&HashMap::new());
    assert_eq!(positive[id(&literal,"tie")].color,0x445566);
    assert_eq!(positive[id(&literal,"specific")].background,Some(0x334455));
    for name in ["active","classed","off"] {assert_eq!(positive[id(&literal,name)].color,0xaa1122,"literal {name}");}

    // First new parse boundary, after the literal attribute/cascade positives.
    assert!(valid_query_selector("[data-action=choose]:not(.disabled):not(:disabled)"),"original compound negation must parse");
    let source=concat!(
        ".candidate{color:#123456}",
        ".candidate:not(.missing){color:#111111}",
        ".candidate.extra{color:#445566}",
        ".candidate:not(#absent){background-color:#abcdef}",
        "#specific{background-color:#334455}",
        ".action[data-action=choose]{color:#aa1122}",
        "[data-action=choose]:not(.disabled):not(:disabled){color:#0d6efd}"
    );
    let doc=crate::html::parse(&format!("<!doctype html><style>{source}</style>{markup}"));
    let styles=compute(&doc,&HashMap::new());
    assert_eq!(styles[id(&doc,"tie")].color,0x445566,"not contributes its class argument, no extra pseudo-class weight");
    assert_eq!(styles[id(&doc,"specific")].background,Some(0xabcdef),"ID argument participates in specificity");
    assert_eq!(styles[id(&doc,"active")].color,0x0d6efd);
    assert_eq!(styles[id(&doc,"classed")].color,0xaa1122);
    assert_eq!(styles[id(&doc,"off")].color,0xaa1122);
}
