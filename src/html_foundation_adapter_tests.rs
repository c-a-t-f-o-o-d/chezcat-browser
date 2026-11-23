//! Explicit development-only adapter over the real parser and DOM host.
//! Corpus parsing, expectations and classification stay in the Python driver.
use super::*;
use std::fmt::Write;

fn decode(text:&str)->Result<String,String> {
    if text.len()%2!=0 {return Err("Odd request hex length.".into());}
    let mut bytes=Vec::new();
    bytes.try_reserve_exact(text.len()/2).map_err(|_|"Request allocation failed.")?;
    for pair in text.as_bytes().chunks_exact(2) {
        let digit=|c:u8|match c {b'0'..=b'9'=>Some(c-b'0'),b'a'..=b'f'=>Some(c-b'a'+10),_=>None};
        bytes.push(digit(pair[0]).and_then(|a|digit(pair[1]).map(|b|a*16+b)).ok_or("Invalid request hex.")?);
    }
    String::from_utf8(bytes).map_err(|_|"Native UTF-8 parser input cannot represent this code-unit stream.".into())
}
fn encode(text:&str)->String {
    let mut out=String::new();
    for byte in text.bytes(){write!(&mut out,"{byte:02x}").unwrap();}
    out
}
fn attribute_name(name:&AttributeName)->Result<String,String> {
    Ok(match name.namespace.first() {
        None=>name.qualified.scalar()?.to_owned(),
        Some(ns)=>{
            let prefix=match ns.uri.scalar().unwrap_or("") {
                "http://www.w3.org/1999/xlink"=>"xlink ",
                "http://www.w3.org/XML/1998/namespace"=>"xml ",
                "http://www.w3.org/2000/xmlns/"=>"xmlns ",
                _=>"unknown-namespace ",
            };
            format!("{prefix}{}",ns.local.scalar()?)
        }
    })
}
fn dump(doc:&Document)->Result<String,String> {
    let mut output=String::new();
    let mut pending=Vec::new();
    pending.try_reserve(doc.nodes.len()).map_err(|_|"Tree dump traversal reservation failed.")?;
    let mut seen=vec![false;doc.nodes.len()];
    for &node in doc.nodes.first().ok_or("Missing native root.")?.children.iter().rev(){pending.push((node,0usize));}
    while let Some((index,depth))=pending.pop() {
        let node=doc.nodes.get(index).ok_or("Native tree has an invalid child index.")?;
        if seen[index] {return Err("Native tree repeats an existing child identity.".into());}
        seen[index]=true;
        let indent="  ".repeat(depth);
        match &node.kind {
            NodeKind::Element {tag,attrs,..}=>{
                let namespace=match node.namespace {
                    Namespace::Html=>"",
                    Namespace::Svg=>"svg ",
                    Namespace::None=>"null-namespace ",
                    Namespace::UnknownForeign=>"unknown-foreign ",
                    Namespace::Other=>return Err("F01: arbitrary-namespace native tree dump schema is unavailable.".into()),
                };
                writeln!(&mut output,"| {indent}<{namespace}{tag}>").unwrap();
                let mut attributes:Vec<_>=attrs.iter().map(|attr|Ok::<_,String>((attribute_name(&attr.name)?,&attr.value))).collect::<Result<Vec<_>,_>>()?;
                attributes.sort_by(|a,b|a.0.encode_utf16().cmp(b.0.encode_utf16()));
                for (name,value) in attributes {writeln!(&mut output,"| {indent}  {name}=\"{value}\"").unwrap();}
            },
            NodeKind::Text(text)=>{writeln!(&mut output,"| {indent}\"{text}\"").unwrap();},
            NodeKind::Cdata(_)=>return Err("F01: XML CDATA tree dump schema is unavailable in this HTML parser adapter.".into()),
            NodeKind::Comment(text)=>{writeln!(&mut output,"| {indent}<!-- {text} -->").unwrap();},
            NodeKind::ProcessingInstruction {target,data}=>{writeln!(&mut output,"| {indent}<?{target} {data}?>").unwrap();},
            NodeKind::DocumentType(data)=>{
                if data.public_id.is_empty()&&data.system_id.is_empty(){writeln!(&mut output,"| {indent}<!DOCTYPE {}>",data.name).unwrap();}
                else{writeln!(&mut output,"| {indent}<!DOCTYPE {} \"{}\" \"{}\">",data.name,data.public_id,data.system_id).unwrap();}
            },
            NodeKind::DocumentFragment=>return Err("Unexpected inner native fragment in parser output.".into()),
        }
        // The native parser's retained template children are the source used
        // by the genuine template-content bridge. This is its corpus notation,
        // not an insertion of missing html/head/body or other DOM nodes.
        let template=node.namespace==Namespace::Html&&matches!(&node.kind,NodeKind::Element {tag,..} if tag=="template");
        if template {writeln!(&mut output,"| {indent}  content").unwrap();}
        for &child in node.children.iter().rev(){pending.push((child,depth+1+usize::from(template)));}
    }
    Ok(output)
}
fn evaluate(lines:&[&str])->Result<(&'static str,String),String> {
    if lines.len()!=7||lines[0]!="CHEZCAT-NATIVE-PARSER-1" {return Err("Malformed native parser request.".into());}
    let kind=lines[1];let entry=lines[2];let scripting=match lines[3] {"on"=>true,"off"=>false,_=>return Err("Invalid scripting mode.".into())};
    let namespace=decode(lines[4])?;let context=decode(lines[5])?;let source=decode(lines[6])?;
    if kind=="capabilities" {
        return Ok(("capabilities",format!("{{\"tree\":true,\"page\":true,\"tokenizer\":false,\"diagnostics\":false,\"script_integration\":false,\"foreign_fragment\":false,\"document_node_guard\":{MAX_LOADED_DOCUMENT_NODES},\"fragment_node_guard\":{MAX_NODES},\"open_element_guard\":{MAX_DEPTH}}}")));
    }
    if kind=="tokenizer" {return Ok(("harness_blocked","F04: standalone token-stream/state/diagnostic adapter entry is unavailable.".into()));}
    if kind=="page" {
        let mut doc=match parse_loaded_document("<!doctype html><html><head></head><body></body></html>",&||Ok(())) {
            Ok(doc)=>doc,Err(error)=>return Ok(("fail",format!("native DOM setup parser refusal: {error}"))),
        };
        doc.scripting_enabled=true;
        let mut forms=crate::forms::Forms::new(&doc);let mut page=crate::scripting::Page::new();
        if page.start(&mut doc,&mut forms,&[]).errors!=0 {return Ok(("fail",page.report()));}
        let result=page.console_eval(&mut doc,&mut forms,&source);
        return if result.errors==0 {Ok(("page",doc.title))}else{Ok(("fail",page.report()))};
    }
    if kind!="tree" {return Err("Unknown native adapter kind.".into());}
    let parsed=match entry {
        "document"=>parse_internal(&source,None,scripting,Some(LoadedTree {nodes:MAX_LOADED_DOCUMENT_NODES,check_cancel:&||Ok(())}),None),
        "fragment"=>{
            if namespace!="http://www.w3.org/1999/xhtml" {return Ok(("harness_blocked","F01/F07: non-HTML fragment context entry is unavailable.".into()));}
            match parse_fragment(&source,&context,scripting) {
                Err(error) if error=="HTML fragment parsing for this element context is not implemented."=>return Ok(("harness_blocked",format!("F07: {error}"))),
                result=>result,
            }
        },
        _=>return Err("Unknown native parser entry.".into()),
    };
    match parsed {
        Ok(doc)=>Ok(match dump(&doc) {Ok(tree)=>("tree",tree),Err(error) if error.starts_with("F01:")=>("harness_blocked",error),Err(error)=>("fail",format!("malformed native tree: {error}"))}),
        Err(error)=>Ok(("fail",error)),
    }
}

