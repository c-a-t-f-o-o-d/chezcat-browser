//! Live indexed inline CSS declarations and the supported CSSOM member names.
use super::*;

pub(super) const PROPERTIES: &[&str] = &[
    "align-items",
    "align-self",
    "animation-timing-function",
    "background",
    "background-color",
    "background-image",
    "background-position",
    "background-repeat",
    "background-size",
    "border",
    "border-color",
    "border-style",
    "border-width",
    "bottom",
    "box-sizing",
    "color",
    "column-gap",
    "display",
    "flex",
    "flex-basis",
    "flex-direction",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "font",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "gap",
    "height",
    "inset",
    "justify-content",
    "left",
    "line-height",
    "margin",
    "margin-bottom",
    "margin-left",
    "margin-right",
    "margin-top",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "order",
    "overflow",
    "padding",
    "padding-bottom",
    "padding-left",
    "padding-right",
    "padding-top",
    "position",
    "right",
    "row-gap",
    "text-align",
    "text-decoration",
    "text-decoration-line",
    "top",
    "transform",
    "vertical-align",
    "white-space",
    "width",
];

pub(super) fn handle(object: u32) -> bool {
    (STYLE_BASE..DATASET_BASE).contains(&object)
}

pub(super) fn method(name: &str) -> Option<&'static str> {
    match name {
        "item" => Some("item"),
        "getPropertyValue" => Some("getPropertyValue"),
        "setProperty" => Some("setProperty"),
        "removeProperty" => Some("removeProperty"),
        _ => None,
    }
}

pub(super) fn same_name(raw: &str, expected: &str) -> bool {
    if expected == "animation-timing-function" {
        crate::css::easing_trim(raw).eq_ignore_ascii_case(expected)
    } else if expected.starts_with("--") {
        raw.trim() == expected
    } else {
        raw.trim().eq_ignore_ascii_case(expected)
    }
}

fn skip_trivia(mut raw: &str) -> Option<&str> {
    loop {
        raw = raw.trim_start_matches([' ', '\t', '\r', '\n', '\u{c}']);
        let Some(after) = raw.strip_prefix("/*") else {
            return Some(raw);
        };
        raw = after.split_once("*/")?.1;
    }
}

fn declaration(item: &str) -> Option<(&str, &str)> {
    let item = skip_trivia(item)?;
    let end = item.find(|ch: char| ch == ':' || ch == '/' || ch.is_ascii_whitespace())?;
    let value = skip_trivia(&item[end..])?.strip_prefix(':')?;
    Some((&item[..end], value))
}

pub(super) fn winner<'a>(style: &'a str, name: &str) -> Option<(usize, &'a str)> {
    let mut winner = None;
    let mut important = false;
    for (index, item) in crate::css::declaration_items(style).enumerate() {
        let Some((key, raw)) = declaration(item) else {
            continue;
        };
        if !same_name(key, name) {
            continue;
        }
        let clean;
        let value = if raw.contains("/*") {
            clean = crate::css::without_comments(raw);
            &clean
        } else {
            raw
        };
        let (specified, priority) = if name == "animation-timing-function" { crate::css::easing_priority(value) } else { crate::css::declaration_priority(value) };
        if matches!(name, "color" | "background-color")
            && !crate::css::inline_color_value(specified)
        {
            continue;
        }
        if name == "animation-timing-function" && !crate::css::inline_easing_value(specified) { continue; }
        if !important || priority {
            winner = Some((index, raw));
            important = priority;
        }
    }
    winner
}

pub(super) fn replace(
    existing: &str,
    name: &str,
    value: &str,
    priority: &str,
    limit: usize,
) -> Result<Option<String>, String> {
    let winning = winner(existing, name).map(|(index, _)| index);
    if winning.is_none() && value.is_empty() {
        return Ok(None);
    }
    let mut result = String::new();
    let mut append = |text: &str| -> Result<(), String> {
        if result.len().saturating_add(text.len()) > limit {
            return Err("Inline style exceeds the host string limit.".into());
        }
        result
            .try_reserve(text.len())
            .map_err(|_| "Inline style allocation failed.")?;
        result.push_str(text);
        Ok(())
    };
    let serialized = if value.is_empty() {
        String::new()
    } else {
        format!(
            "{name}:{value}{};",
            if priority.is_empty() {
                ""
            } else {
                " !important"
            }
        )
    };
    let mut offset = 0;
    for (index, item) in crate::css::declaration_items(existing).enumerate() {
        offset += item.len();
        let separator = offset < existing.len();
        if separator {
            offset += 1;
        }
        if declaration(item).is_some_and(|(key, _)| same_name(key, name)) {
            if Some(index) == winning {
                append(&serialized)?;
            }
        } else {
            append(item)?;
            if separator {
                append(";")?;
            }
        }
    }
    if winning.is_none() {
        if !existing.is_empty() && !existing.trim_end().ends_with(';') {
            append(";")?;
        }
        append(&serialized)?;
    }
    Ok(Some(result))
}

