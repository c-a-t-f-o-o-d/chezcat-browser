//! Top-level Window properties live on its realm's actual global object.
use super::*;

const MEMBERS: &[(&str, &str, Option<&str>, bool)] = &[
    ("parent", "\0Window.get.parent", Some("\0Window.set.parent"), true),
    ("top", "\0Window.get.top", None, false),
    ("opener", "\0Window.get.opener", Some("\0Window.set.opener"), true),
    ("frameElement", "\0Window.get.frameElement", None, true),
];

pub(crate) fn operation(method: &str) -> Option<(&'static str, bool)> {
    if !method.starts_with("\0Window.") { return None; }
    MEMBERS.iter().find_map(|&(name, get, set, _)| {
        if method == get { Some((name, false)) }
        else if Some(method) == set { Some((name, true)) }
        else { None }
    })
}

impl Runtime {
    pub(crate) fn bind_top_level_browser_global(&mut self, window: u32) -> Result<(), String> {
        self.bind_browser_global(window)?;
        let (_, properties) = self.browser_global.unwrap();
        for &(name, get, set, configurable) in MEMBERS {
            self.define_host_attribute(properties, window, name, get, set).map_err(Fault::display)?;
            self.objects[properties].properties.get_mut(name).unwrap().configurable = configurable;
            // Use the realm's global-object declaration path. A bare var
            // preserves the accessor; a function can replace a configurable one.
            self.operation_globals.insert(name);
        }
        Ok(())
    }

    pub(super) fn set_window_relationship(&mut self, window: u32, name: &'static str, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        let value = args.first().ok_or_else(|| Fault::exception("TypeError", "A Window setter requires a value."))?;
        // A separately opened App page has no auxiliary opener to disown.
        if name == "opener" && matches!(value, Value::Null) { return Ok(Value::Undefined); }
        let realm = self.window_realms.get(&window).copied().ok_or_else(|| Fault::new("Unknown Window realm."))?;
        let value = self.copy_value(value)?;
        self.with_intrinsic_realm(realm, |runtime| {
            let (owner, properties) = runtime.browser_global.ok_or_else(|| Fault::new("Missing Window property storage."))?;
            if owner != window { return Err(Fault::new("Window relationship receiver differs from its global.")); }
            runtime.create_data_property(&Value::Object(properties), PropertyKey::String(name.into()), value, host)?;
            Ok(Value::Undefined)
        })
    }
}
