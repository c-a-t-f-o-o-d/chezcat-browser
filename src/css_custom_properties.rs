//! Finite unregistered custom properties for boxes, opaque colors and font-size.
//! https://www.w3.org/TR/css-variables-1/
//! Values inherit after substitution. This adds neither CSSOM nor URL loads.
use super::*;
use std::collections::BTreeMap;
use std::rc::Rc;

const MAX_NAMES: usize = 256;
const MAX_MAP_BYTES: usize = 64 * 1024;
const MAX_OUTPUT: usize = 2048;
const MAX_DEPTH: usize = 32;
const MAX_REFERENCES: usize = 4096;

pub(super) struct Budget {
    work: usize, copies: usize,
    used_work: usize, used_copies: usize,
    failed_work: bool, failed_copies: bool,
}
impl Default for Budget {
    fn default() -> Self { Self::with_remaining(8 * 1024 * 1024, 8 * 1024 * 1024) }
}
impl Budget {
    fn with_remaining(work: usize, copies: usize) -> Self {
        Self { work, copies, used_work: 0, used_copies: 0, failed_work: false, failed_copies: false }
    }
    pub(super) fn remaining(&self) -> (usize, usize) { (self.work, self.copies) }
    pub(super) fn operation(&self, work: usize, copies: usize) -> Self {
        Self::with_remaining(self.work.min(work), self.copies.min(copies))
    }
    pub(super) fn usage(&self) -> (usize, usize, bool, bool) {
        (self.used_work, self.used_copies, self.failed_work, self.failed_copies)
    }
    pub(super) fn commit_operation(&mut self, operation: &Self) {
        if operation.failed_work || operation.failed_copies { self.work = 0; self.copies = 0; }
        else { self.work -= operation.used_work; self.copies -= operation.used_copies; }
    }
    fn work(&mut self, amount: usize) -> Result<(), ()> {
        match self.work.checked_sub(amount) {
            Some(remaining)=>{self.used_work += amount; self.work=remaining;Ok(())},
            None=>{self.used_work += self.work; self.failed_work = true; self.work=0;self.copies=0;Err(())},
        }
    }
    fn copy(&mut self, amount: usize) -> Result<(), ()> {
        self.work(amount)?;
        match self.copies.checked_sub(amount) {
            Some(remaining)=>{self.used_copies += amount; self.copies=remaining;Ok(())},
            None=>{self.used_copies += self.copies; self.failed_copies = true; self.work=0;self.copies=0;Err(())},
        }
    }
    fn text(&mut self, text: &str) -> Result<String, ()> { self.copy(text.len())?; Ok(text.to_owned()) }
}

#[derive(Clone, Default)]
pub(super) struct Variables { values: Rc<BTreeMap<String, Option<String>>>, blocked: bool }
enum Raw { Ready(Option<String>), Tokens(String) }
enum Part<'a> { Text(&'a str), Var(&'a str, Option<&'a str>) }

pub(super) fn custom_name(name: &str) -> bool {
    name.len() > 2 && name.len() <= 64 && name.starts_with("--")
        && name[2..].bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'-'|b'_') || b >= 128)
}
fn ident(b: u8) -> bool { b.is_ascii_alphanumeric() || matches!(b,b'_'|b'-') || b >= 128 }

