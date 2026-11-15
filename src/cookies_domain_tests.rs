use super::*;

const NOW: i64 = 1_700_000_000;

fn url(value: &str) -> HttpUrl {
    crate::url::parse(value).unwrap().0
}

fn header(jar: &Session, value: &str) -> String {
    jar.get_at(&url(value), false, true, false, NOW)
}

#[test]
fn host_only_and_domain_cookies_have_distinct_identity_and_deletion() {
    let jar = Session::default();
    let origin = url("https://example.com/");
    jar.set_at(&origin, "id=host; Path=/", true, NOW).unwrap();
    jar.set_at(&origin, "id=domain; Domain=example.com; Path=/", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://example.com/"), "id=host; id=domain");
    assert_eq!(header(&jar, "https://www.example.com/"), "id=domain");
    jar.set_at(&origin, "id=; Max-Age=0; Path=/", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://example.com/"), "id=domain");
    jar.set_at(&url("https://www.example.com/"), "id=new; Domain=.example.com; Path=/", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://consent.example.com/"), "id=new");
    jar.set_at(&origin, "id=; Domain=example.com; Path=/; Max-Age=0", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://example.com/"), "");
    assert_eq!(header(&jar, "https://www.example.com/"), "");
}

#[test]
fn public_suffix_and_private_tenant_scope_are_rejected_atomically() {
    for (origin, forbidden) in [
        ("https://www.example.com/", "com"),
        ("https://www.example.co.uk/", "co.uk"),
        ("https://alice.github.io/", "github.io"),
        ("https://alice.blogspot.com/", "blogspot.com"),
        ("https://x.a.ck/", "a.ck"),
        ("https://a.example.test/", "test"),
    ] {
        let jar = Session::default();
        let origin = url(origin);
        jar.set_at(&origin, "id=keep; Path=/", true, NOW).unwrap();
        for http in [true, false] {
            for value in ["secret", ""] {
                let assignment = format!("id={value}; Domain={forbidden}; Path=/; Max-Age=0");
                let error = jar.set_at(&origin, &assignment, http, NOW).unwrap_err();
                assert!(!error.contains("secret"));
            }
        }
        assert_eq!(jar.get_at(&origin, false, true, false, NOW), "id=keep");
    }
}

#[test]
fn suffix_host_itself_is_host_only_and_exceptions_can_share() {
    let jar = Session::default();
    for host in ["co.uk", "github.io", "a.ck"] {
        let origin = url(&format!("https://{host}/"));
        jar.set_at(&origin, &format!("id=v; Domain={host}; Path=/"), true, NOW).unwrap();
        assert_eq!(jar.get_at(&origin, false, true, false, NOW), "id=v");
        assert_eq!(header(&jar, &format!("https://child.{host}/")), "");
    }
    let origin = url("https://a.www.ck/");
    jar.set_at(&origin, "exception=yes; Domain=www.ck", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://b.www.ck/"), "exception=yes");
    assert_eq!(header(&jar, "https://other.ck/"), "");
}

#[test]
fn empty_domain_is_ignored_but_malformed_or_foreign_scope_is_not() {
    let jar = Session::default();
    let origin = url("https://www.example.com/");
    jar.set_at(&origin, "empty=yes; Domain=", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://www.example.com/"), "empty=yes");
    assert_eq!(header(&jar, "https://child.www.example.com/"), "");
    for domain in ["evil.com", "com", "child.www.example.com", "..example.com", "example.com.", "example.com:443", "example.com/", "*.example.com"] {
        assert!(jar.set_at(&origin, &format!("bad=v; Domain={domain}"), true, NOW).is_err(), "{domain}");
    }
    jar.set_at(&origin, "last=yes; Domain=evil.com; Domain=.EXAMPLE.COM", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://consent.example.com/"), "last=yes");
}

#[test]
fn ip_literals_and_punycode_never_acquire_an_unsafe_suffix_scope() {
    for (origin, exact, bad, other) in [
        ("http://127.0.0.1/", "127.0.0.1", "0.0.1", "http://127.0.0.2/"),
        ("http://[::1]/", "::1", "1", "http://[::2]/"),
    ] {
        let jar = Session::default();
        let origin = url(origin);
        jar.set_at(&origin, &format!("ip=one; Domain={exact}"), true, NOW).unwrap();
        assert!(jar.set_at(&origin, &format!("bad=x; Domain={bad}"), true, NOW).is_err());
        assert_eq!(jar.get_at(&origin, false, true, false, NOW), "ip=one");
        assert_eq!(header(&jar, other), "");
        assert!(jar.store.lock().unwrap().cookies[0].host_only);
    }
    let jar = Session::default();
    let origin = url("https://www.example.xn--p1ai/");
    assert!(jar.set_at(&origin, "bad=x; Domain=xn--p1ai", true, NOW).is_err());
    jar.set_at(&origin, "id=yes; Domain=example.xn--p1ai", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://other.example.xn--p1ai/"), "id=yes");
}

#[test]
fn secure_overlay_checks_cover_parent_and_child_domains_and_deletion() {
    for (secure_origin, assignment, insecure_origin, attack_domain) in [
        ("https://www.example.com/login", "auth=secret; Secure; Path=/login", "http://evil.example.com/login", "example.com"),
        ("https://example.com/login", "auth=secret; Secure; Domain=example.com; Path=/login", "http://www.example.com/login", "www.example.com"),
    ] {
        let jar = Session::default();
        let secure_origin = url(secure_origin);
        jar.set_at(&secure_origin, assignment, true, NOW).unwrap();
        let insecure_origin = url(insecure_origin);
        for http in [true, false] {
            for path in ["/login", "/login/deep"] {
                for age in ["", "; Max-Age=0"] {
                    let attack = format!("auth=attacker; Domain={attack_domain}; Path={path}{age}");
                    assert!(jar.set_at(&insecure_origin, &attack, http, NOW).is_err());
                }
            }
        }
        assert_eq!(jar.get_at(&secure_origin, false, true, false, NOW), "auth=secret");
        // The specification's path overlay rule is intentionally asymmetric.
        jar.set_at(&insecure_origin, &format!("auth=other; Domain={attack_domain}; Path=/other"), true, NOW).unwrap();
    }
}

#[test]
fn httponly_domain_cookies_cannot_be_replaced_or_deleted_from_sibling_scripts() {
    let jar = Session::default();
    let origin = url("https://www.example.com/");
    let sibling = url("https://consent.example.com/");
    jar.set_at(&origin, "auth=secret; Domain=example.com; Secure; HttpOnly; Path=/", true, NOW).unwrap();
    assert_eq!(jar.get_at(&sibling, false, true, false, NOW), "auth=secret");
    assert_eq!(jar.get_at(&sibling, true, true, false, NOW), "");
    for age in ["", "; Max-Age=0"] {
        assert!(jar.set_at(&sibling, &format!("auth=changed; Domain=example.com; Path=/{age}"), false, NOW).is_err());
    }
    jar.set_at(&sibling, "auth=server; Domain=example.com; Secure; HttpOnly; Path=/", true, NOW).unwrap();
    assert_eq!(jar.get_at(&origin, false, true, false, NOW), "auth=server");
}

#[test]
fn host_prefix_and_secure_none_restrictions_survive_domain_support() {
    let jar = Session::default();
    let origin = url("https://example.com/");
    for text in [
        "__Host-id=v; Secure; Domain=example.com; Path=/",
        "__hOsT-id=v; Secure; Domain=; Path=/",
        "__Host-id=v; Domain=example.com; Path=/",
        "none=v; Domain=example.com; SameSite=None",
    ] {
        assert!(jar.set_at(&origin, text, true, NOW).is_err());
    }
    jar.set_at(&origin, "__Host-id=v; Secure; Path=/", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://www.example.com/"), "");
    assert_eq!(header(&jar, "https://example.com/"), "__Host-id=v");
}

#[test]
fn domain_capacity_is_shared_across_setters_and_updates_remain_atomic() {
    let jar = Session::default();
    for i in 0..MAX_DOMAIN_COOKIES {
        let origin = url(&format!("https://h{i}.example.com/"));
        jar.set_at(&origin, &format!("c{i}=v; Domain=example.com; Path=/"), true, NOW).unwrap();
    }
    let origin = url("https://other.example.com/");
    assert!(jar.set_at(&origin, "overflow=v; Domain=example.com; Path=/", true, NOW).is_err());
    jar.set_at(&origin, "c0=changed; Domain=example.com; Path=/", true, NOW).unwrap();
    assert!(header(&jar, "https://example.com/").starts_with("c0=changed; c1=v"));
    assert_eq!(jar.store.lock().unwrap().cookies.len(), MAX_DOMAIN_COOKIES);
}

#[test]
fn schemeful_sites_separate_tenants_schemes_ips_and_label_boundaries() {
    for (left, right, expected) in [
        ("https://example.com/", "https://www.example.com/", true),
        ("https://www.example.com/", "https://consent.example.com:8443/", true),
        ("https://a.example.co.uk/", "https://b.example.co.uk/", true),
        ("https://a.co.uk/", "https://b.co.uk/", false),
        ("https://alice.github.io/", "https://bob.github.io/", false),
        ("https://a.alice.github.io/", "https://b.alice.github.io/", true),
        ("https://a.blogspot.com/", "https://b.blogspot.com/", false),
        ("https://example.com/", "http://www.example.com/", false),
        ("https://example.com/", "https://badexample.com/", false),
        ("https://example.com/", "https://example.com.evil.test/", false),
        ("http://127.0.0.1/", "http://127.0.0.1:8080/", true),
        ("http://127.0.0.1/", "http://127.0.0.2/", false),
        ("http://[::1]/", "http://[::1]:8080/", true),
        ("http://[::1]/", "http://[::2]/", false),
        ("http://localhost/", "http://child.localhost/", false),
        ("https://a.www.ck/", "https://b.www.ck/", true),
        ("https://x.a.ck/", "https://y.a.ck/", false),
    ] {
        let (left, right) = (url(left), url(right));
        assert_eq!(same_site(&left, &right), expected, "{} / {}", left.display(), right.display());
        assert_eq!(same_site(&right, &left), expected);
    }
    assert!(!url("https://a.example.com/").same_origin(&url("https://b.example.com/")));
}

#[test]
fn domain_path_expiry_and_document_visibility_remain_independent() {
    let jar = Session::default();
    let origin = url("https://www.example.com/a/index");
    jar.set_at(&origin, "path=one; Domain=example.com; Max-Age=2", false, NOW).unwrap();
    jar.set_at(&origin, "root=two; Domain=example.com; Path=/", false, NOW).unwrap();
    assert_eq!(header(&jar, "https://consent.example.com/a/page"), "path=one; root=two");
    assert_eq!(header(&jar, "https://consent.example.com/ab"), "root=two");
    assert_eq!(jar.get_at(&url("https://consent.example.com/a"), true, true, false, NOW + 2), "root=two");
}

#[test]
fn same_site_and_lax_navigation_are_separate_from_cookie_domain_scope() {
    let jar = Session::default();
    let origin = url("https://www.example.com/");
    let sibling = url("https://consent.example.com/");
    for text in [
        "strict=s; Domain=example.com; Path=/; SameSite=Strict",
        "lax=l; Domain=example.com; Path=/; SameSite=Lax",
        "none=n; Domain=example.com; Path=/; SameSite=None; Secure",
        "host=h; Path=/; SameSite=None; Secure",
    ] {
        jar.set_at(&origin, text, true, NOW).unwrap();
    }
    assert_eq!(jar.get_at(&sibling, false, same_site(&origin, &sibling), false, NOW), "strict=s; lax=l; none=n");
    assert_eq!(jar.get_at(&sibling, false, false, true, NOW), "lax=l; none=n");
    assert_eq!(jar.get_at(&sibling, false, false, false, NOW), "none=n");
    assert_eq!(jar.get_at(&url("https://unrelated.com/"), false, true, true, NOW), "");
}

#[test]
fn navigation_context_keeps_siblings_but_taints_cross_site_round_trips() {
    let origin = url("https://www.example.com/");
    let apex = url("https://example.com/");
    let consent = url("https://consent.example.com/");
    let away = url("https://other.example.net/");
    let jar = Session::default();
    jar.set_at(&origin, "strict=s; Domain=example.com; SameSite=Strict; Path=/", true, NOW).unwrap();
    jar.set_at(&origin, "lax=l; Domain=example.com; SameSite=Lax; Path=/", true, NOW).unwrap();
    let mut context = NavigationContext::new(Some(&origin), None);
    context.redirect(&origin, &apex);
    context.redirect(&apex, &consent);
    assert!(context.is_same_site(&consent));
    assert_eq!(jar.get_at(&consent, false, context.is_same_site(&consent), true, NOW), "strict=s; lax=l");
    context.redirect(&consent, &away);
    assert!(!context.is_same_site(&away));
    context.redirect(&away, &origin);
    assert!(!context.is_same_site(&origin));
    assert_eq!(jar.get_at(&origin, false, context.is_same_site(&origin), true, NOW), "lax=l");
    assert_eq!(jar.get_at(&origin, false, context.is_same_site(&origin), false, NOW), "");
}

#[test]
fn navigation_context_preserves_reload_and_missing_initiator_restrictions() {
    let origin = url("https://www.example.com/");
    let sibling = url("https://consent.example.com/");
    let foreign = url("https://foreign.net/");
    assert!(!NavigationContext::new(Some(&origin), Some(false)).is_same_site(&sibling));
    assert!(NavigationContext::new(Some(&origin), Some(true)).is_same_site(&sibling));
    assert!(!NavigationContext::new(Some(&foreign), Some(true)).is_same_site(&sibling));
    assert!(!NavigationContext::new(None, Some(true)).is_same_site(&sibling));
    let mut context = NavigationContext::new(Some(&origin), None);
    context.redirect(&origin, &url("http://www.example.com/"));
    context.redirect(&url("http://www.example.com/"), &origin);
    assert!(!context.is_same_site(&origin));
}

#[test]
fn private_suffix_redirects_never_treat_different_tenants_as_same_site() {
    let alice = url("https://alice.github.io/");
    let bob = url("https://bob.github.io/");
    let mut context = NavigationContext::new(Some(&alice), None);
    assert!(!context.is_same_site(&bob));
    context.redirect(&alice, &bob);
    context.redirect(&bob, &alice);
    assert!(!context.is_same_site(&alice));
}

#[test]
fn private_suffix_cannot_be_skipped_by_choosing_a_still_higher_parent() {
    let jar = Session::default();
    // blogspot.com is private; .com is also public. These nested private
    // suffixes additionally test a higher parent that is NOT itself public.
    for (origin, forbidden) in [
        ("https://tenant.execute-api.af-south-1.amazonaws.com/", "amazonaws.com"),
        ("https://tenant.execute-api.af-south-1.amazonaws.com/", "af-south-1.amazonaws.com"),
        ("https://tenant.a.kawasaki.jp/", "kawasaki.jp"),
    ] {
        assert!(jar.set_at(&url(origin), &format!("bad=v; Domain={forbidden}"), true, NOW).is_err());
    }
    let base = url("https://kawasaki.jp/");
    jar.set_at(&base, "base=yes; Domain=kawasaki.jp", true, NOW).unwrap();
    assert_eq!(header(&jar, "https://kawasaki.jp/"), "base=yes");
    assert_eq!(header(&jar, "https://a.kawasaki.jp/"), "");
}
