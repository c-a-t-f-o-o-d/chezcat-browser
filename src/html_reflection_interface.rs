//! Canonical common HTMLElement attribute accessors shared with the native host.
pub(crate) const ATTRIBUTES: &[(&str, &str, &str)] = &[
    (
        "hidden",
        "\0HTMLElement.get.hidden",
        "\0HTMLElement.set.hidden",
    ),
    (
        "title",
        "\0HTMLElement.get.title",
        "\0HTMLElement.set.title",
    ),
    ("lang", "\0HTMLElement.get.lang", "\0HTMLElement.set.lang"),
    ("dir", "\0HTMLElement.get.dir", "\0HTMLElement.set.dir"),
    ("tabIndex", "\0HTMLElement.get.tabIndex", "\0HTMLElement.set.tabIndex"),
];
pub(crate) fn attribute(name:&str)->Option<&'static str> {
    property(name).map(|name|if name=="tabIndex" {"tabindex"}else{name})
}
pub(crate) fn property(name: &str) -> Option<&'static str> {
    ATTRIBUTES
        .iter()
        .find_map(|&(key, _, _)| (name == key).then_some(key))
}
pub(crate) fn operation(method: &str) -> Option<(&'static str, bool)> {
    ATTRIBUTES.iter().find_map(|&(key, get, set)| {
        if method == get {
            Some((key, false))
        } else if method == set {
            Some((key, true))
        } else {
            None
        }
    })
}

// IDL property, null-namespace content attribute, getter and setter.
pub(crate) const META: &[(&str, &str, &str, &str)] = &[
    ("name", "name", "\0HTMLMetaElement.get.name", "\0HTMLMetaElement.set.name"),
    ("httpEquiv", "http-equiv", "\0HTMLMetaElement.get.httpEquiv", "\0HTMLMetaElement.set.httpEquiv"),
    ("content", "content", "\0HTMLMetaElement.get.content", "\0HTMLMetaElement.set.content"),
    ("media", "media", "\0HTMLMetaElement.get.media", "\0HTMLMetaElement.set.media"),
    ("scheme", "scheme", "\0HTMLMetaElement.get.scheme", "\0HTMLMetaElement.set.scheme"),
];

pub(crate) fn meta_attribute(name: &str) -> Option<&'static str> {
    META.iter().find_map(|&(property, attr, _, _)| (name == property).then_some(attr))
}
pub(crate) fn meta_operation(method: &str) -> Option<(&'static str, &'static str, bool)> {
    META.iter().find_map(|&(property, attr, get, set)| {
        (method == get || method == set).then_some((property, attr, method == set))
    })
}
