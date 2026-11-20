//! Original checked-topology, traversal storage and live compaction controls.
use super::*;
fn page(markup:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse(markup);doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
#[test]
fn browser_query_topology_keeps_siblings_text_root_detached_scope_and_fatal_work_refusal() {
    let(mut doc,mut forms,mut page)=page("<html><body><div id=root>text<p id=a></p>between<p id=b></p>tail<span id=c></span></div><section id=outside></section></body></html>");
    execute(&mut page,&mut doc,&mut forms,"var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');if(document.querySelector(':root')!==document.documentElement||root.querySelector('#a + p')!==b||root.querySelector('#a ~ span')!==c||!b.matches('#a + p')||c.closest('div')!==root||root.querySelector('#outside')!==null)throw 'native query topology';document.body.removeChild(root);if(root.querySelector('#a + p')!==b||root.querySelectorAll('p').length!==2||document.querySelector('#a')!==null||b.closest('div')!==root)throw 'detached scope';var image=new Image(),fragment=document.createDocumentFragment(),comment=document.createComment('parked');if(!image.matches('img')||image.closest('body')!==null||root.querySelector('#b')!==b)throw 'new detached metadata';");
    let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="c")).unwrap();
    let before=page.state.retained;let mut work=1;let mut copies=100;
    assert!(crate::css::query_with_topology(&doc,"#a ~ span",vec![node],None,&mut work,&page.state.tree.parents,&page.state.tree.sibling_slots,true,&mut copies).unwrap_err().contains("work limit"));
    assert_eq!(work,0);assert_eq!(page.state.retained,before);
    execute(&mut page,&mut doc,&mut forms,"if(root.querySelector('#a ~ span')!==c)throw 'query recovery';");
}
#[test]
fn browser_query_candidates_fit_exact_owned_capacity_and_refuse_one_byte_short_without_mutation() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body>{}</body></html>","<p></p>".repeat(6000)));
    let baseline=page.state.retained;let limit=page.state.retained_limit;let count=6002;
    page.state.retained_limit=baseline+count*size_of::<usize>()-1;page.begin_job();
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.query_candidates(0).is_err());
    assert_eq!(page.state.retained,baseline);assert!(page.state.node_lists.is_empty());
    page.state.retained_limit+=1;page.begin_job();
    let nodes=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.query_candidates(0).unwrap();
    assert_eq!(nodes.len(),count);assert_eq!(nodes.capacity(),count);assert_eq!(page.state.retained,baseline);
    drop(nodes);page.state.retained_limit=limit;
}
#[test]
fn browser_live_name_list_borrows_filter_and_compacts_one_admitted_candidate_vector() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body>{}</body></html>","<div name=match></div>".repeat(6000)));
    let Value::Host(handle)=execute(&mut page,&mut doc,&mut forms,"var named=document.getElementsByName('match');named;") else{panic!("Expected name list")};
    let baseline=page.state.retained;let limit=page.state.retained_limit;
    page.state.retained_limit=baseline+6002*size_of::<usize>()-1;page.begin_job();
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.node_list_get(handle,"length").is_err());
    assert_eq!(page.state.retained,baseline);page.state.retained_limit+=1;page.begin_job();
    assert_eq!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.node_list_get(handle,"length").unwrap().to_number(),6000.0);
    assert_eq!(page.state.retained,baseline);page.state.retained_limit=limit;
    execute(&mut page,&mut doc,&mut forms,"document.body.removeChild(named[0]);if(named.length!==5999)throw 'live name removal';");
}

#[test]
fn browser_sparse_and_empty_selector_snapshots_release_unneeded_candidate_backing() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body><p id=only></p>{}</body></html>","<div></div>".repeat(6000)));
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<100;i++){var sparse=document.querySelectorAll('p'),empty=document.querySelectorAll('aside');if(sparse.length!==1||sparse[0]!==document.getElementById('only')||empty.length!==0)throw 'sparse snapshots';}");
    let mut sparse=0;let mut empty=0;
    for list in &page.state.node_lists {
        if let NodeList::Snapshot(nodes)=list {
            if nodes.is_empty(){assert_eq!(nodes.capacity(),0);empty+=1;}
            else {assert_eq!(nodes.len(),1);assert_eq!(nodes.capacity(),1);sparse+=1;}
        }
    }
    assert_eq!((sparse,empty),(100,100));
}