// Balanced token boundaries, strings and fallbacks; escapes outside strings are
// outside this finite identifier subset. Never recognize var-looking string data.
fn parts<'a>(text: &'a str, budget: &mut Budget) -> Result<Vec<Part<'a>>, ()> {
    if text.len() > MAX_OUTPUT { return Err(()); }
    budget.work(text.len())?;
    let bytes = text.as_bytes(); let mut result = Vec::new();
    let mut stack = Vec::new(); let mut i = 0; let mut start = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                let quote = bytes[i]; i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' { i += 1; }
                    i += 1;
                }
                if i >= bytes.len() { return Err(()); } i += 1;
            }
            b'\\' => return Err(()),
            b'(' | b'[' | b'{' => { if stack.len() == MAX_DEPTH { return Err(()); } stack.push(bytes[i]); i += 1; }
            b')' | b']' | b'}' => {
                let expected = match bytes[i] { b')'=>b'(', b']'=>b'[', _=>b'{' };
                if stack.pop() != Some(expected) { return Err(()); } i += 1;
            }
            b if ident(b) => {
                let begin = i; while i < bytes.len() && ident(bytes[i]) { i += 1; }
                if !text[begin..i].eq_ignore_ascii_case("var") || bytes.get(i) != Some(&b'(') { continue; }
                let body = i + 1; let mut end = body; let mut nesting = vec![b'(']; let mut comma = None;
                while end < bytes.len() {
                    match bytes[end] {
                        b'\''|b'"' => {
                            let quote = bytes[end]; end += 1;
                            while end < bytes.len() && bytes[end] != quote {
                                if bytes[end] == b'\\' { end += 1; } end += 1;
                            }
                            if end >= bytes.len() { return Err(()); }
                        }
                        b'\\' => return Err(()),
                        b'('|b'['|b'{' => { if nesting.len() + stack.len() == MAX_DEPTH { return Err(()); } nesting.push(bytes[end]); }
                        b')'|b']'|b'}' => {
                            let expected = match bytes[end] { b')'=>b'(', b']'=>b'[', _=>b'{' };
                            if nesting.pop() != Some(expected) { return Err(()); }
                            if nesting.is_empty() { break; }
                        }
                        b',' if nesting.len() == 1 && comma.is_none() => comma = Some(end),
                        _ => {}
                    }
                    end += 1;
                }
                if end == bytes.len() { return Err(()); }
                let name = text[body..comma.unwrap_or(end)].trim();
                if !custom_name(name) { return Err(()); }
                budget.work(64)?;
                result.push(Part::Text(&text[start..begin]));
                result.push(Part::Var(name, comma.map(|comma| &text[comma+1..end])));
                i = end + 1; start = i;
            }
            _ => i += 1,
        }
    }
    if !stack.is_empty() { return Err(()); }
    budget.work(32)?; result.push(Part::Text(&text[start..])); Ok(result)
}
fn references<'a>(text: &'a str, out: &mut Vec<&'a str>, depth: usize, budget: &mut Budget) -> Result<(), ()> {
    if depth > MAX_DEPTH { return Err(()); }
    for part in parts(text,budget)? {
        if let Part::Var(name,fallback) = part {
            if out.len() == MAX_REFERENCES { return Err(()); }
            budget.work(16)?; out.push(name);
            if let Some(fallback) = fallback { references(fallback,out,depth+1,budget)?; }
        }
    } Ok(())
}
pub(super) fn valid_syntax(text: &str) -> bool {
    references(text,&mut Vec::new(),0,&mut Budget::default()).is_ok()
}
pub(super) fn has_var(text: &str) -> bool {
    parts(text,&mut Budget::default()).is_ok_and(|parts|parts.iter().any(|p|matches!(p,Part::Var(..))))
}

