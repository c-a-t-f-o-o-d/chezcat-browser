//! Original shared selector reduction over actual native disabled controls.
use super::*;

fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}
fn editor(forms:&Forms,node:usize)->(String,usize,usize,usize,bool,Option<usize>) {
    let c=&forms.controls[&node];(c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.owner)
}

#[test]
fn selector_negation_original_scoped_queries_follow_real_disabledness_and_first_legend() {
    let mut doc=crate::html::parse(concat!(
        "<!doctype html><title>Original selector negation</title><body><form id=f>",
        "<input id=neighbor name=keep value=original><div id=scope>",
        "<button id=enabled type=button data-action=choose>Enabled</button>",
        "<button id=classed type=button class=disabled data-action=choose>Class only</button>",
        "<button id=own type=button disabled data-action=choose>Own disabled</button>",
        "<fieldset id=fieldset disabled><legend><button id=first type=button data-action=choose>First legend</button></legend>",
        "<button id=blocked type=button data-action=choose>Fieldset disabled</button>",
        "<legend><button id=later type=button data-action=choose>Later legend</button></legend></fieldset>",
        "<span id=decorative disabled data-action=choose>Ordinary disabled attribute</span></div></form>",
        "<button id=outside type=button data-action=choose>Outside scope</button>"
    ));
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    let location=crate::navigation::address("https://selector-negation.test/original",&crate::navigation::Location::home()).unwrap();
    page.set_location(&location);page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    let neighbor=id(&doc,"neighbor");forms.script_set_value(neighbor,"edited neighbor").unwrap();
    {let e=&mut forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
    let kept=editor(&forms,neighbor);
    for (name,disabled) in [("enabled",false),("classed",false),("own",true),("first",false),("blocked",true),("later",true)] {
        let node=id(&doc,name);assert_eq!(forms.controls[&node].disabled,disabled,"native {name}");
        assert_eq!(forms.controls[&node].owner,Some(id(&doc,"f")),"native owner {name}");
    }
    assert!(!forms.controls.contains_key(&id(&doc,"decorative")));
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");

    let prefix=page.console_eval(&mut doc,&mut forms,r#"
var scope=document.getElementById('scope'),enabled=document.getElementById('enabled'),classed=document.getElementById('classed'),own=document.getElementById('own'),first=document.getElementById('first'),blocked=document.getElementById('blocked'),later=document.getElementById('later'),decorative=document.getElementById('decorative'),outside=document.getElementById('outside');
var existing=scope.querySelectorAll('[data-action=choose]');
if(existing.length!==7||existing[0]!==enabled||existing[1]!==classed||existing[2]!==own||existing[3]!==first||existing[4]!==blocked||existing[5]!==later||existing[6]!==decorative||scope.querySelector('#outside')!==null)throw 'existing attribute query identities/order/scope';
if(!classed.matches('.disabled')||enabled.matches('.disabled')||!own.matches('[disabled]')||!decorative.matches('[disabled]')||blocked.getAttribute('disabled')!==null||first.getAttribute('disabled')!==null||later.getAttribute('disabled')!==null)throw 'class/raw attribute versus inherited native disabledness';
if(first.parentNode.nodeName!=='LEGEND'||blocked.parentNode!==document.getElementById('fieldset')||later.parentNode.nodeName!=='LEGEND'||first.ownerDocument!==document||decorative.ownerDocument!==document)throw 'physical first/later legend topology';
if(document.querySelectorAll('[data-action=choose]').length!==8||document.getElementById('neighbor').value!=='edited neighbor'||document.getElementById('neighbor').defaultValue!=='original'||document.getElementById('neighbor').form!==document.getElementById('f'))throw 'Document/native prefix';
document.title='Selector negation existing prefix passed';
"#);
    assert_eq!(prefix.errors,0,"Existing prefix: {}",page.report());
    assert_eq!(doc.title,"Selector negation existing prefix passed");
    assert_eq!(editor(&forms,neighbor),kept);
    let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;

    let outcome=page.console_eval(&mut doc,&mut forms,r#"
var selector='[data-action="choose"]:not(.disabled):not(:disabled)';
var selected=scope.querySelectorAll(selector);
if(selected.length!==3||selected[0]!==enabled||selected[1]!==first||selected[2]!==decorative||scope.querySelector(selector)!==enabled)throw 'compound negation must use actual native disabledness';
if(!enabled.matches(selector)||classed.matches(selector)||own.matches(selector)||!first.matches(selector)||blocked.matches(selector)||later.matches(selector)||!decorative.matches(selector)||first.closest(selector)!==first)throw 'matches/closest positive and negative controls';
if(!own.matches(':disabled')||!blocked.matches(':disabled')||!later.matches(':disabled')||first.matches(':disabled')||classed.matches(':disabled')||decorative.matches(':disabled'))throw 'disabled pseudo is effective control state rather than arbitrary attribute';
if(scope.querySelectorAll('[data-action=choose]:not(.disabled)').length!==6||scope.querySelectorAll('[data-action=choose]:not(:disabled)').length!==4||document.querySelectorAll('#scope '+selector).length!==3||document.querySelectorAll(selector).length!==4)throw 'single negation and explicit descendant scope';
"#);
    assert_eq!(format!("{:?}",doc.nodes),nodes);assert_eq!(page.state.dom_commits,commits);
    assert_eq!(editor(&forms,neighbor),kept);assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");
    assert_eq!(doc.attr(neighbor,"value"),Some("original"));
    assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
    assert_eq!(outcome.errors,0,"{}",page.report());
}
