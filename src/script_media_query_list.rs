//! Document-owned CSSOM media-query readback for the supported CSS grammar.
//! https://drafts.csswg.org/cssom-view/#dom-window-matchmedia
use super::*;
use crate::javascript::{MEDIA_QUERY_LIST, MEDIA_QUERY_MATCHES, MEDIA_QUERY_MEDIA};

const BASE: u32 = 0x8b00_0000;
const END: u32 = 0x8c00_0000;

impl Bridge<'_> {
    fn media_query_index(&self, handle: u32) -> Option<usize> {
        let index = handle.checked_sub(BASE).filter(|_| handle < END)? as usize;
        (index < self.state.media_queries.len()).then_some(index)
    }

    pub(super) fn media_query_owner(&self, handle: u32) -> Option<u32> {
        self.media_query_index(handle).map(|_| WINDOW)
    }

    pub(super) fn create_media_query(&mut self, args: &[Value]) -> Result<Value, String> {
        let Some(Value::String(query)) = args.first() else { return Err("matchMedia requires a CSS media query string.".into()); };
        if self.event_target_count() >= self.state.limits.host.event_targets {
            return Err("MediaQueryList target allowance exceeded.".into());
        }
        let index = u32::try_from(self.state.media_queries.len()).map_err(|_| "MediaQueryList identity overflow.")?;
        let handle = BASE.checked_add(index).filter(|&handle| handle < END).ok_or("MediaQueryList identity overflow.")?;
        self.state.work(query.len().saturating_add(1))?;
        self.state.copies(query.len().saturating_mul(3))?;
        if query.len() > self.state.limits.host.string_bytes { return Err("Media query exceeds the host string bound.".into()); }
        let serialized = crate::css::serialize_media_query_list(query);
        if serialized.len() > self.state.limits.host.string_bytes { return Err("Serialized media query exceeds the host string bound.".into()); }
        self.state.media_queries.try_reserve(1).map_err(|_| "Could not reserve MediaQueryList state.")?;
        self.state.replace_bytes(0, size_of::<String>().saturating_add(serialized.capacity()))?;
        self.state.media_queries.push(serialized);
        Ok(Value::Host(handle))
    }

    pub(super) fn media_query_get(&mut self, object: u32, method: &str) -> Result<Value, String> {
        let index = self.media_query_index(object).ok_or("MediaQueryList requires a genuine receiver.")?;
        let len = self.state.media_queries[index].len();
        self.state.work(len.saturating_add(1))?;
        match method {
            MEDIA_QUERY_MEDIA => {
                self.state.copies(len)?;
                Ok(Value::String(self.state.media_queries[index].clone()))
            }
            MEDIA_QUERY_MATCHES => Ok(Value::Bool(crate::css::media_query_list_matches(
                &self.state.media_queries[index], self.doc.media_environment,
            ))),
            _ => Err(format!("{MEDIA_QUERY_LIST} operation is unavailable.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_query_list_uses_the_current_document_viewport_and_palette() {
        let mut doc = crate::html::parse("<html><body><main>Original media fixture</main></body></html>");
        doc.scripting_enabled = true;
        doc.media_environment = Some(crate::css::MediaEnvironment::desktop_viewport(1000, 600, 1.0));
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let result = page.start(&mut doc, &mut forms, &[Script {
            node: 0, label: "original MediaQueryList fixture".into(), source: r#"
var reads=0, wide=window.matchMedia({toString(){reads++;return ' SCREEN and ( MIN-WIDTH : 900PX ) ';}});
var second=matchMedia(wide.media), colors=matchMedia('(forced-colors: active)');
if(reads!==1||wide===second||!(wide instanceof MediaQueryList)||!(wide instanceof EventTarget)||
   Object.getPrototypeOf(wide)!==MediaQueryList.prototype||wide.media!=='screen and (min-width: 900px)'||
   !wide.matches||!second.matches||colors.matches||!matchMedia('(forced-colors: none)').matches||
   matchMedia('(forced-colors)').matches)throw 'native media query result';
if(!matchMedia('').matches||matchMedia('').media!==''||matchMedia('only, screen').media!=='not all, screen'||
   !matchMedia('only, screen').matches||matchMedia('(width: invalid)').matches||
   matchMedia('(width: invalid)').media!=='not all')throw 'valid alternatives or invalid syntax';
var getter=Object.getOwnPropertyDescriptor(MediaQueryList.prototype,'matches'), refused=0;
if(getter.set!==undefined||getter.get.call(wide)!==true||Object.hasOwn(wide,'matches'))throw 'readonly prototype';
for(var run of [function(){window.matchMedia();},function(){window.matchMedia(Symbol());},
   function(){window.matchMedia.call({},'all');},function(){getter.get.call({});},function(){new MediaQueryList();}]){
   try{run();}catch(error){if(error.name==='TypeError'){refused++;continue;}throw error;}
}
if(refused!==5)throw 'invalid receiver';
"#.into(),
        }]);
        assert_eq!(result.errors, 0, "{}", page.report());
        doc.media_environment = Some(crate::css::MediaEnvironment::desktop_viewport(1000, 600, 1.25));
        let result = page.console_eval(&mut doc, &mut forms, r#"
if(wide.matches||second.matches||!matchMedia('(width: 800px)').matches||
   colors.matches||wide.media!=='screen and (min-width: 900px)')throw 'live zoomed CSS viewport';
document.title='media query fixture verified';
"#);
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "media query fixture verified");
    }
}