impl Variables {
    pub(super) fn read(&self, name: &str) -> Result<Option<&str>, ComputedStyleUnavailable> {
        if self.blocked { return Err(ComputedStyleUnavailable); }
        Ok(self.values.get(name).and_then(|value| value.as_deref()))
    }
    pub(super) fn cascade<'a>(parent: &Self, declarations: impl Iterator<Item=&'a Declaration>, budget: &mut Budget) -> Self {
        Self::prepare(parent,declarations,budget).unwrap_or_else(|_|Self { blocked:true,..Self::default() })
    }
    fn prepare<'a>(parent: &Self, declarations: impl Iterator<Item=&'a Declaration>, budget: &mut Budget) -> Result<Self,()> {
        if parent.blocked { return Err(()); }
        let mut local = BTreeMap::new();
        for declaration in declarations {
            budget.work(1)?;
            if custom_name(&declaration.name) {
                if !local.contains_key(declaration.name.as_str()) && local.len() == MAX_NAMES { return Err(()); }
                budget.work(64)?;
                local.insert(declaration.name.as_str(),declaration.value.as_str());
            }
        }
        if local.is_empty() { return Ok(parent.clone()); }
        let mut raw = BTreeMap::new(); let mut map_bytes = 0usize;
        for (name,value) in parent.values.iter() {
            map_bytes = map_bytes.checked_add(name.len()+value.as_ref().map_or(0,String::len)+64).ok_or(())?;
            if map_bytes > MAX_MAP_BYTES { return Err(()); }
            raw.insert(budget.text(name)?,Raw::Ready(value.as_deref().map(|v|budget.text(v)).transpose()?));
        }
        for (name,text) in local {
            if !raw.contains_key(name) && raw.len() == MAX_NAMES { return Err(()); }
            let inherited = parent.values.get(name).and_then(|value|value.as_deref());
            let keyword = text.eq_ignore_ascii_case("initial") || text.eq_ignore_ascii_case("inherit") || text.eq_ignore_ascii_case("unset");
            let old = raw.get(name).map_or(0,|value|name.len()+64+match value {Raw::Ready(v)=>v.as_ref().map_or(0,String::len),Raw::Tokens(v)=>v.len()});
            let size = if text.eq_ignore_ascii_case("initial") {0} else if keyword {inherited.map_or(0,str::len)} else {text.len()};
            map_bytes = map_bytes.checked_sub(old).and_then(|bytes|bytes.checked_add(name.len()+64+size)).ok_or(())?;
            if map_bytes > MAX_MAP_BYTES { return Err(()); }
            let value = if text.eq_ignore_ascii_case("initial") { Raw::Ready(None) }
                else if text.eq_ignore_ascii_case("inherit") || text.eq_ignore_ascii_case("unset") {
                    Raw::Ready(inherited.map(|v|budget.text(v)).transpose()?)
                } else { Raw::Tokens(budget.text(text)?) };
            raw.insert(budget.text(name)?,value);
        }
        let entries: Vec<_> = raw.into_iter().collect();
        map_bytes = entries.iter().map(|(n,v)|n.len()+64+match v {Raw::Ready(v)=>v.as_ref().map_or(0,String::len),Raw::Tokens(v)=>v.len()}).sum();
        if map_bytes > MAX_MAP_BYTES { return Err(()); }
        budget.work(entries.len().saturating_mul(64))?;
        let mut edges = vec![Vec::new();entries.len()]; let mut reference_count = 0;
        for (index,(_,value)) in entries.iter().enumerate() {
            if let Raw::Tokens(text) = value {
                let mut refs = Vec::new(); references(text,&mut refs,0,budget)?;
                reference_count += refs.len(); if reference_count > MAX_REFERENCES { return Err(()); }
                for name in refs {
                    budget.work(entries.len())?;
                    if let Ok(target) = entries.binary_search_by(|(key,_)|key.as_str().cmp(name)) { edges[index].push(target); }
                }
            }
        }
        let mut graph = CycleGraph { indices:vec![None;entries.len()], low:vec![0;entries.len()],
            active:vec![false;entries.len()], stack:Vec::new(), next:0 };
        let mut cyclic = vec![false;entries.len()];
        for index in 0..entries.len() { cycles(index,&edges,&mut graph,0,&mut cyclic,budget)?; }
        let mut resolver = Resolver { entries:&entries, cyclic:&cyclic, memo:vec![None;entries.len()], budget };
        let mut values = BTreeMap::new(); let mut bytes = 0;
        for (index,(name,_)) in entries.iter().enumerate() {
            let value = resolver.resolve(index,0)?;
            bytes += name.len()+64+value.as_ref().map_or(0,String::len);
            if bytes > MAX_MAP_BYTES { return Err(()); }
            values.insert(resolver.budget.text(name)?,value);
        }
        Ok(Self {values:Rc::new(values),blocked:false})
    }
    pub(super) fn substitute(&self,text:&str,budget:&mut Budget)->Result<Option<String>,()> {
        if self.blocked { return Err(()); }
        expand(text,0,budget,&mut |name,budget,_|self.values.get(name).and_then(|v|v.as_deref()).map(|v|budget.text(v)).transpose())
    }
}
struct CycleGraph { indices:Vec<Option<usize>>, low:Vec<usize>, active:Vec<bool>, stack:Vec<usize>, next:usize }
// Tarjan's active component stack differs from the recursion stack: a completed
// DFS child can still belong to an overlapping cycle reached by a later child.
fn cycles(index:usize,edges:&[Vec<usize>],graph:&mut CycleGraph,depth:usize,cyclic:&mut[bool],budget:&mut Budget)->Result<(),()> {
    budget.work(1)?;
    if graph.indices[index].is_some() { return Ok(()); }
    if depth == MAX_DEPTH { return Err(()); }
    let ordinal=graph.next; graph.next+=1;
    graph.indices[index]=Some(ordinal); graph.low[index]=ordinal;
    graph.active[index]=true; budget.work(16)?; graph.stack.push(index);
    for &next in &edges[index] {
        budget.work(1)?;
        if graph.indices[next].is_none() {
            cycles(next,edges,graph,depth+1,cyclic,budget)?;
            graph.low[index]=graph.low[index].min(graph.low[next]);
        } else if graph.active[next] {
            graph.low[index]=graph.low[index].min(graph.indices[next].ok_or(())?);
        }
    }
    if graph.low[index] == ordinal {
        let mut component=Vec::new();
        loop {
            budget.work(16)?;
            let member=graph.stack.pop().ok_or(())?;
            graph.active[member]=false; component.push(member);
            if member==index {break;}
        }
        budget.work(edges[index].len())?;
        if component.len()>1 || edges[index].contains(&index) {
            for member in component {cyclic[member]=true;}
        }
    }
    Ok(())
}
struct Resolver<'a,'b> {entries:&'a[(String,Raw)],cyclic:&'a[bool],memo:Vec<Option<Option<String>>>,budget:&'b mut Budget}
impl Resolver<'_,'_> {
    fn resolve(&mut self,index:usize,depth:usize)->Result<Option<String>,()> {
        if depth > MAX_DEPTH { return Err(()); }
        if let Some(value) = &self.memo[index] { return value.as_deref().map(|v|self.budget.text(v)).transpose(); }
        let value = if self.cyclic[index] { None } else { match &self.entries[index].1 {
            Raw::Ready(value)=>value.as_deref().map(|v|self.budget.text(v)).transpose()?,
            Raw::Tokens(text)=> {
                // Split borrowing of immutable source and mutable memo/budget.
                let text = self.budget.text(text)?;
                let mut budget = Budget::with_remaining(0, 0); std::mem::swap(&mut budget,self.budget);
                let result = expand(&text,depth,&mut budget,&mut |name,budget,depth| {
                    let Ok(next)=self.entries.binary_search_by(|(key,_)|key.as_str().cmp(name)) else{return Ok(None);};
                    std::mem::swap(budget,self.budget);
                    let result=self.resolve(next,depth+1);
                    std::mem::swap(budget,self.budget); result
                });
                std::mem::swap(&mut budget,self.budget); result?
            }
        }};
        self.memo[index]=Some(value.as_deref().map(|v|self.budget.text(v)).transpose()?); Ok(value)
    }
}
fn append(out:&mut String,text:&str,budget:&mut Budget)->Result<(),()> {
    if out.len().checked_add(text.len()).ok_or(())? > MAX_OUTPUT { return Err(()); }
    budget.copy(text.len())?; out.push_str(text); Ok(())
}
fn expand(text:&str,depth:usize,budget:&mut Budget,lookup:&mut impl FnMut(&str,&mut Budget,usize)->Result<Option<String>,()>)->Result<Option<String>,()> {
    if depth > MAX_DEPTH { return Err(()); }
    let mut output=String::new();
    for part in parts(text,budget)? { match part {
        Part::Text(text)=>append(&mut output,text,budget)?,
        Part::Var(name,fallback)=> {
            let value=match lookup(name,budget,depth)? {Some(value)=>Some(value),None=>match fallback {
                Some(fallback)=>expand(fallback,depth+1,budget,lookup)?,None=>None,
            }};
            let Some(value)=value else{return Ok(None);};
            // Token substitution cannot manufacture a dimension/function by
            // adjoining source text to a substituted number/identifier.
            append(&mut output," ",budget)?;append(&mut output,&value,budget)?;append(&mut output," ",budget)?;
        }
    }} Ok(Some(output))
}

fn box_property(name:&str)->bool {
    matches!(name,"margin"|"margin-top"|"margin-right"|"margin-bottom"|"margin-left"|
        "padding"|"padding-top"|"padding-right"|"padding-bottom"|"padding-left"|"border"|"border-width"|"border-style"|"border-color"|"background-color"|"background"|"color"|"font-size")
        || ["top","right","bottom","left"].iter().any(|side|
            name==format!("border-{side}") || ["width","style","color"].iter().any(|part|name==format!("border-{side}-{part}")))
}
fn border_value(value:&str)->bool {
    parse_border(value).is_some()
}
fn valid_box(name:&str,value:&str)->bool {
    if name=="font-size" {return font_size_value(value,INITIAL_FONT_SIZE,INITIAL_FONT_SIZE).is_some();}
    if name=="color" {return value=="currentcolor" || color(value).is_some();}
    if matches!(name,"background-color"|"background") {
        return matches!(value,"transparent"|"none"|"currentcolor") || color(value).is_some();
    }
    if matches!(name,"margin"|"padding") {return box_spacing::shorthand(value,name=="margin").is_some();}
    if name.starts_with("margin-") || name.starts_with("padding-") {
        return box_spacing::length(value,name.starts_with("margin-")).is_some();
    }
    if name=="border" || ["top","right","bottom","left"].iter().any(|side|name==format!("border-{side}")) {return border_value(value);}
    let Some(parts)=border_parts(value) else{return false;};
    let valid=|part:&str| if name.ends_with("-width") {border_length(part).is_some()}
        else if name.ends_with("-style") {matches!(part,"none"|"hidden"|"solid")}
        else {border_color_value(part).is_some()};
    let shorthand=matches!(name,"border-width"|"border-style"|"border-color");
    !parts.is_empty() && parts.len()<=if shorthand{4}else{1} && parts.iter().all(|part|valid(part))
}
fn reset_box(style:&mut Style,parent:&Style,name:&str,inherit:bool) {
    let initial=Style::default(); let source=if inherit{parent}else{&initial};
    if name=="color" {style.color=source.color;return;}
    if name=="font-size" {style.font_size=source.font_size;return;}
    if matches!(name,"background-color"|"background") {
        style.background=source.background;
        style.background_current_color=source.background_current_color;
        if name=="background" {
            style.background_clip=source.background_clip;
            style.background_image=source.background_image.clone();
            style.background_repeat=source.background_repeat;
            style.background_position=source.background_position;
            style.background_size=source.background_size;
        }
        return;
    }
    for (side,index) in [("top",0),("right",1),("bottom",2),("left",3)] {
        if name=="margin" || name==format!("margin-{side}") {style.margin[index]=source.margin[index];}
        if name=="padding" || name==format!("padding-{side}") {style.padding[index]=source.padding[index];}
        let entire=name=="border" || name==format!("border-{side}");
        if entire || name=="border-width" || name==format!("border-{side}-width") {style.border_width[index]=source.border_width[index];}
        if entire || name=="border-style" || name==format!("border-{side}-style") {style.border_solid[index]=source.border_solid[index];}
        if entire || name=="border-color" || name==format!("border-{side}-color") {
            style.border_color[index]=if inherit{Some(source.border_color[index].unwrap_or(source.color))}else{source.border_color[index]};
            style.border_alpha[index]=source.border_alpha[index];
        }
    }
}
/// Preserve invalid-at-computed-value behavior while distinguishing a refused
/// finite substitution from a genuinely missing/cyclic or invalid value.
pub(super) fn apply_overflow(style:&mut Style,parent:&Style,declaration:&Declaration,variables:&Variables,budget:&mut Budget,
    known:&mut [bool;2],parent_known:[bool;2])->bool {
    if !overflow::property(&declaration.name) { return false; }
    if !has_var(&declaration.value) {
        overflow::apply_with_availability(style,parent,&declaration.name,declaration.value.trim(),known,parent_known);
        return true;
    }
    let result = variables.substitute(&declaration.value,budget);
    let available = result.is_ok();
    if !result.ok().flatten().is_some_and(|value| overflow::apply_with_availability(style,parent,&declaration.name,value.trim(),known,parent_known)) {
        overflow::apply_with_availability(style,parent,&declaration.name,"initial",known,parent_known);
        if !available { overflow::set_availability(&declaration.name,known,[false;2]); }
    }
    true
}

pub(super) fn apply_box(style:&mut Style,parent:&Style,declaration:&Declaration,variables:&Variables,budget:&mut Budget)->bool {
    if !box_property(&declaration.name) || !has_var(&declaration.value) {return false;}
    // A var()-bearing declaration participates in the cascade before its
    // substituted grammar is known. Failure uses unset (the parent's computed
    // color/font-size for inherited text properties, initial box components); it cannot
    // resurrect an earlier author declaration.
    let substituted=variables.substitute(&declaration.value,budget);
    let available=substituted.is_ok();
    let result=substituted.ok().flatten();
    let value=result.as_deref().unwrap_or("").trim().to_ascii_lowercase();
    let inherited=matches!(declaration.name.as_str(),"color"|"font-size");
    if matches!(value.as_str(),"inherit"|"initial"|"unset") {
        // The finite background shorthand can inherit an existing single image
        // layer. Charge its URL copy; substitution does not introduce URL syntax.
        if value=="inherit" && declaration.name=="background"
            && budget.copy(parent.background_image.as_ref().map_or(0,|image|image.url.len())).is_err() {
            reset_box(style,parent,&declaration.name,false);
            style.background_clip=None;
            return true;
        }
        reset_box(style,parent,&declaration.name,value=="inherit" || (inherited && value=="unset"));
    } else if valid_box(&declaration.name,&value) {
        apply(style,parent,&Declaration {name:declaration.name.clone(),value,important:declaration.important,stylesheet:declaration.stylesheet});
    } else {reset_box(style,parent,&declaration.name,inherited);}
    if !available && declaration.name=="background" {style.background_clip=None;}
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn styles(css:&str,body:&str)->(Document,Vec<Style>) {
        let doc=crate::html::parse(&format!("<html><head><style>{css}</style></head><body>{body}</body></html>"));
        let result=compute(&doc,&HashMap::new());(doc,result)
    }
    fn at<'a>(doc:&Document,styles:&'a[Style],name:&str)->&'a Style {
        &styles[(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()]
    }
    #[test]
    fn custom_box_original_inherited_case_sensitive_padding_and_border_reduction() {
        let(doc,styles)=styles("#theme{--Pad:6px 10px;--pad:99px;--Stroke:2px solid #003366}#panel{width:80px;height:30px;padding:var(--Pad);border:var(--Stroke)}",
            "<div id=theme><div id=panel><a href=next style='display:block;width:20px;height:10px;background:#cc3300'></a></div></div>");
        let panel=at(&doc,&styles,"panel");
        assert_eq!(panel.padding,[Length::Px(6.0),Length::Px(10.0),Length::Px(6.0),Length::Px(10.0)]);
        assert_eq!(panel.border_width,[Length::Px(2.0);4]);assert_eq!(panel.border_solid,[true;4]);assert_eq!(panel.border_color,[Some(0x003366);4]);
    }
    #[test]
    fn custom_box_cascade_finishes_before_use_and_inherits_computed_aliases() {
        let(doc,styles)=styles("#p{--x:4px;--alias:var(--x);--Priority:3px!important}#c{padding:var(--later);--later:var(--alias);--x:9px;border-width:var(--Priority);border-style:solid}#c{--later:var(--alias)}",
            "<div id=p><div id=c style='--Priority:8px'></div><div id=other style='padding:var(--alias)'></div></div>");
        assert_eq!(at(&doc,&styles,"c").padding,[Length::Px(4.0);4]);
        // A child declaration wins over an inherited value regardless of the
        // parent's priority; importance is applied on the declaring element.
        assert_eq!(at(&doc,&styles,"c").border_width,[Length::Px(8.0);4]);
        assert_eq!(at(&doc,&styles,"other").padding,[Length::Px(4.0);4]);
    }
    #[test]
    fn custom_box_same_element_priority_specificity_and_inline_remain_ordered() {
        let(doc,styles)=styles("div{--p:1px}#c{--p:2px!important;--p:7px}#c{padding:var(--p)}",
            "<div id=c style='--p:8px'></div><div id=d style='--p:3px!important;padding:var(--p)'></div>");
        assert_eq!(at(&doc,&styles,"c").padding,[Length::Px(2.0);4]);assert_eq!(at(&doc,&styles,"d").padding,[Length::Px(3.0);4]);
    }
    #[test]
    fn custom_box_cycles_include_unused_fallbacks_and_invalid_dependencies_can_fall_back() {
        let(doc,styles)=styles("#c{--a:var(--b);--b:var(--a);padding:var(--a,5px)}#self{--x:var(--x,9px);padding:var(--x,4px)}#unused{--ok:2px;--x:var(--ok,var(--x));padding:var(--x,6px)}#dependent{--bad:var(--bad);--good:var(--bad,7px);padding:var(--good)}",
            "<div id=c></div><div id=self></div><div id=unused></div><div id=dependent></div>");
        for(name,value)in[("c",5.0),("self",4.0),("unused",6.0),("dependent",7.0)] {assert_eq!(at(&doc,&styles,name).padding,[Length::Px(value);4],"{name}");}
    }
    #[test]
    fn custom_box_overlapping_cycle_invalidates_the_entire_strong_component() {
        let(doc,styles)=styles("#c{--a:var(--b) var(--c);--b:var(--a);--c:var(--b,7px);padding:var(--c,5px)}#outside{--a:var(--b) var(--c);--b:var(--a);--c:var(--b,7px);--d:var(--c,6px);padding:var(--d)}",
            "<div id=c></div><div id=outside></div>");
        assert_eq!(at(&doc,&styles,"c").padding,[Length::Px(5.0);4],"c is part of a->c->b->a, even after DFS finishes b");
        assert_eq!(at(&doc,&styles,"outside").padding,[Length::Px(6.0);4],"a dependent outside the SCC may still use its fallback");
    }
    #[test]
    fn custom_box_empty_values_initial_inherit_unset_and_nested_fallback_are_distinct() {
        let(doc,styles)=styles("#p{--x:3px}#empty{--x:;padding:var(--x,9px)}#initial{--x:initial;padding:var(--x,4px)}#inherit{--x:inherit;padding:var(--x)}#unset{--x:unset;padding:var(--x)}#nested{padding:var(--missing,var(--other,2px 5px));border:var(--missing,2px solid #123456)}#blank{padding:var(--missing,)}",
            "<div id=p><div id=empty></div><div id=initial></div><div id=inherit></div><div id=unset></div><div id=nested></div><div id=blank></div></div>");
        for(name,value)in[("empty",0.0),("initial",4.0),("inherit",3.0),("unset",3.0),("blank",0.0)] {assert_eq!(at(&doc,&styles,name).padding,[Length::Px(value);4],"{name}");}
        assert_eq!(at(&doc,&styles,"nested").padding,[Length::Px(2.0),Length::Px(5.0),Length::Px(2.0),Length::Px(5.0)]);
        assert_eq!(at(&doc,&styles,"nested").border_color,[Some(0x123456);4]);
    }
    #[test]
    fn custom_box_substitution_cannot_join_tokens_or_revive_lower_cascade() {
        let(doc,styles)=styles("#c{--n:2;padding:9px;padding:var(--n)px;padding-left:3px;border:7px solid red;border:var(--missing)}#wrong{--x:solid;padding:8px;padding:var(--x)}#malformed{padding:9px;padding:var(not-custom)}#wide{padding:8px;padding:var(--missing,initial)}",
            "<div id=c></div><div id=wrong></div><div id=malformed></div><div id=wide></div>");
        let c=at(&doc,&styles,"c");assert_eq!(c.padding,[Length::Px(0.0),Length::Px(0.0),Length::Px(0.0),Length::Px(3.0)]);
        assert_eq!(c.border_width,[Length::Px(3.0);4]);assert_eq!(c.border_solid,[false;4]);assert_eq!(c.border_color,[None;4]);
        assert_eq!(at(&doc,&styles,"wrong").padding,[Length::Px(0.0);4]);assert_eq!(at(&doc,&styles,"malformed").padding,[Length::Px(9.0);4]);assert_eq!(at(&doc,&styles,"wide").padding,[Length::Px(0.0);4]);
    }
    #[test]
    fn custom_box_scanner_preserves_quoted_case_and_checks_nested_grammar() {
        let declarations=parse_declarations("--Case:'Mixed var(--missing)';--case:2px;--empty:;");
        let mut budget=Budget::default();let variables=Variables::cascade(&Variables::default(),declarations.iter(),&mut budget);
        assert_eq!(variables.values["--Case"].as_deref(),Some("'Mixed var(--missing)'"));assert_eq!(variables.values["--empty"].as_deref(),Some(""));
        for text in ["var(--x", "var(--x, var(no))", "var(--x])", "var(--x, [)"] {assert!(!valid_syntax(text),"{text}");}
        assert!(valid_syntax("var(--x, rgb(1,2,3))"));assert!(!has_var("'var(--x)'"));
    }
    #[test]
    fn custom_box_depth_count_output_map_and_job_allowances_fail_closed() {
        let decl=Declaration{name:"--x".into(),value:"2px".into(),important:false,stylesheet:None};
        let mut exhausted=Budget::with_remaining(0,0);
        assert!(Variables::cascade(&Variables::default(),[&decl].into_iter(),&mut exhausted).blocked);
        let mut many=String::new();for index in 0..=MAX_NAMES {many.push_str(&format!("--n{index}:1px;"));}
        let declarations=parse_declarations(&many);assert!(Variables::cascade(&Variables::default(),declarations.iter(),&mut Budget::default()).blocked);
        let mut chain=String::new();for index in 0..=MAX_DEPTH+1 {chain.push_str(&format!("--n{index:03}:var(--n{:03});",index+1));}chain.push_str("--n099:1px;");
        let declarations=parse_declarations(&chain);assert!(Variables::cascade(&Variables::default(),declarations.iter(),&mut Budget::default()).blocked);
        let mut doubling=String::from("--n0:1px;");for index in 1..=10 {doubling.push_str(&format!("--n{index}:var(--n{0}) var(--n{0});",index-1));}
        let declarations=parse_declarations(&doubling);assert!(Variables::cascade(&Variables::default(),declarations.iter(),&mut Budget::default()).blocked);
        let mut large=String::new();for index in 0..40 {large.push_str(&format!("--n{index}:{};","x".repeat(2000)));}
        let declarations=parse_declarations(&large);assert!(Variables::cascade(&Variables::default(),declarations.iter(),&mut Budget::default()).blocked);
        let mut budget=Budget::with_remaining(10000,0);assert!(Variables::default().substitute("var(--missing,2px)",&mut budget).is_err());
    }

