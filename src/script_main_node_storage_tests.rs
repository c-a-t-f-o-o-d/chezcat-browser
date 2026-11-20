//! Original main node-record backing, producer and stable-handle acceptance.
use super::*;
fn page(markup: &str, spare: usize) -> (Document, Forms, Page) {
    let mut doc=crate::html::parse_loaded_document(markup,&||Ok(())).unwrap();
    let mut nodes=Vec::new();nodes.try_reserve_exact(doc.nodes.len()+spare).unwrap();
    nodes.append(&mut doc.nodes);doc.nodes=nodes;doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}
fn element(doc:&Document,name:&str)->usize{
    doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag==name)).unwrap()
}
fn wide(text:&str,capacity:usize)->String{
    let mut value=String::new();value.try_reserve_exact(capacity).unwrap();value.push_str(text);value
}

#[test]
fn main_node_storage_exact_record_payload_and_movement_refuse_without_capacity_publication() {
    let(mut doc,mut forms,mut page)=page("<html><body><p>kept</p></body></html>",0);
    let text=doc.nodes.iter().position(|node|node.kind.character_data()==Some("kept")).unwrap();
    let before=page.state.retained;let count=doc.nodes.len();let capacity=doc.nodes.capacity();
    assert_eq!(count,capacity);assert_eq!(before,document_bytes(&doc)+page.state.tree_storage_bytes());
    let bundle=size_of::<Node>()+2+size_of::<Option<usize>>()+size_of::<usize>()+64;
    page.state.retained_limit=before+bundle-1;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(bridge.create_dom_node(NodeKind::Comment(wide("x",64))).is_err());
    bridge.state.retained_limit+=1;bridge.state.copy_work=0;
    assert!(bridge.create_dom_node(NodeKind::Comment(wide("x",64))).is_err());
    assert_eq!(bridge.doc.nodes.len(),count);assert_eq!(bridge.doc.nodes.capacity(),capacity);
    assert_eq!(bridge.state.retained,before);assert!(bridge.state.dom.created.is_empty());
    bridge.state.copy_work=(count+1)*(size_of::<Node>()+2+size_of::<Option<usize>>()+size_of::<usize>())+1;
    let Value::Host(created)=bridge.create_dom_node(NodeKind::Comment(wide("x",64))).unwrap()else{panic!("native Comment")};
    assert_eq!(created,node_handle(count));assert_eq!(bridge.doc.nodes.capacity(),count+1);
    assert_eq!(bridge.state.retained,before+bundle);assert_eq!(bridge.state.copy_work,0);
    assert_eq!(bridge.doc.nodes[text].kind.character_data(),Some("kept"));
}

#[test]
fn main_node_storage_paid_spare_slot_uses_no_more_retained_bytes_but_pays_initialization() {
    let(mut doc,mut forms,mut page)=page("<html><body><p>kept</p></body></html>",4);
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_dom_node(NodeKind::DocumentFragment).unwrap();
    let before=page.state.retained;let nodes=doc.nodes.len();let capacity=doc.nodes.capacity();
    page.state.retained_limit=before;page.state.copy_work=size_of::<Node>()+2+size_of::<Option<usize>>()+size_of::<usize>();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.create_dom_node(NodeKind::DocumentFragment).unwrap();
    assert_eq!(bridge.state.retained,before);assert_eq!(bridge.doc.nodes.capacity(),capacity);
    assert_eq!(bridge.doc.nodes.len(),nodes+1);assert_eq!(bridge.state.copy_work,0);
    assert!(bridge.create_dom_node(NodeKind::DocumentFragment).is_err());
    assert_eq!(bridge.doc.nodes.len(),nodes+1);assert_eq!(bridge.state.retained,before);
}

#[test]
fn main_node_storage_every_producer_admits_one_exact_record_with_its_owned_payload() {
    for producer in 0..6 {
        let(mut doc,mut forms,mut page)=page("<html><body><p></p><template></template></body></html>",0);
        let parent=element(&doc,"p");let template=element(&doc,"template");
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        if producer==5 {bridge.template_content(node_handle(template)).unwrap();}
        let nodes=bridge.doc.nodes.len();assert_eq!(nodes,bridge.doc.nodes.capacity());
        let before=bridge.state.retained;
        let payload=match producer {
            0|4=>1,
            1=>3,
            2|3=>1+size_of::<usize>(),
            _=>size_of::<Node>()+2*size_of::<usize>()+1+size_of::<u32>()
                +size_of::<GeneratedNode>()+size_of::<(u32,InertNode)>(),
        };
        // The inert producer owns one generated-source registry slot and one
        // adopted-record slot in addition to the Rc<Node> payload. Their actual slot sizes
        // join the same byte allowance as the main Node record.
        let bundle=size_of::<Node>()+2+size_of::<Option<usize>>()+size_of::<usize>()+payload;
        let operation=|bridge:&mut Bridge<'_>|->Result<(),String>{match producer {
            0=>bridge.create_dom_node(NodeKind::Text("x".into())).map(|_|()),
            1=>bridge.create_image(&[]).map(|_|()),
            2=>bridge.set_text_content(parent,"x".into()),
            3=>bridge.set_inner_html(parent,"<b></b>"),
            4=>bridge.clone_node(node_handle(parent),false).map(|_|()),
            _=>bridge.set_template_html(node_handle(template),"<b></b>"),
        }};
        bridge.state.retained_limit=before+bundle-1;
        assert!(operation(&mut bridge).is_err(),"producer {producer}");
        assert_eq!((bridge.doc.nodes.len(),bridge.doc.nodes.capacity(),bridge.state.retained),(nodes,nodes,before));
        bridge.state.retained_limit+=1;bridge.state.copy_work=bridge.state.limits.copy_bytes;
        operation(&mut bridge).unwrap();
        assert_eq!(bridge.doc.nodes.len(),nodes+1,"producer {producer}");
        assert_eq!(bridge.doc.nodes.capacity(),nodes+1,"producer {producer}");
        assert_eq!(bridge.state.retained,before+bundle,"producer {producer}");
    }
}

#[test]
fn main_node_storage_twelve_thousand_detached_records_keep_initial_node_handles_and_paid_capacity() {
    let(mut doc,mut forms,mut page)=page("<html><body><p>kept</p></body></html>",0);
    let held=doc.nodes.iter().position(|node|node.kind.character_data()==Some("kept")).unwrap();
    let before=page.state.retained;let states=page.state.node_state_bytes()+page.state.node_sets_storage_bytes();let capacity=doc.nodes.capacity();let nodes=doc.nodes.len();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for _ in 0..12_000 {bridge.create_dom_node(NodeKind::DocumentFragment).unwrap();}
    assert_eq!(bridge.doc.nodes.len(),nodes+12_000);
    assert_eq!(bridge.state.retained-before,(bridge.doc.nodes.capacity()-capacity)*size_of::<Node>()+bridge.state.node_state_bytes()+bridge.state.node_sets_storage_bytes()-states);
    assert_eq!(bridge.doc.nodes[held].kind.character_data(),Some("kept"));
    assert!(bridge.state.retained<=bridge.state.retained_limit);
}
