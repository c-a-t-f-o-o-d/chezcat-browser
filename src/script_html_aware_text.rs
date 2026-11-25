//! Original HTML-aware descendant text for option labels and text consumers.
//! https://html.spec.whatwg.org/multipage/form-elements.html#get-html-aware-text-content
use super::*;
use crate::html::Namespace;
use script_inert_documents::ImportSource;

impl Bridge<'_> {
    fn html_text_skips_children(&self, object:u32)->Result<bool,String>{
        let (namespace,name)=match self.inert_import_source(object){
            Some(ImportSource::Element(name))=>(Namespace::Html,name),
            Some(ImportSource::Parsed{node:Node{namespace,kind:NodeKind::Element{tag,..},..},..})=>(*namespace,tag.as_str()),
            Some(_)=>return Ok(false),
            None=>{
                let node=node_index(object).filter(|&n|self.dom_accessible(n)).ok_or("HTML-aware text requires an accessible node.")?;
                let item=&self.doc.nodes[node];
                let NodeKind::Element{tag,..}=&item.kind else{return Ok(false)};
                (item.namespace,tag.as_str())
            }
        };
        Ok(matches!((namespace,name),(Namespace::Html|Namespace::Svg,"script")|(Namespace::Html,"img"|"template")))
    }

    // This consumer uses includeAltText=false. Separate template documents and
    // frame documents never become descendants of their host element.
    pub(super) fn html_aware_text_content(&mut self,object:u32)->Result<DomString,String>{
        self.native_node_access(object).map_err(|(_,message)|message)?;
        if self.inert_node_type(object).is_none(){self.state.rebuild(self.doc)?;}
        let mut pending=Vec::new();let mut text=Vec::new();
        script_text_replacement::push_values(self.state,&mut pending,object,0)?;
        while let Some(handle)=pending.pop(){
            self.state.work(1)?;
            if self.html_text_skips_children(handle)? && (handle!=object || self.template_receiver(handle)){continue;}
            let kind=if let Some(kind)=self.inert_node_type(handle){kind}else{
                let node=node_index(handle).filter(|&n|self.dom_accessible(n)).ok_or("HTML-aware text node became unavailable.")?;
                self.check_iframe_child_topology(node)?;
                match self.doc.nodes[node].kind{NodeKind::Text(_)=>3,NodeKind::Cdata(_)=>4,_=>0}
            };
            if matches!(kind,3|4){
                let other=script_text_replacement::bytes(&[&pending],0)?;
                script_text_replacement::push_values(self.state,&mut text,handle,other)?;
                continue;
            }
            let count=if let Some(children)=self.inert_children(handle){children.len()}else{
                self.doc.nodes[node_index(handle).ok_or("Missing HTML-aware text native node.")?].children.len()
            };
            self.state.work(count)?;
            for at in (0..count).rev(){
                let child=if let Some(children)=self.inert_children(handle){children[at]}else{
                    node_handle(self.doc.nodes[node_index(handle).unwrap()].children[at])
                };
                let other=script_text_replacement::bytes(&[&text],0)?;
                script_text_replacement::push_values(self.state,&mut pending,child,other)?;
            }
        }
        drop(pending);
        let mut count=0usize;let mut inspected=0usize;
        for &handle in &text{
            let source=self.character_data_source(handle).ok_or("Missing HTML-aware Text data.")?;
            let bytes=source.logical_bytes();
            self.state.work(bytes)?;
            let source=self.character_data_source(handle).unwrap();
            count=count.checked_add(source.utf16_len()).ok_or("HTML-aware text unit overflow.")?;
            inspected=inspected.checked_add(bytes).ok_or("HTML-aware text inspection overflow.")?;
        }
        self.state.work(inspected.checked_mul(4).ok_or("HTML-aware text work overflow.")?)?;
        let scratch=script_text_replacement::bytes(&[&text],0)?;
        let available=script_text_content::available(self.state,scratch)?;
        let unit_bytes=count.checked_mul(size_of::<u16>()).ok_or("HTML-aware text unit backing overflow.")?;
        if unit_bytes>available{return Err("HTML-aware text scratch storage allowance exhausted.".into());}
        self.state.copies(unit_bytes)?;
        let mut units=Vec::new();units.try_reserve_exact(count).map_err(|_|"Cannot reserve HTML-aware text units.")?;
        let backing=units.capacity().checked_mul(size_of::<u16>()).ok_or("HTML-aware text backing overflow.")?;
        if backing>available{return Err("HTML-aware text scratch storage allowance exhausted.".into());}
        normalize_html_units(self.state,&mut units,text.iter().flat_map(|&handle|self.character_data_source(handle).unwrap().code_units()))?;
        script_text_content::deadline(self.state)?;
        let (projection,raw)=DomString::projected_size(&units)?;
        let logical=if raw{units.len().checked_mul(2).ok_or("HTML-aware text result overflow.")?}else{projection};
        if logical>self.state.limits.host.string_bytes{return Err("HTML-aware text exceeds the host string allowance.".into());}
        self.state.copies(projection.checked_add(if raw{logical}else{0}).ok_or("HTML-aware text result copy overflow.")?)?;
        let budget=available.checked_sub(backing).ok_or("HTML-aware text result storage allowance exhausted.")?
            .min(DomString::storage_bound(self.state.limits.host.string_bytes)?);
        let result=DomString::from_units(&units,budget)?;script_text_content::deadline(self.state)?;Ok(result)
    }
    pub(super) fn html_aware_text_value(&mut self,text:&DomString)->Result<DomString,String>{
        self.state.work(text.logical_bytes().checked_mul(5).ok_or("HTML-aware proposed text work overflow.")?)?;
        let count=text.utf16_len();let available=script_text_content::available(self.state,text.capacity())?;
        let bytes=count.checked_mul(size_of::<u16>()).ok_or("HTML-aware proposed text backing overflow.")?;
        if bytes>available{return Err("HTML-aware proposed text scratch allowance exhausted.".into());}
        self.state.copies(bytes)?;let mut units=Vec::new();units.try_reserve_exact(count).map_err(|_|"Cannot reserve HTML-aware proposed text.")?;
        let backing=units.capacity().checked_mul(size_of::<u16>()).ok_or("HTML-aware proposed text capacity overflow.")?;
        if backing>available{return Err("HTML-aware proposed text scratch allowance exhausted.".into());}
        normalize_html_units(self.state,&mut units,text.code_units())?;
        let (projection,raw)=DomString::projected_size(&units)?;let logical=if raw{units.len().checked_mul(2).ok_or("HTML-aware proposed result overflow.")?}else{projection};
        if logical>self.state.limits.host.string_bytes{return Err("HTML-aware proposed text exceeds host string allowance.".into());}
        self.state.copies(projection.checked_add(if raw{logical}else{0}).ok_or("HTML-aware proposed result copy overflow.")?)?;
        let budget=available.checked_sub(backing).ok_or("HTML-aware proposed result scratch allowance exhausted.")?.min(DomString::storage_bound(self.state.limits.host.string_bytes)?);
        let result=DomString::from_units(&units,budget)?;script_text_content::deadline(self.state)?;Ok(result)
    }

}

fn normalize_html_units(state:&State,units:&mut Vec<u16>,input:impl Iterator<Item=u16>)->Result<(),String>{
    let mut space=false;let mut visited=0usize;
    for unit in input{
        if visited%1024==0{script_text_content::deadline(state)?;}visited+=1;
        if matches!(unit,9|10|12|13|32){space=!units.is_empty();continue;}
        if space{units.push(32);space=false;}units.push(unit);
    }
    script_text_content::deadline(state)
}
