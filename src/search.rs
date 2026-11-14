//! Browser-owned explicit address-bar searches. No suggestions or page hooks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchEngine { #[default] Google, Bing, DuckDuckGo, None }

impl SearchEngine {
    pub const ALL: [Self;4] = [Self::Google,Self::Bing,Self::DuckDuckGo,Self::None];
    pub fn name(self) -> &'static str { match self { Self::Google=>"Google",Self::Bing=>"Bing",Self::DuckDuckGo=>"DuckDuckGo",Self::None=>"None" } }
    pub fn parse(value: &str) -> Result<Self,String> { Self::ALL.into_iter().find(|engine|engine.name()==value).ok_or_else(||"Unknown address-bar search engine; preferences have not been changed.".into()) }
    pub fn hint(self) -> &'static str { match self {Self::Google=>"Search Google or enter address",Self::Bing=>"Search Bing or enter address",Self::DuckDuckGo=>"Search DuckDuckGo or enter address",Self::None=>"Enter address"} }
    fn template(self) -> Option<&'static str> { match self {Self::Google=>Some("https://www.google.com/search?q="),Self::Bing=>Some("https://www.bing.com/search?q="),Self::DuckDuckGo=>Some("https://duckduckgo.com/?q="),Self::None=>None} }
    pub fn query_url(self, query: &str, local_fixture: Option<&str>) -> Result<String,String> {
        let template=self.template().ok_or("Address-bar search is disabled.")?;
        let encoded=crate::forms::encode_query_component(query,crate::profile::MAX_ADDRESS_BYTES)?;
        let address=match local_fixture {Some(endpoint)=>format!("{endpoint}?q={encoded}"),None=>format!("{template}{encoded}")};
        // Enforce the same complete encoded URL bounds as ordinary navigation.
        crate::url::parse(&address).map(|(url,_)|url.display())
    }
}

/// Only the local TSV driver accepts this override, never settings or documents.
pub fn local_fixture(value: &str) -> Result<String,String> {
    let (url,fragment)=crate::url::parse(value)?;
    if url.secure || !matches!(url.host.as_str(),"127.0.0.1"|"::1") || url.query.is_some() || !fragment.is_empty() {
        return Err("Development search fixture requires an HTTP literal loopback URL without query or fragment.".into());
    }
    Ok(url.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn address_search_providers_encode_one_utf8_component_without_tracking_or_fallback() {
        for (engine,prefix) in [(SearchEngine::Google,"https://www.google.com/search?q="),(SearchEngine::Bing,"https://www.bing.com/search?q="),(SearchEngine::DuckDuckGo,"https://duckduckgo.com/?q=")] {
            assert_eq!(engine.query_url("C++ & C# café 🦀 %",None).unwrap(),format!("{prefix}C%2B%2B+%26+C%23+caf%C3%A9+%F0%9F%A6%80+%25"));
            assert!(engine.query_url(&"&".repeat(16*1024),None).is_err());
        }
        assert!(SearchEngine::None.query_url("private text",Some("http://127.0.0.1:1/")).is_err());
        for bad in ["http://localhost/","https://127.0.0.1/","http://127.0.0.2/","http://127.0.0.1/?q=x","http://127.0.0.1/#x","http://user@127.0.0.1/"] {assert!(local_fixture(bad).is_err(),"{bad}");}
        assert_eq!(local_fixture("http://[::1]:3210/search").unwrap(),"http://[::1]:3210/search");
    }
}
