//! Native DOM adjacent text and element operation contracts.
use super::*;

#[test]
fn adjacent_insertion_positions_preserve_native_text_element_order_and_observers() {
    let (mut doc, mut forms, mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var host=document.getElementById('mount'),center=document.createElement('span'),tail=document.createElement('b');
host.appendChild(center);host.appendChild(tail);
var rootList=host.childNodes,inner=center.childNodes,r=new Range();r.setStart(host,1);r.setEnd(host,2);
var obs=new MutationObserver(function(){});obs.observe(host,{childList:true,subtree:true});
if(center.insertAdjacentText('BEFOREBEGIN','\ud800')!==undefined||center.insertAdjacentText('AfTeReNd','')!==undefined)throw 'text return';
center.insertAdjacentText('beforeend','last');center.insertAdjacentText('afterbegin','<b>&');
if(rootList.length!==4||rootList[0].nodeType!==3||rootList[0].data!=='\ud800'||rootList[1]!==center||rootList[2].data!==''||rootList[3]!==tail||inner.length!==2||inner[0].data!=='<b>&'||inner[1].data!=='last')throw 'actual four positions';
if(r.startOffset!==2||r.endOffset!==4)throw 'range offsets';
var records=obs.takeRecords();if(records.length!==4||records[0].target!==host||records[0].nextSibling!==center||records[0].addedNodes[0]!==rootList[0]||records[1].previousSibling!==center||records[1].nextSibling!==tail||records[3].nextSibling!==inner[1])throw 'ordered observer edges';
var v=document.createElement('i');if(center.insertAdjacentElement('beforebegin',v)!==v||v.nextSibling!==center||center.insertAdjacentElement('afterend',v)!==v||v.previousSibling!==center)throw 'outer move';
if(center.insertAdjacentElement('afterbegin',v)!==v||v!==center.firstChild||center.insertAdjacentElement('beforeend',v)!==v||v!==center.lastChild)throw 'inner move identity';
var orphan=document.createElement('section');if(orphan.insertAdjacentElement('afterend',v)!==null||orphan.insertAdjacentText('beforebegin','unused')!==undefined||v.parentNode!==center)throw 'detached outer noop';
"#);
}

#[test]
fn adjacent_insertion_inert_owners_conversion_order_and_hierarchy() {
    let (mut doc, mut forms, mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var xml=document.implementation.createDocument(null,'Root'),root=xml.documentElement,el=xml.createElement('child');
root.appendChild(el);var list=root.childNodes, add=Element.prototype.insertAdjacentText, moved=Element.prototype.insertAdjacentElement;
if(add.length!==2||moved.length!==2||Object.hasOwn(el,'insertAdjacentText'))throw 'canonical methods';
var order='',a={toString:function(){order+='a';return 'afterbegin';}},v={toString:function(){order+='v';return '\udfff';}};
add.call(el,a,v);if(order!=='av'||el.firstChild.nodeType!==3||el.firstChild.data!=='\udfff'||el.firstChild.ownerDocument!==xml)throw 'exact conversion and owner';
var pivot=xml.createElement('next');if(moved.call(el,'afterend',pivot)!==pivot||list[1]!==pivot)throw 'inert afterend';
add.call(pivot,'beforebegin','between');if(list.length!==3||list[1].data!=='between'||list[1].parentNode!==root)throw 'inert edge';
var caught=false;try{add.call({},a,v)}catch(e){caught=e instanceof TypeError;}if(!caught||order!=='av')throw 'brand before coercion';
caught=false;try{add.call(el,'\\u0000',v);}catch(e){caught=e.name==='SyntaxError';}if(!caught||order!=='avv')throw 'bad position after conversion';
caught=false;try{moved.call(el,a,document.createTextNode('wrong'));}catch(e){caught=e instanceof TypeError;}if(!caught||order!=='avva')throw 'second argument Element';
caught=false;try{add.call(root,'afterend','outside');}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||xml.documentElement!==root)throw 'Document hierarchy';
var detached=xml.createElement('detached'), effect={toString:function(){root.appendChild(detached);return 'afterend';}};
add.call(detached,effect,'reentered');if(root.lastChild.data!=='reentered'||root.lastChild.previousSibling!==detached)throw 'current topology after conversion';
"#);
}

#[test]
fn adjacent_insertion_storage_and_resource_refusal_publish_no_prefix() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,"var target=document.getElementById('mount'),observer=new MutationObserver(function(){});observer.observe(target,{childList:true});var xml=document.implementation.createDocument(null,'Root');");
    let limit=page.state.retained_limit;
    for code in ["target.insertAdjacentText('beforeend','storage');","xml.documentElement.insertAdjacentText('afterbegin','storage');"] {
        let before=(doc.nodes.len(),page.state.dom_commits,page.state.retained);
        page.state.retained_limit=page.state.retained;
        assert_eq!(page.console_eval(&mut doc,&mut forms,code).errors,1,"{}",page.report());
        assert_eq!((doc.nodes.len(),page.state.dom_commits,page.state.retained),before);
        page.state.retained_limit=limit;
    }
    assert_eq!(page.console_eval(&mut doc,&mut forms,"document.getElementById('field').insertAdjacentText('beforeend','bad');").errors,1);
    run(&mut doc,&mut forms,&mut page,"if(target.firstChild!==null||xml.documentElement.firstChild!==null||observer.takeRecords().length)throw 'prefix';target.insertAdjacentText('beforeend','recovered');if(target.firstChild.data!=='recovered'||observer.takeRecords().length!==1)throw 'recovery';");
}