    #[test]
    fn background_variable_exhausted_and_blocked_allowances_reset_without_resource_growth() {
        let declaration=Declaration{name:"background-color".into(),value:"var(--missing,#0d6efd)".into(),important:false,stylesheet:None};
        let mut style=Style::default();style.background=Some(0xff0000);style.background_current_color=true;style.padding=[Length::Px(7.0);4];style.border_alpha=[51;4];
        assert!(apply_box(&mut style,&Style::default(),&declaration,&Variables::default(),&mut Budget::with_remaining(0,0)));
        assert_eq!(style.background,None);assert!(!style.background_current_color);assert_eq!(style.padding,[Length::Px(7.0);4]);assert_eq!(style.border_alpha,[51;4]);
        let mut many=String::new();for index in 0..=MAX_NAMES {many.push_str(&format!("--n{index}:red;"));}
        let declarations=parse_declarations(&many);let blocked=Variables::cascade(&Variables::default(),declarations.iter(),&mut Budget::default());
        assert!(blocked.blocked);style.background=Some(0xff0000);
        assert!(apply_box(&mut style,&Style::default(),&declaration,&blocked,&mut Budget::default()));assert_eq!(style.background,None);
        let mut parent=Style::default();parent.background=Some(0x112233);
        parent.background_image=Some(BackgroundImage{url:"ParentCase".repeat(128),stylesheet:Some(17)});parent.background_repeat=[false;2];parent.background_size=BackgroundSize::Cover;
        let inherit=Declaration{name:"background".into(),value:"var(--missing,inherit)".into(),important:false,stylesheet:None};
        style.background=Some(0xff0000);style.background_current_color=true;style.background_image=Some(BackgroundImage{url:"OldCase.ppm".into(),stylesheet:Some(9)});
        assert!(apply_box(&mut style,&parent,&inherit,&Variables::default(),&mut Budget::with_remaining(10000,64)));
        assert_eq!(style.background,None);assert!(!style.background_current_color);assert_eq!(style.background_image,None);assert_eq!(style.background_repeat,[true;2]);assert_eq!(style.background_size,BackgroundSize::Auto);
        assert_eq!(parent.background_image.as_ref().unwrap().url,"ParentCase".repeat(128));
    }

