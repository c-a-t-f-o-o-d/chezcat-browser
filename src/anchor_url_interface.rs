//! URL-valued properties of the native HTMLAnchorElement prototype.
macro_rules! component {
    ($name:literal) => {
        (
            $name,
            concat!("\0HTMLAnchorElement.get.", $name),
            Some(concat!("\0HTMLAnchorElement.set.", $name)),
        )
    };
}
pub(crate) const MEMBERS: &[(&str, &str, Option<&str>)] = &[
    component!("href"),
    ("origin", "\0HTMLAnchorElement.get.origin", None),
    component!("protocol"),
    component!("username"),
    component!("password"),
    component!("host"),
    component!("hostname"),
    component!("port"),
    component!("pathname"),
    component!("search"),
    component!("hash"),
];
pub(crate) fn operation(method: &str) -> Option<(&'static str, bool)> {
    if !method.starts_with("\0HTMLAnchorElement.") {
        return None;
    }
    MEMBERS.iter().find_map(|&(name, get, set)| {
        if method == get {
            Some((name, false))
        } else if set == Some(method) {
            Some((name, true))
        } else {
            None
        }
    })
}
