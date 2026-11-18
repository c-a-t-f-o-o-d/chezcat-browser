//! Embedded descriptive build identity. No Git, network, paths or receipts at runtime.
//! A declared tag is build metadata, not proof of publication or authentication.

pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const THIRD_PARTY_NOTICES: &str = include_str!("../THIRD_PARTY_NOTICES.md");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceOrigin { Git, Exported, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeState { Clean, Dirty, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source<'a> {
    revision: Option<&'a str>,
    origin: SourceOrigin,
    tree: TreeState,
}
impl<'a> Source<'a> {
    pub const UNKNOWN: Self = Self {revision:None,origin:SourceOrigin::Unknown,tree:TreeState::Unknown};
    pub fn revision(self)->Option<&'a str> {self.revision}
    pub fn origin(self)->SourceOrigin {self.origin}
    pub fn tree(self)->TreeState {self.tree}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildKind<'a> { Development, DeclaredTag(&'a str) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity<'a> {
    product_version: &'a str,
    source: Source<'a>,
    kind: BuildKind<'a>,
    main_ordinal: Option<u64>,
}

/// Git's full SHA-1 form is the source identity used by the current publication
/// process. Whitespace, placeholders, paths and abbreviations are not identities.
pub fn revision(text: &str) -> Option<&str> {
    (text.len()==40 && text.bytes().all(|byte|byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))).then_some(text)
}
pub fn source<'a>(revision_text:Option<&'a str>,origin:Option<&str>,tree:Option<&str>)->Source<'a> {
    let Some(revision)=revision_text.and_then(revision) else {return Source::UNKNOWN;};
    match (origin,tree) {
        (Some("git"),Some("clean"))=>Source{revision:Some(revision),origin:SourceOrigin::Git,tree:TreeState::Clean},
        (Some("git"),Some("dirty"))=>Source{revision:Some(revision),origin:SourceOrigin::Git,tree:TreeState::Dirty},
        (Some("git"),Some("unknown"))=>Source{revision:Some(revision),origin:SourceOrigin::Git,tree:TreeState::Unknown},
        (Some("exported"),Some("unknown"))=>Source{revision:Some(revision),origin:SourceOrigin::Exported,tree:TreeState::Unknown},
        _=>Source::UNKNOWN,
    }
}
/// A hidden index entry cannot justify a clean-source label. -v marks
/// assume-unchanged entries lowercase; skip-worktree entries use S/s.
pub fn tracked_path(record:&[u8])->Option<&str> {
    let text=std::str::from_utf8(record.strip_prefix(b"H ")?).ok()?;
    if text.is_empty() || text.contains('\r') || text.contains('\n') {return None;}
    std::path::Path::new(text).components().all(|part|matches!(part,std::path::Component::Normal(_))).then_some(text)
}
fn numeric(text:&str)->bool {
    !text.is_empty() && text.bytes().all(|byte|byte.is_ascii_digit()) && (text.len()==1 || !text.starts_with('0'))
}
fn identifiers(text:&str,prerelease:bool)->bool {
    !text.is_empty() && text.split('.').all(|part|!part.is_empty()
        && part.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'-')
        && (!prerelease || !part.bytes().all(|byte|byte.is_ascii_digit()) || numeric(part)))
}
pub fn product_version(text:&str)->bool {
    if text.is_empty() || text.len()>128 || !text.is_ascii() {return false;}
    let (version,build)=text.split_once('+').map_or((text,None),|(version,build)|(version,Some(build)));
    if build.is_some_and(|build|!identifiers(build,false)) {return false;}
    let (core,prerelease)=version.split_once('-').map_or((version,None),|(core,pre)|(core,Some(pre)));
    if prerelease.is_some_and(|pre|!identifiers(pre,true)) {return false;}
    let mut parts=core.split('.');
    (0..3).all(|_|parts.next().is_some_and(numeric)) && parts.next().is_none()
}
impl<'a> Identity<'a> {
    pub fn version(self)->&'a str {self.product_version}
    pub fn source(self)->Source<'a> {self.source}
    pub fn kind(self)->BuildKind<'a> {self.kind}
    pub fn new(product_version:&'a str,source:Source<'a>,declared_tag:Option<&'a str>)->Result<Self,&'static str> {
        if !self::product_version(product_version) {return Err("Invalid product version metadata.");}
        let kind=if let Some(tag)=declared_tag {
            if tag.strip_prefix('v')!=Some(product_version) {return Err("Declared tag must be v plus the exact product version.");}
            if source.revision.is_none() || source.tree==TreeState::Dirty {return Err("A declared tag requires a known source revision without known dirty changes.");}
            BuildKind::DeclaredTag(tag)
        } else {BuildKind::Development};
        Ok(Self{product_version,source,kind,main_ordinal:None})
    }
    /// The build collector supplies this only after proving a complete public
    /// main first-parent history through this exact source revision.
    pub fn with_main_ordinal(mut self,text:Option<&str>)->Self {
        self.main_ordinal=if self.kind==BuildKind::Development && self.source.origin==SourceOrigin::Git
            && self.source.tree==TreeState::Clean && self.source.revision.is_some()
        {text.filter(|value|numeric(value)).and_then(|value|value.parse::<u64>().ok()).filter(|&value|value>0)} else {None};
        self
    }
    pub fn main_ordinal(self)->Option<u64> {self.main_ordinal}
    /// This is a descriptive development label, never a Cargo/PE/license bump.
    /// Existing Cargo build metadata keeps its single SemVer '+' separator.
    pub fn display_version(self)->String {
        if matches!(self.kind,BuildKind::DeclaredTag(_)) {return self.product_version.into();}
        let suffix=match (self.source.revision,self.main_ordinal) {
            (Some(revision),Some(number))=>format!("dev.{number}.g{}",&revision[..12]),
            (Some(revision),None)=>format!("dev.g{}",&revision[..12]),
            (None,_)=>"dev.unknown".into(),
        };
        format!("{}{}{suffix}",self.product_version,if self.product_version.contains('+') {'.'} else {'+'})
    }
    /// This fixed field list deliberately excludes executable/profile paths,
    /// URLs, usernames, timestamps, cookies, history and acceptance receipts.
    pub fn safe_copy(self)->String {
        let kind=match self.kind {
            BuildKind::Development=>if self.main_ordinal.is_some() {"Development build"}else{"Development build (public main ordinal unavailable)"}.to_owned(),
            BuildKind::DeclaredTag(tag)=>format!("Declared tagged build {tag} (publication unverified)"),
        };
        let state=match (self.source.origin,self.source.tree) {
            (SourceOrigin::Git,TreeState::Clean)=>"Git working tree clean at build check",
            (SourceOrigin::Git,TreeState::Dirty)=>"Git working tree dirty at build check",
            (SourceOrigin::Git,TreeState::Unknown)=>"Git working-tree state unknown",
            (SourceOrigin::Exported,_)=>"Exported source revision; modified state unverified",
            _=>"Source provenance unavailable",
        };
        format!("ChezCat {}\nBuild: {kind}\nSource revision: {}\nSource state: {state}\nLicense: CSAPUL {}\nLicense SHA-256: {}\n",self.display_version(),self.source.revision.unwrap_or("unknown"),crate::license_acceptance::LICENSE_VERSION,crate::license_acceptance::LICENSE_SHA256)
    }
}
pub fn current()->Identity<'static> {
    let source=source(option_env!("CHEZCAT_EMBEDDED_SOURCE_REVISION"),option_env!("CHEZCAT_EMBEDDED_SOURCE_ORIGIN"),option_env!("CHEZCAT_EMBEDDED_TREE_STATE"));
    Identity::new(PRODUCT_VERSION,source,option_env!("CHEZCAT_EMBEDDED_DECLARED_TAG").filter(|tag|!tag.is_empty()))
        .unwrap_or(Identity{product_version:PRODUCT_VERSION,source:Source::UNKNOWN,kind:BuildKind::Development,main_ordinal:None})
        .with_main_ordinal(option_env!("CHEZCAT_EMBEDDED_MAIN_ORDINAL"))
}
pub fn license_text()->&'static str {crate::license_acceptance::LICENSE_TEXT}