    #[test]
    fn color_variable_consumer_limits_inherit_without_mutating_other_style_components() {
        let declaration=Declaration{name:"color".into(),value:"var(--missing,#ffffff)".into(),important:false,stylesheet:None};
        let mut parent=Style::default();parent.color=0x112233;
        let mut style=Style::default();style.color=0xff0000;style.background=Some(0x0d6efd);style.background_current_color=true;
        style.background_image=Some(BackgroundImage{url:"KeptCase.ppm".into(),stylesheet:Some(17)});style.background_size=BackgroundSize::Contain;
        style.padding=[Length::Px(7.0);4];style.border_color=[Some(0);4];style.border_alpha=[51;4];style.font_families=vec!["monospace".into()];style.font_size=19.0;
        let snapshot=style.clone();
        assert!(apply_box(&mut style,&parent,&declaration,&Variables::default(),&mut Budget::with_remaining(0,0)));assert_eq!(style.color,0x112233);
        let mut many=String::new();for index in 0..=MAX_NAMES {many.push_str(&format!("--n{index}:red;"));}
        let declarations=parse_declarations(&many);let blocked=Variables::cascade(&Variables::default(),declarations.iter(),&mut Budget::default());assert!(blocked.blocked);
        style.color=0xff0000;assert!(apply_box(&mut style,&parent,&declaration,&blocked,&mut Budget::default()));assert_eq!(style.color,0x112233,"Blocked maps cannot revive author red or use an unbudgeted white fallback");
        style.color=0xff0000;assert!(apply_box(&mut style,&parent,&declaration,&Variables::default(),&mut Budget::with_remaining(10000,0)));assert_eq!(style.color,0x112233);
        assert_eq!(style.background,snapshot.background);assert_eq!(style.background_current_color,snapshot.background_current_color);assert_eq!(style.background_image,snapshot.background_image);assert_eq!(style.background_size,snapshot.background_size);
        assert_eq!(style.padding,snapshot.padding);assert_eq!(style.border_color,snapshot.border_color);assert_eq!(style.border_alpha,snapshot.border_alpha);assert_eq!(style.font_families,snapshot.font_families);assert_eq!(style.font_size,snapshot.font_size);
        assert_eq!(parent.color,0x112233);
    }

