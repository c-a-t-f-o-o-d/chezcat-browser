//! Original existing-native-checkbox moves; no new INPUT admission or parser mode.
use super::*;

fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}
fn current()->crate::navigation::Location {
    crate::navigation::address("https://checkbox.test/start",&crate::navigation::Location::home()).unwrap()
}
fn fixture(checkbox:&str,source:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse(&format!("<html><body><form id=a action='/a'><input id=neighbor name=keep value=seed><section id=shell>{checkbox}</section><input name=tail value=after></form><form id=b action='/b'><div id=dest><span id=old>Old</span></div><fieldset disabled><legend><span id=allowed></span></legend><div id=blocked></div><legend><span id=not-allowed></span></legend></fieldset></form><svg id=foreign></svg><template id=inert></template></body></html>"));
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();page.set_location(&current());
    let result=page.start(&mut doc,&mut forms,&[Script{node:0,label:"Original native checkbox moves".into(),source:source.into()}]);
    assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn eval(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let result=page.console_eval(doc,forms,source);assert_eq!(result.errors,0,"{source}\n{}",page.report());
}
fn query(doc:&Document,forms:&Forms,owner:&str)->String {
    forms.submit_request(doc,id(doc,owner),None,&current(),&current()).unwrap().location.url.unwrap().query.unwrap_or_default()
}
fn editor(forms:&Forms,node:usize)->(String,usize,usize,usize,bool,Option<usize>) {
    let control=&forms.controls[&node];
    (control.value.text(),control.value.cursor,control.value.anchor,control.value.offset,control.dirty_value,control.owner)
}
struct Kept {
    value:String, caption:String, raw_value:Option<String>, raw_checked:Option<String>,
    neighbor:(String,usize,usize,usize,bool,Option<usize>), controls:Vec<usize>,
}
impl Kept {
    fn new(doc:&Document,forms:&mut Forms)->Self {
        let neighbor=id(doc,"neighbor");let check=id(doc,"check");
        let edit=forms.controls.get_mut(&neighbor).unwrap();edit.value.cursor=2;edit.value.anchor=6;edit.value.offset=1;
        let control=&forms.controls[&check];
        assert_eq!(control.kind,Kind::Checkbox);assert_eq!(control.owner,Some(id(doc,"a")));
        assert_eq!(editor(forms,neighbor).0,"neighbor edited");assert!(forms.controls[&neighbor].dirty_value);
        assert_eq!(forms.text_default_value(neighbor).unwrap(),"seed");
        Self{value:control.value.text(),caption:control.caption.clone(),
            raw_value:doc.attr(check,"value").map(str::to_owned),raw_checked:doc.attr(check,"checked").map(str::to_owned),
            neighbor:editor(forms,neighbor),controls:forms.controls.keys().copied().collect()}
    }
    fn assert(&self,doc:&Document,forms:&Forms,checked:bool,owner:Option<&str>,disabled:bool) {
        let check=id(doc,"check");let control=&forms.controls[&check];
        assert_eq!(control.kind,Kind::Checkbox);assert_eq!(control.value.text(),self.value);assert_eq!(control.caption,self.caption);
        assert_eq!(control.checked,checked);assert_eq!(control.owner,owner.map(|name|id(doc,name)));assert_eq!(control.disabled,disabled);
        assert_eq!(doc.attr(check,"value"),self.raw_value.as_deref());assert_eq!(doc.attr(check,"checked"),self.raw_checked.as_deref());
        assert_eq!(editor(forms,id(doc,"neighbor")),self.neighbor);assert_eq!(forms.text_default_value(id(doc,"neighbor")).unwrap(),"seed");
        assert_eq!(forms.controls.keys().copied().collect::<Vec<_>>(),self.controls);
    }
}

#[test]
fn checkbox_move_existing_native_through_fragment_form_and_fieldset_preserves_state() {
    let(mut doc,mut forms,mut page)=fixture("<input id=check type=cHeCkBoX name=choice value=yes checked>",
        "var check=document.getElementById('check'),fragment=document.createDocumentFragment(),list=fragment.childNodes,dest=document.getElementById('dest');document.getElementById('neighbor').value='neighbor edited';check.checked=false;");
    let kept=Kept::new(&doc,&mut forms);kept.assert(&doc,&forms,false,Some("a"),false);
    assert_eq!(kept.value,"yes");assert_eq!(kept.raw_checked.as_deref(),Some(""));assert_eq!(query(&doc,&forms,"a"),"keep=neighbor+edited&tail=after");
    eval(&mut doc,&mut forms,&mut page,r#"
if(fragment.appendChild(check)!==check||check.parentNode!==fragment||check.getRootNode()!==fragment||check.form!==null||check.isConnected||check.ownerDocument!==document||list.length!==1||list[0]!==check)throw 'genuine detached existing checkbox';
"#);
    kept.assert(&doc,&forms,false,None,false);assert_eq!(query(&doc,&forms,"b"),"");
    eval(&mut doc,&mut forms,&mut page,r#"
if(dest.appendChild(fragment)!==fragment||list.length!==0||check.parentNode!==dest||check.getRootNode()!==document||check.form!==document.getElementById('b')||!check.isConnected||check.ownerDocument!==document||document.getElementById('check')!==check)throw 'genuine reconnected existing checkbox';
"#);
    kept.assert(&doc,&forms,false,Some("b"),false);
    eval(&mut doc,&mut forms,&mut page,"document.getElementById('blocked').appendChild(check);");kept.assert(&doc,&forms,false,Some("b"),true);
    eval(&mut doc,&mut forms,&mut page,"document.getElementById('allowed').appendChild(check);");kept.assert(&doc,&forms,false,Some("b"),false);
    eval(&mut doc,&mut forms,&mut page,"document.getElementById('not-allowed').appendChild(check);");kept.assert(&doc,&forms,false,Some("b"),true);
    eval(&mut doc,&mut forms,&mut page,"dest.appendChild(check);");kept.assert(&doc,&forms,false,Some("b"),false);
    forms.reset(id(&doc,"b"));kept.assert(&doc,&forms,true,Some("b"),false);
    assert_eq!(query(&doc,&forms,"b"),"choice=yes");assert_eq!(query(&doc,&forms,"a"),"keep=neighbor+edited&tail=after");
    assert!(!page.has_navigation_request());
}

#[test]
fn checkbox_move_existing_subtree_replacement_retirement_and_reset_keep_default_on() {
    let(mut doc,mut forms,mut page)=fixture("<input id=check type=checkbox name=choice>",
        "var check=document.getElementById('check'),shell=document.getElementById('shell'),dest=document.getElementById('dest'),old=document.getElementById('old');document.getElementById('neighbor').value='neighbor edited';check.checked=true;");
    let kept=Kept::new(&doc,&mut forms);kept.assert(&doc,&forms,true,Some("a"),false);
    assert_eq!(kept.value,"on");assert_eq!(kept.raw_value,None);assert_eq!(kept.raw_checked,None);
    assert_eq!(query(&doc,&forms,"a"),"keep=neighbor+edited&choice=on&tail=after");
    eval(&mut doc,&mut forms,&mut page,"if(dest.replaceChild(shell,old)!==old||shell.parentNode!==dest||check.form!==document.getElementById('b')||check.ownerDocument!==document)throw 'whole native checkbox replacement';");
    kept.assert(&doc,&forms,true,Some("b"),false);assert_eq!(query(&doc,&forms,"b"),"choice=on");
    eval(&mut doc,&mut forms,&mut page,"if(dest.replaceChild(old,shell)!==shell||shell.parentNode!==null||check.form!==null||check.getRootNode()!==shell||check.isConnected)throw 'whole native checkbox removal';");
    kept.assert(&doc,&forms,true,None,false);assert_eq!(query(&doc,&forms,"b"),"");
    eval(&mut doc,&mut forms,&mut page,"dest.appendChild(shell);dest.innerHTML='<p id=replacement>Replacement</p>';if(check.form!==null||check.isConnected||shell.parentNode!==null||document.getElementById('check')!==null)throw 'native checkbox retirement';");
    kept.assert(&doc,&forms,true,None,false);
    eval(&mut doc,&mut forms,&mut page,"dest.appendChild(shell);if(check.form!==document.getElementById('b')||check.ownerDocument!==document||!check.isConnected||document.getElementById('check')!==check)throw 'saved native checkbox reconnection';");
    kept.assert(&doc,&forms,true,Some("b"),false);forms.reset(id(&doc,"b"));kept.assert(&doc,&forms,false,Some("b"),false);
    assert_eq!(query(&doc,&forms,"b"),"");assert_eq!(query(&doc,&forms,"a"),"keep=neighbor+edited&tail=after");
    assert!(!page.has_navigation_request());
}

#[derive(Debug,PartialEq,Eq)]
struct Snapshot {
    tree:String, parents:Vec<Option<usize>>, connected:Vec<bool>, created:HashSet<usize>, detached:HashSet<usize>,
    native:Vec<(usize,Kind,String,String,String,bool,bool,Option<usize>,bool,(usize,usize,usize))>,
    defaults:Vec<(usize,String)>, attributes:Vec<(Option<usize>,String,Option<String>,String)>,
    retained:usize, commits:Option<u64>,
}
impl Snapshot {
    fn capture(doc:&Document,forms:&Forms,page:&Page)->Self {
        Self{tree:format!("{:?}",doc.nodes),parents:page.state.tree.parents.clone(),connected:page.state.dom.connected.clone(),
            created:page.state.dom.created.clone(),detached:page.state.detached_nodes.clone(),
            native:forms.controls.iter().map(|(&node,c)|(node,c.kind,c.name.clone(),c.value.text(),c.caption.clone(),c.checked,c.dirty_value,c.owner,c.disabled,(c.value.cursor,c.value.anchor,c.value.offset))).collect(),
            defaults:forms.controls.iter().filter(|(_,c)|c.kind==Kind::Text).map(|(&node,_)|(node,forms.text_default_value(node).unwrap().to_owned())).collect(),
            attributes:page.state.attributes.iter().map(|attr|(attr.owner,attr.name.qualified.clone(),attr.name.namespace_uri().map(str::to_owned),attr.value.clone())).collect(),
            retained:page.state.retained,commits:page.state.dom_commits}
    }
}
fn no_resources(page:&mut Page) {
    assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());assert!(page.take_image_changes().2.is_empty());assert!(!page.has_navigation_request());
}

#[test]
fn checkbox_move_forbidden_contexts_and_explicit_form_association_remain_atomic() {
    let(mut doc,mut forms,mut page)=fixture("<input id=check type=checkbox name=choice value=yes checked>",
        "var check=document.getElementById('check'),shell=document.getElementById('shell'),text=document.createTextNode('forbidden');document.getElementById('neighbor').value='neighbor edited';check.checked=false;");
    let kept=Kept::new(&doc,&mut forms);page.state.rebuild(&doc).unwrap();let before=Snapshot::capture(&doc,&forms,&page);
    for source in ["document.getElementById('foreign').appendChild(check);","document.getElementById('inert').appendChild(shell);","document.getElementById('neighbor').appendChild(check);","check.appendChild(text);","check.innerHTML='<span>not native contents</span>';"] {
        let result=page.console_eval(&mut doc,&mut forms,source);assert_eq!(result.errors,1,"{source}\n{}",page.report());
        assert_eq!(Snapshot::capture(&doc,&forms,&page),before,"{source}");kept.assert(&doc,&forms,false,Some("a"),false);no_resources(&mut page);
    }
    let(mut doc,mut forms,mut page)=fixture("<input id=check type=checkbox form=a name=choice value=yes checked>",
        "var check=document.getElementById('check'),shell=document.getElementById('shell'),fragment=document.createDocumentFragment();document.getElementById('neighbor').value='neighbor edited';check.checked=false;");
    let kept=Kept::new(&doc,&mut forms);page.state.rebuild(&doc).unwrap();let before=Snapshot::capture(&doc,&forms,&page);
    for source in ["document.getElementById('dest').appendChild(check);","fragment.appendChild(shell);","document.getElementById('dest').replaceChild(shell,document.getElementById('old'));"] {
        let result=page.console_eval(&mut doc,&mut forms,source);assert_eq!(result.errors,1,"{source}\n{}",page.report());
        assert_eq!(Snapshot::capture(&doc,&forms,&page),before,"{source}");kept.assert(&doc,&forms,false,Some("a"),false);no_resources(&mut page);
    }
}

#[test]
fn checkbox_move_keeps_new_registration_innerhtml_and_type_transitions_refused() {
    let(mut doc,mut forms,mut page)=fixture("<input id=check type=checkbox name=choice value=yes checked>",
        "var check=document.getElementById('check'),shell=document.getElementById('shell'),fresh=document.createElement('input'),attr=document.createAttribute('type');attr.value='checkbox';document.getElementById('neighbor').value='neighbor edited';check.checked=false;");
    let kept=Kept::new(&doc,&mut forms);page.state.rebuild(&doc).unwrap();let before=Snapshot::capture(&doc,&forms,&page);
    for source in ["fresh.type='checkbox';","fresh.setAttribute('type','checkbox');","fresh.setAttributeNS(null,'type','checkbox');","fresh.setAttributeNode(attr);","document.getElementById('neighbor').type='checkbox';","check.type='text';","check.type='hidden';","shell.innerHTML='<input type=checkbox>';",
        r#"shell.innerHTML="  <link/><table></table><a href='/a'>a</a><input type='checkbox'/>";"#,
        "shell.innerHTML='<link rel=stylesheet href=/must-not-fetch.css><a href=/a>a</a>';","shell.innerHTML='<input type=radio>';"] {
        let result=page.console_eval(&mut doc,&mut forms,source);assert_eq!(result.errors,1,"{source}\n{}",page.report());
        assert_eq!(Snapshot::capture(&doc,&forms,&page),before,"{source}");kept.assert(&doc,&forms,false,Some("a"),false);no_resources(&mut page);
    }
}

#[test]
fn checkbox_move_resource_batches_keep_donor_native_and_queues_unchanged() {
    for (container,factory) in [("ordinary","document.createElement('section')"),("fragment","document.createDocumentFragment()")] {
        for resource in ["script","link"] {
            let setup=format!("var check=document.getElementById('check'),carrier={factory},resource=document.createElement('{resource}');document.getElementById('neighbor').value='neighbor edited';check.checked=false;");
            let(mut doc,mut forms,mut page)=fixture("<input id=check type=checkbox name=choice value=yes checked>",&setup);
            let kept=Kept::new(&doc,&mut forms);
            eval(&mut doc,&mut forms,&mut page,if resource=="script" {"resource.src='/must-not-fetch.js';carrier.appendChild(resource);carrier.appendChild(check);"} else {"resource.rel='stylesheet';resource.href='/must-not-fetch.css';carrier.appendChild(resource);carrier.appendChild(check);"});
            kept.assert(&doc,&forms,false,None,false);no_resources(&mut page);page.state.rebuild(&doc).unwrap();let before=Snapshot::capture(&doc,&forms,&page);
            let result=page.console_eval(&mut doc,&mut forms,"document.getElementById('dest').appendChild(carrier);");assert_eq!(result.errors,1,"{container}/{resource}: {}",page.report());
            assert_eq!(Snapshot::capture(&doc,&forms,&page),before,"{container}/{resource}");kept.assert(&doc,&forms,false,None,false);no_resources(&mut page);
        }
    }
}

#[test]
fn checkbox_move_work_copy_and_existing_node_retention_limits_publish_no_prefix() {
    for copy_limit in [false,true] {
        let mut failures=0;let mut successes=0;
        for allowance in [0,1,8,64,256,4096,65536] {
            let(mut doc,mut forms,mut page)=fixture("<input id=check type=checkbox name=choice value=yes checked>",
                "var check=document.getElementById('check'),shell=document.getElementById('shell'),fragment=document.createDocumentFragment();document.getElementById('neighbor').value='neighbor edited';check.checked=false;");
            let kept=Kept::new(&doc,&mut forms);eval(&mut doc,&mut forms,&mut page,"fragment.appendChild(shell);");kept.assert(&doc,&forms,false,None,false);
            page.state.rebuild(&doc).unwrap();let donor=doc.nodes.iter().position(|node|matches!(node.kind,NodeKind::DocumentFragment)).unwrap();let dest=id(&doc,"dest");
            let before=Snapshot::capture(&doc,&forms,&page);page.state.node_limit=doc.nodes.len();page.state.retained_limit=page.state.retained;
            if copy_limit {page.state.copy_work=allowance;} else {page.state.host_work=allowance;}
            let result=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.splice_document_fragment(dest,"appendChild",&[Value::Host(node_handle(donor))]);
            if result.is_err() {
                failures+=1;assert_eq!(Snapshot::capture(&doc,&forms,&page),before,"copy={copy_limit} allowance={allowance}");kept.assert(&doc,&forms,false,None,false);
            } else {
                successes+=1;kept.assert(&doc,&forms,false,Some("b"),false);assert!(doc.nodes[donor].children.is_empty());assert_eq!(page.state.retained,before.retained);assert_eq!(page.state.dom_commits,before.commits.and_then(|value|value.checked_add(1)));
            }
            no_resources(&mut page);
        }
        assert!(failures>0&&successes>0,"Both atomic refusal and ordinary successful transfer must be observed");
    }
}
