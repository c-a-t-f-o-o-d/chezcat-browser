//! Original mode classification from the normative identifier tables.
//! HTML reviewed revision 0e292f402d0c3dd17fe00c1bb2ef3a3c94cd8bce.
//! https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode
use super::DocumentMode;
fn starts(text:&str,prefix:&str)->bool{text.get(..prefix.len()).is_some_and(|s|s.eq_ignore_ascii_case(prefix))}
pub(super) fn for_token(name:&str,public:Option<&str>,system:Option<&str>,force:bool)->DocumentMode{
    let public=public.unwrap_or("");
    let legacy_401=starts(public,"-//W3C//DTD HTML 4.01 Frameset//")||starts(public,"-//W3C//DTD HTML 4.01 Transitional//");
    if force||name!="html"||QUIRKS_EXACT.iter().any(|s|public.eq_ignore_ascii_case(s))
        ||system.is_some_and(|s|s.eq_ignore_ascii_case("http://www.ibm.com/data/dtd/v11/ibmxhtml1-transitional.dtd"))
        ||QUIRKS_PREFIX.iter().any(|s|starts(public,s))||legacy_401&&system.is_none_or(str::is_empty){return DocumentMode::Quirks;}
    if starts(public,"-//W3C//DTD XHTML 1.0 Frameset//")||starts(public,"-//W3C//DTD XHTML 1.0 Transitional//")||legacy_401&&system.is_some_and(|s|!s.is_empty()){DocumentMode::LimitedQuirks}else{DocumentMode::NoQuirks}
}
const QUIRKS_EXACT:&[&str]=&[
    "-//W3O//DTD W3 HTML Strict 3.0//EN//",
    "-/W3C/DTD HTML 4.0 Transitional/EN",
    "HTML",
];
const QUIRKS_PREFIX:&[&str]=&[
    "+//Silmaril//dtd html Pro v0r11 19970101//",
    "-//AS//DTD HTML 3.0 asWedit + extensions//",
    "-//AdvaSoft Ltd//DTD HTML 3.0 asWedit + extensions//",
    "-//IETF//DTD HTML 2.0 Level 1//",
    "-//IETF//DTD HTML 2.0 Level 2//",
    "-//IETF//DTD HTML 2.0 Strict Level 1//",
    "-//IETF//DTD HTML 2.0 Strict Level 2//",
    "-//IETF//DTD HTML 2.0 Strict//",
    "-//IETF//DTD HTML 2.0//",
    "-//IETF//DTD HTML 2.1E//",
    "-//IETF//DTD HTML 3.0//",
    "-//IETF//DTD HTML 3.2 Final//",
    "-//IETF//DTD HTML 3.2//",
    "-//IETF//DTD HTML 3//",
    "-//IETF//DTD HTML Level 0//",
    "-//IETF//DTD HTML Level 1//",
    "-//IETF//DTD HTML Level 2//",
    "-//IETF//DTD HTML Level 3//",
    "-//IETF//DTD HTML Strict Level 0//",
    "-//IETF//DTD HTML Strict Level 1//",
    "-//IETF//DTD HTML Strict Level 2//",
    "-//IETF//DTD HTML Strict Level 3//",
    "-//IETF//DTD HTML Strict//",
    "-//IETF//DTD HTML//",
    "-//Metrius//DTD Metrius Presentational//",
    "-//Microsoft//DTD Internet Explorer 2.0 HTML Strict//",
    "-//Microsoft//DTD Internet Explorer 2.0 HTML//",
    "-//Microsoft//DTD Internet Explorer 2.0 Tables//",
    "-//Microsoft//DTD Internet Explorer 3.0 HTML Strict//",
    "-//Microsoft//DTD Internet Explorer 3.0 HTML//",
    "-//Microsoft//DTD Internet Explorer 3.0 Tables//",
    "-//Netscape Comm. Corp.//DTD HTML//",
    "-//Netscape Comm. Corp.//DTD Strict HTML//",
    "-//O'Reilly and Associates//DTD HTML 2.0//",
    "-//O'Reilly and Associates//DTD HTML Extended 1.0//",
    "-//O'Reilly and Associates//DTD HTML Extended Relaxed 1.0//",
    "-//SQ//DTD HTML 2.0 HoTMetaL + extensions//",
    "-//SoftQuad Software//DTD HoTMetaL PRO 6.0::19990601::extensions to HTML 4.0//",
    "-//SoftQuad//DTD HoTMetaL PRO 4.0::19971010::extensions to HTML 4.0//",
    "-//Spyglass//DTD HTML 2.0 Extended//",
    "-//Sun Microsystems Corp.//DTD HotJava HTML//",
    "-//Sun Microsystems Corp.//DTD HotJava Strict HTML//",
    "-//W3C//DTD HTML 3 1995-03-24//",
    "-//W3C//DTD HTML 3.2 Draft//",
    "-//W3C//DTD HTML 3.2 Final//",
    "-//W3C//DTD HTML 3.2//",
    "-//W3C//DTD HTML 3.2S Draft//",
    "-//W3C//DTD HTML 4.0 Frameset//",
    "-//W3C//DTD HTML 4.0 Transitional//",
    "-//W3C//DTD HTML Experimental 19960712//",
    "-//W3C//DTD HTML Experimental 970421//",
    "-//W3C//DTD W3 HTML//",
    "-//W3O//DTD W3 HTML 3.0//",
    "-//WebTechs//DTD Mozilla HTML 2.0//",
    "-//WebTechs//DTD Mozilla HTML//",
];
