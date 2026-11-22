//! Original iterative DOCTYPE token reader over the preprocessed HTML stream.
//! Token diagnostics remain owned by the F04 diagnostic adapter integration.
//! https://html.spec.whatwg.org/multipage/parsing.html#doctype-state
use super::{DocumentTypeData,DocumentMode};

#[derive(Clone,Copy,PartialEq,Eq)]
enum State {BeforeName,Name,AfterName,AfterPublic,BeforePublic,Public(char),AfterPublicId,Between,AfterSystem,BeforeSystem,System(char),AfterSystemId,Bogus}
pub(super) struct Token {pub(super) data:DocumentTypeData,pub(super) mode:DocumentMode,pub(super) consumed:usize}
fn space(ch:char)->bool{matches!(ch,' '| '\t'|'\n'|'\r'|'\u{c}')}
fn push(text:&mut String,ch:char)->Result<(),String>{text.try_reserve(ch.len_utf8()).map_err(|_|"Cannot reserve native doctype token data.")?;text.push(ch);Ok(())}

// input begins immediately after the ASCII case-insensitive DOCTYPE keyword.
pub(super) fn read(input:&str,check:Option<&dyn Fn()->Result<(),String>>)->Result<Token,String>{
    let mut name=String::new();let mut public=None;let mut system=None;
    let mut state=State::BeforeName;let mut force=false;let mut position=0usize;let mut checked=0usize;
    loop {
        if position.saturating_sub(checked)>=4096 {if let Some(check)=check{check()?;}checked=position;}
        let Some(mut ch)=input[position..].chars().next() else {if state!=State::Bogus{force=true;}break;};
        let start=position;position+=ch.len_utf8();
        if ch=='\0'{ch='\u{fffd}';}
        match state {
            State::BeforeName=>{
                if space(ch){continue;}if ch=='>'{force=true;break;}
                push(&mut name,ch.to_ascii_lowercase())?;state=State::Name;
            },
            State::Name=>{if space(ch){state=State::AfterName;}else if ch=='>'{break;}else{push(&mut name,ch.to_ascii_lowercase())?;}},
            State::AfterName=>{
                if space(ch){continue;}if ch=='>'{break;}
                let rest=&input[start..];
                if rest.get(..6).is_some_and(|text|text.eq_ignore_ascii_case("PUBLIC")){position=start+6;state=State::AfterPublic;}
                else if rest.get(..6).is_some_and(|text|text.eq_ignore_ascii_case("SYSTEM")){position=start+6;state=State::AfterSystem;}
                else{force=true;state=State::Bogus;}
            },
            State::AfterPublic|State::BeforePublic=>{
                if space(ch){state=State::BeforePublic;continue;}
                if ch=='>'{force=true;break;}
                if matches!(ch,'"'|'\''){public=Some(String::new());state=State::Public(ch);}else{force=true;state=State::Bogus;}
            },
            State::Public(quote)=>{
                if ch==quote{state=State::AfterPublicId;}else if ch=='>'{force=true;break;}else{push(public.as_mut().unwrap(),ch)?;}
            },
            State::AfterPublicId|State::Between=>{
                if space(ch){state=State::Between;continue;}if ch=='>'{break;}
                if matches!(ch,'"'|'\''){system=Some(String::new());state=State::System(ch);}else{force=true;state=State::Bogus;}
            },
            State::AfterSystem|State::BeforeSystem=>{
                if space(ch){state=State::BeforeSystem;continue;}if ch=='>'{force=true;break;}
                if matches!(ch,'"'|'\''){system=Some(String::new());state=State::System(ch);}else{force=true;state=State::Bogus;}
            },
            State::System(quote)=>{
                if ch==quote{state=State::AfterSystemId;}else if ch=='>'{force=true;break;}else{push(system.as_mut().unwrap(),ch)?;}
            },
            State::AfterSystemId=>{if space(ch){continue;}if ch=='>'{break;}state=State::Bogus;},
            State::Bogus=>{if ch=='>'{break;}},
        }
    }
    let mode=super::document_mode::for_token(&name,public.as_deref(),system.as_deref(),force);
    Ok(Token{data:DocumentTypeData{name:name.into(),public_id:public.unwrap_or_default().into(),system_id:system.unwrap_or_default().into()},mode,consumed:position})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_document_records_token_boundaries_nulls_quotes_and_force_quirks_are_real(){
        for (source,name,public,system,mode) in [
            (" html>","html","","",DocumentMode::NoQuirks),
            ("html>","html","","",DocumentMode::NoQuirks),
            (">tail","","","",DocumentMode::Quirks),
            (" html PUBLIC\"p\"'s'>tail","html","p","s",DocumentMode::NoQuirks),
            (" html SYSTEM 'x>tail","html","","x",DocumentMode::Quirks),
            (" html SYSTEM 'x' trailing","html","","x",DocumentMode::NoQuirks),
            (" html PUBLIC","html","","",DocumentMode::Quirks),
            (" html PUBLIC 'a\0b'>","html","a\u{fffd}b","",DocumentMode::NoQuirks),
            (" html PUBLIC '-//W3C//DTD XHTML 1.0 Transitional//EN'>","html","-//W3C//DTD XHTML 1.0 Transitional//EN","",DocumentMode::LimitedQuirks),
            (" html PUBLIC '-//W3C//DTD HTML 4.01 Transitional//EN' ''>","html","-//W3C//DTD HTML 4.01 Transitional//EN","",DocumentMode::Quirks),
            (" html PUBLIC '-//W3C//DTD HTML 4.01 Transitional//EN' 'native'>","html","-//W3C//DTD HTML 4.01 Transitional//EN","native",DocumentMode::LimitedQuirks),
        ] {
            let token=read(source,None).unwrap();assert_eq!(token.data.name,name,"{source}");assert_eq!(token.data.public_id,public,"{source}");assert_eq!(token.data.system_id,system,"{source}");assert_eq!(token.mode,mode,"{source}");
            assert_eq!(token.consumed,source.find('>').map_or(source.len(),|i|i+1),"{source}");
        }
    }
    #[test]
    fn native_document_records_doctype_token_scan_checks_cancellation_before_publication(){
        let source=format!(" html SYSTEM '{}", "x".repeat(32768));let calls=std::cell::Cell::new(0);
        let check=||{calls.set(calls.get()+1);if calls.get()==2{Err("original cancellation".into())}else{Ok(())}};
        assert!(read(&source,Some(&check)).is_err());assert_eq!(calls.get(),2);
    }
    #[test]
    fn native_document_records_mode_classification_keeps_three_modes_and_case_rules(){
        let classify=super::super::document_mode::for_token;
        assert_eq!(classify("html",Some("-//ietf//dtd html 2.0//en"),None,false),DocumentMode::Quirks);
        assert_eq!(classify("html",Some("unrecognized-public"),None,false),DocumentMode::NoQuirks);
        assert_eq!(classify("html",None,Some("about:legacy-compat"),false),DocumentMode::NoQuirks);
        assert_eq!(classify("html",None,Some("HTTP://WWW.IBM.COM/data/dtd/v11/ibmxhtml1-transitional.dtd"),false),DocumentMode::Quirks);
        assert_eq!(classify("html",None,None,true),DocumentMode::Quirks);
        assert_ne!(DocumentMode::LimitedQuirks,DocumentMode::NoQuirks);
        assert_eq!(DocumentMode::LimitedQuirks.compat_mode(),"CSS1Compat");
    }
}