#[derive(Clone, Copy)]
pub(super) enum EasingText { Specified, Cssom }

pub(super) fn normalize_easing(value: &str, state: &mut State, kind: EasingText) -> Result<Option<String>, String> {
    let (work, copies) = crate::css::easing_source_cost(value.len());
    state.work(work)?;
    state.copies(copies)?;
    Ok(match kind {
        EasingText::Specified => crate::css::specified_easing(value),
        EasingText::Cssom => crate::css::normalize_easing(value),
    })
}

pub(super) fn easing_declaration(style: &str, state: &mut State, kind: EasingText) -> Result<Option<(String, bool)>, String> {
    state.work(1 + style.len().saturating_mul(128))?;
    state.copies(style.len())?;
    let Some((_, raw)) = winner(style, "animation-timing-function") else { return Ok(None); };
    let clean;
    let raw = if raw.contains("/*") { clean = crate::css::without_comments(raw); clean.as_str() } else { raw };
    let (value, important) = crate::css::easing_priority(raw);
    Ok(normalize_easing(value, state, kind)?.map(|value| (value, important)))
}

impl Bridge<'_> {

    pub(super) fn inline_names(&mut self, node: usize) -> Result<Vec<String>, String> {
        let raw = self.doc.attr(node, "style").unwrap_or("");
        if raw.len() > self.state.limits.host.string_bytes {
            return Err("Inline CSS exceeds the string limit.".into());
        }
        // The scanner, canonical property names and bounded cascade ordering
        // are temporary per query. Neither a failed parse nor a read mutates the DOM.
        self.state.work(1 + raw.len().saturating_mul(4))?;
        self.state.copies(raw.len().saturating_mul(4))?;
        let mut meter =
            crate::css::CssBudget::new(&mut self.state.host_work, &mut self.state.copy_work);
        crate::css::inline_declaration_names(raw, PROPERTIES, Some(&mut meter))
    }

    pub(super) fn inline_index(
        &mut self,
        node: usize,
        index: usize,
    ) -> Result<Option<Value>, String> {
        let names = self.inline_names(node)?;
        names.get(index).map(|name| self.string(name)).transpose()
    }

    pub(super) fn inline_keys(
        &mut self,
        object: u32,
        inherited: bool,
    ) -> Result<Vec<String>, String> {
        let node = self.style_node(object)?;
        let count = self.inline_names(node)?.len();
        self.state.work(count)?;
        self.state.copies(count.saturating_mul(4))?; // at most 4096 slots
        let mut keys: Vec<String> = (0..count).map(|index| index.to_string()).collect();
        if inherited {
            let methods = [
                "length",
                "item",
                "getPropertyValue",
                "setProperty",
                "removeProperty",
            ];
            let bytes = PROPERTIES.iter().map(|name| name.len() * 2).sum::<usize>()
                + methods.iter().map(|name| name.len()).sum::<usize>();
            self.state.work(bytes)?;
            self.state.copies(bytes)?;
            for &name in PROPERTIES {
                keys.push(name.to_owned());
                if name.contains('-') {
                    let mut camel = String::with_capacity(name.len());
                    let mut upper = false;
                    for byte in name.bytes() {
                        if byte == b'-' {
                            upper = true;
                            continue;
                        }
                        camel.push(if upper {
                            byte.to_ascii_uppercase() as char
                        } else {
                            byte as char
                        });
                        upper = false;
                    }
                    keys.push(camel);
                }
            }
            keys.extend(methods.into_iter().map(str::to_owned));
        }
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Document, Forms, Page) {
        let mut doc = crate::html::parse("<html><body><div id=tile style='color:red !important; --Tone:13px; background-color:#204060; color:blue; --tone:9px'></div><span id=empty></span></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        (doc, forms, page)
    }

    #[test]
    fn inline_style_enumeration_inherited_members_and_live_own_indices() {
        let (mut doc, mut forms, mut page) = fixture();
        let result = page.console_eval(&mut doc, &mut forms, r#"
var el=document.getElementById('tile'),s=el.style,empty=document.getElementById('empty').style;
var names=[];for(var key in empty)names.push(key);
for(var key of ['backgroundColor','background-color','width','setProperty','item','length']){
    if(names.indexOf(key)<0||!(key in empty)||Object.hasOwn(empty,key)||Object.getOwnPropertyDescriptor(empty,key)!==undefined)throw 'inherited member '+key;
}
if(names.indexOf('madeUpProperty')>=0||Object.keys(empty).length||empty.length!==0||empty.item(0)!==''||empty[0]!==undefined)throw 'empty declaration list';
if(s.length!==4||s[0]!=='color'||s[1]!=='--Tone'||s[2]!=='background-color'||s[3]!=='--tone'||s.item(5)!==''||s[-0]!=='color')throw 'declaration order and custom case';
if(s.item.call(empty,0)!==''||s.item!==s.item||s.item.length!==1)throw 'real item operation';
var d=Object.getOwnPropertyDescriptor(s,'1');
if(d.value!=='--Tone'||d.writable||!d.enumerable||!d.configurable||!Object.hasOwn(s,'1')||Object.keys(s).join(',')!=='0,1,2,3'||Object.getOwnPropertyNames(s).join(',')!=='0,1,2,3')throw 'own numeric descriptors';
var all=[];for(var name in s)all.push(name);
if(all[0]!=='0'||all[3]!=='3'||all.indexOf('--Tone')!==-1||!('2' in s)||'4' in s)throw 'live for-in';
if(s.color!=='red'||s.getPropertyValue('--Tone')!=='13px'||s.getPropertyValue('--tone')!=='9px')throw 'winning case sensitive declarations';
s.backgroundColor='#a0c0e0';if(s[2]!=='background-color'||s.backgroundColor!=='#a0c0e0')throw 'replacement order';
s.setProperty('--Tone','17px');if(s[1]!=='--Tone'||s.getPropertyValue('--tone')!=='9px')throw 'case-sensitive replacement';
s.setProperty('height','12px');if(s[4]!=='height'||s.length!==5)throw 'append';
if(s.removeProperty('COLOR')!=='red'||s[0]!=='--Tone'||s.length!==4||s.color!=='')throw 'remove and compact';
var prior=s[0];s[0]='bad';if(s[0]!==prior||delete s[0]||!delete s[19])throw 'indexed deletion and write';
var caught=false;try{(function(){'use strict';s[0]='bad';})();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'readonly strict index';
var seen=[];for(var k in s){seen.push(k);if(k==='0')s.removeProperty('height');}
if(seen.indexOf('3')!==-1||seen.indexOf('backgroundColor')<0||s.length!==3)throw 'removed index must disappear during iteration';
var added=[];for(var k in empty){added.push(k);if(k==='backgroundColor')empty.setProperty('color','red');}
if(added.indexOf('0')!==-1||empty[0]!=='color')throw 'new index must wait for next iteration';
"#);
        assert_eq!(result.errors, 0, "{}", page.report());
    }

    #[test]
    fn inline_style_enumeration_uses_parsed_winners_and_bounded_shorthands() {
        let names = crate::css::inline_declaration_names(
            "color:red; width:2px; color:blue; margin:1px 2px; margin-left:4px; gap:3px; unknown-thing:1; color:purple!important; color:green; --X:ok; --x:;",
            PROPERTIES, None).unwrap();
        assert_eq!(
            names,
            [
                "width",
                "margin-top",
                "margin-right",
                "margin-bottom",
                "margin-left",
                "row-gap",
                "column-gap",
                "color",
                "--X",
                "--x"
            ]
        );
        let names = crate::css::inline_declaration_names(
            "color:not-a-color; color:; margin:bad; position:potato; --Test:'one;two';",
            PROPERTIES,
            None,
        )
        .unwrap();
        assert_eq!(names, ["--Test"]);
        assert!(
            crate::css::inline_declaration_names("border:1px solid black", PROPERTIES, None)
                .is_err()
        );
    }

    #[test]
    fn inline_style_enumeration_replacement_keeps_unrelated_raw_comments_and_priorities() {
        let raw =
            "--Ink: one !important; /* punctuation :; */ color /**/ : red;   width:2px!important;";
        let result = replace(raw, "color", "blue", "", 256).unwrap().unwrap();
        assert_eq!(
            result,
            "--Ink: one !important;color:blue;   width:2px!important;"
        );
        assert_eq!(
            crate::css::inline_declaration_names(&result, PROPERTIES, None).unwrap(),
            ["--Ink", "color", "width"]
        );
        assert!(replace(raw, "color", "blue", "", 4).is_err());
        assert!(replace(raw, "--ink", "", "", 256).unwrap().is_none());
    }

    #[test]
    fn inline_style_enumeration_failed_budget_or_value_keeps_raw_state_and_indices() {
        let (mut doc, mut forms, mut page) = fixture();
        let node = (0..doc.nodes.len())
            .find(|&n| doc.attr(n, "id") == Some("tile"))
            .unwrap();
        let before = doc.attr(node, "style").unwrap().to_owned();
        let commits = page.state.dom_commits;
        assert_eq!(
            page.console_eval(
                &mut doc,
                &mut forms,
                "document.getElementById('tile').style.setProperty('--Tone','url(unavailable)');"
            )
            .errors,
            1
        );
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        bridge.state.copy_work = 0;
        assert!(bridge.inline_keys(STYLE_BASE + node as u32, true).is_err());
        assert_eq!(bridge.doc.attr(node, "style"), Some(before.as_str()));
        assert_eq!(bridge.state.dom_commits, commits);
        assert_eq!(
            bridge.native_access_owner(STYLE_BASE + node as u32),
            Some(WINDOW)
        );
    }
}