#[cfg(test)]
mod tests {
    use super::*;
    const SHA:&str="0123456789abcdef0123456789abcdef01234567";
    #[test]
    fn build_identity_revision_refuses_placeholders_abbreviations_paths_and_control_data() {
        assert_eq!(revision(SHA),Some(SHA));
        for invalid in ["$Format:%H$","0123456","0123456789ABCDEF0123456789ABCDEF01234567","../private","C:\\Users\\owner","https://example.invalid/", "0123456789abcdef0123456789abcdef01234567\n"] {assert_eq!(revision(invalid),None,"{invalid:?}");}
    }
    #[test]
    fn build_identity_absence_and_inconsistent_source_states_are_unknown() {
        for args in [(None,None,None),(Some("$Format:%H$"),Some("exported"),Some("unknown")),(Some(SHA),Some("exported"),Some("clean")),(Some(SHA),Some("git"),None)] {assert_eq!(source(args.0,args.1,args.2),Source::UNKNOWN);}
        let exported=source(Some(SHA),Some("exported"),Some("unknown"));assert_eq!(exported.revision,Some(SHA));assert_eq!(exported.tree,TreeState::Unknown);
    }
    #[test]
    fn build_identity_dirty_and_unchecked_git_state_are_never_clean_or_published() {
        for state in ["dirty","unknown"] {
            let identity=Identity::new("2.3.4-alpha.2",source(Some(SHA),Some("git"),Some(state)),None).unwrap();
            assert_ne!(identity.source.tree,TreeState::Clean);assert_eq!(identity.kind,BuildKind::Development);assert!(!identity.safe_copy().contains("working tree clean"));
        }
    }
    #[test]
    fn build_identity_product_preserves_prerelease_build_and_separate_license_values() {
        let identity=Identity::new("2.3.4-alpha.2+original.007",Source::UNKNOWN,None).unwrap();let text=identity.safe_copy();
        assert_eq!(identity.version(),"2.3.4-alpha.2+original.007");
        assert!(text.starts_with("ChezCat 2.3.4-alpha.2+original.007.dev.unknown\n"));assert!(text.contains(&format!("License: CSAPUL {}\n",crate::license_acceptance::LICENSE_VERSION)));assert_eq!(license_text(),crate::license_acceptance::LICENSE_TEXT);assert!(!THIRD_PARTY_NOTICES.is_empty());
        for invalid in ["1.2","01.2.3","1.2.3-alpha.01","1.2.3+","1.2.3-alpha..2","1.2.3\nprivate","v1.2.3","1.2.3+build+other"] {assert!(!product_version(invalid),"{invalid:?}");}
    }
    #[test]
    fn build_identity_declared_tag_is_exact_explicit_and_not_publication_evidence() {
        let clean=source(Some(SHA),Some("git"),Some("clean"));
        let identity=Identity::new("2.3.4-alpha.2",clean,Some("v2.3.4-alpha.2")).unwrap();assert_eq!(identity.kind,BuildKind::DeclaredTag("v2.3.4-alpha.2"));assert!(identity.safe_copy().contains("publication unverified"));
        for tag in ["2.3.4-alpha.2","v2.3.4","v2.3.4-alpha.2\nprivate"] {assert!(Identity::new("2.3.4-alpha.2",clean,Some(tag)).is_err());}
        assert!(Identity::new("2.3.4-alpha.2",Source::UNKNOWN,Some("v2.3.4-alpha.2")).is_err());
        assert!(Identity::new("2.3.4-alpha.2",source(Some(SHA),Some("git"),Some("dirty")),Some("v2.3.4-alpha.2")).is_err());
    }
    #[test]
    fn build_identity_safe_copy_has_only_allowlisted_metadata_fields() {
        let text=Identity::new("2.3.4-alpha.2",source(Some(SHA),Some("git"),Some("clean")),None).unwrap().safe_copy();
        let names:Vec<_>=text.lines().map(|line|line.split_once(':').map_or("product",|(name,_)|name)).collect();assert_eq!(names,["product","Build","Source revision","Source state","License","License SHA-256"]);
        for forbidden in ["C:\\","/Users/","profile","receipt","history","http://","https://","@"] {assert!(!text.contains(forbidden),"{forbidden}");}
    }
    #[test]
    fn build_identity_tracked_inputs_refuse_hidden_flags_paths_and_line_injection() {
        assert_eq!(tracked_path(b"H src/original.rs"),Some("src/original.rs"));
        for record in [b"h src/hidden.rs".as_slice(),b"S src/skipped.rs",b"s src/both.rs",b"H ../private",b"H /private",b"H source\nother",b"H ",b"H src/\xff.rs"] {assert_eq!(tracked_path(record),None,"{record:?}");}
    }
    #[test]
    fn build_identity_development_label_uses_exact_source_and_separate_base_version() {
        let identity=Identity::new("2.3.4-alpha.2",source(Some(SHA),Some("git"),Some("clean")),None).unwrap().with_main_ordinal(Some("17"));
        assert_eq!(identity.version(),"2.3.4-alpha.2");assert_eq!(identity.main_ordinal(),Some(17));
        assert_eq!(identity.display_version(),"2.3.4-alpha.2+dev.17.g0123456789ab");
        let text=identity.safe_copy();assert!(text.starts_with("ChezCat 2.3.4-alpha.2+dev.17.g0123456789ab\n"));
        assert!(text.contains(&format!("Source revision: {SHA}\n")));assert!(text.contains("Build: Development build\n"));
        let metadata=Identity::new("2.3.4-alpha.2+original.007",identity.source(),None).unwrap().with_main_ordinal(Some("17"));
        assert_eq!(metadata.display_version(),"2.3.4-alpha.2+original.007.dev.17.g0123456789ab");assert!(product_version(&metadata.display_version()));
    }
    #[test]
    fn build_identity_unavailable_or_untrusted_ordinals_keep_honest_sha_fallback() {
        let clean=source(Some(SHA),Some("git"),Some("clean"));
        for value in [None,Some(""),Some("0"),Some("01"),Some("-1"),Some("1\n"),Some("18446744073709551616")] {
            let identity=Identity::new("2.3.4",clean,None).unwrap().with_main_ordinal(value);
            assert_eq!(identity.main_ordinal(),None);assert_eq!(identity.display_version(),"2.3.4+dev.g0123456789ab");
            assert!(identity.safe_copy().contains("Build: Development build (public main ordinal unavailable)\n"));
        }
        for provenance in [source(Some(SHA),Some("git"),Some("dirty")),source(Some(SHA),Some("git"),Some("unknown")),source(Some(SHA),Some("exported"),Some("unknown"))] {
            let identity=Identity::new("2.3.4",provenance,None).unwrap().with_main_ordinal(Some("17"));
            assert_eq!(identity.main_ordinal(),None);assert_eq!(identity.source(),provenance);assert_eq!(identity.display_version(),"2.3.4+dev.g0123456789ab");
        }
        let unknown=Identity::new("2.3.4",Source::UNKNOWN,None).unwrap().with_main_ordinal(Some("17"));
        assert_eq!(unknown.display_version(),"2.3.4+dev.unknown");assert!(unknown.safe_copy().contains("Source revision: unknown\n"));
    }
    #[test]
    fn build_identity_exact_declared_tag_keeps_base_and_cannot_gain_development_ordinal() {
        let clean=source(Some(SHA),Some("git"),Some("clean"));
        let identity=Identity::new("2.3.4-alpha.2",clean,Some("v2.3.4-alpha.2")).unwrap().with_main_ordinal(Some("17"));
        assert_eq!(identity.main_ordinal(),None);assert_eq!(identity.display_version(),"2.3.4-alpha.2");
        assert!(identity.safe_copy().starts_with("ChezCat 2.3.4-alpha.2\nBuild: Declared tagged build v2.3.4-alpha.2 (publication unverified)\n"));
        assert!(Identity::new("2.3.4-alpha.2",source(Some(SHA),Some("git"),Some("dirty")),Some("v2.3.4-alpha.2")).is_err());
    }
}