    #[test]
    fn font_size_variable_existing_allowances_inherit_without_growing_other_style_state() {
        let declaration=Declaration{name:"font-size".into(),value:"var(--missing,24px)".into(),important:false,stylesheet:None};
        let mut parent=Style::default();parent.font_size=18.0;
        let mut style=Style::default();style.font_size=99.0;style.root_font_size=32.0;style.font_families=vec!["Geometry".into()];style.font_weight=700;style.italic=true;style.line_height=LineHeight::Factor(2.0);
        style.color=0x123456;style.background=Some(0x0d6efd);style.background_image=Some(BackgroundImage{url:"KeptCase.ppm".into(),stylesheet:Some(17)});style.padding=[Length::Px(7.0);4];style.border_alpha=[51;4];
        let snapshot=style.clone();
        for mut budget in [Budget::with_remaining(0,0),Budget::with_remaining(10000,0)] {
            style.font_size=99.0;assert!(apply_box(&mut style,&parent,&declaration,&Variables::default(),&mut budget));assert_eq!(style.font_size,18.0);
        }
        let mut many=String::new();for index in 0..=MAX_NAMES {many.push_str(&format!("--n{index}:24px;"));}
        let declarations=parse_declarations(&many);let blocked=Variables::cascade(&Variables::default(),declarations.iter(),&mut Budget::default());assert!(blocked.blocked);
        style.font_size=99.0;assert!(apply_box(&mut style,&parent,&declaration,&blocked,&mut Budget::default()));assert_eq!(style.font_size,18.0,"Blocked variable maps cannot revive author99px or an unbudgeted24px fallback");
        assert_eq!(style.root_font_size,snapshot.root_font_size);assert_eq!(style.font_families,snapshot.font_families);assert_eq!(style.font_weight,snapshot.font_weight);assert_eq!(style.italic,snapshot.italic);assert!(matches!(style.line_height,LineHeight::Factor(value)if value==2.0));
        assert_eq!(style.color,snapshot.color);assert_eq!(style.background,snapshot.background);assert_eq!(style.background_image,snapshot.background_image);assert_eq!(style.padding,snapshot.padding);assert_eq!(style.border_alpha,snapshot.border_alpha);assert_eq!(parent.font_size,18.0);
    }
}