fn reply(raw: &str) -> String {
    let lines: Vec<_> = raw.strip_suffix('\n').unwrap_or(raw).split('\n').collect();
    let (status, value) = evaluate(&lines)
        .unwrap_or_else(|error| ("harness_blocked", format!("adapter_request: {error}")));
    format!("CHEZCAT-NATIVE-PARSER-1\n{status}\ndiagnostics-unavailable\n{}\n", encode(&value))
}

fn exchange(request_path: std::ffi::OsString) -> Result<(), String> {
    let root=std::path::PathBuf::from(std::env::var_os("CHEZCAT_VERIFICATION_ROOT").ok_or("Missing verification root.")?)
        .canonicalize().map_err(|e|e.to_string())?;
    let request=std::path::PathBuf::from(request_path)
        .canonicalize().map_err(|e|e.to_string())?;
    if !request.starts_with(&root)||request==root {return Err("Adapter request escaped verification storage.".into());}
    let raw=std::fs::read_to_string(&request).map_err(|e|e.to_string())?;
    std::fs::write(request.with_extension("out"),reply(&raw)).map_err(|e|e.to_string())
}

#[test]
fn native_parser_data_adapter() -> Result<(), String> {
    // Only an explicitly selected worker may consume a filesystem request.
    // The ordinary suite exercises the identical parser and wire response in memory.
    const WORKER: &str = "html::foundation_adapter::native_parser_data_adapter";
    if std::env::args_os().any(|a| a == "--exact") && std::env::args_os().any(|a| a == WORKER) {
        if let Some(request) = std::env::var_os("CHEZCAT_FOUNDATION_REQUEST") {
            return exchange(request);
        }
    }
    let request = format!("CHEZCAT-NATIVE-PARSER-1\ntree\nfragment\noff\n{}\n{}\n{}\n",
        encode("http://www.w3.org/1999/xhtml"), encode("div"), encode("<b a='x'>A&amp;B</b>"));
    let expected = "| <b>\n|   a=\"x\"\n|   \"A&B\"\n";
    let result = reply(&request);
    let lines: Vec<_> = result.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(&lines[..3], &["CHEZCAT-NATIVE-PARSER-1", "tree", "diagnostics-unavailable"]);
    assert_eq!(decode(lines[3])?, expected);
    assert_eq!(reply("malformed\n"), format!("CHEZCAT-NATIVE-PARSER-1\nharness_blocked\ndiagnostics-unavailable\n{}\n",
        encode("adapter_request: Malformed native parser request.")));
    Ok(())
}
