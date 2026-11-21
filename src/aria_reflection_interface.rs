//! ARIAMixin nullable string reflections, excluding Element-reference relations.
//! https://w3c.github.io/aria/#ARIAMixin
macro_rules! attributes {
    ($($property:literal => $attribute:literal),* $(,)?) => {
        pub(crate) const ATTRIBUTES:&[(&str,&str,&str,&str)]=&[$(($property,$attribute,concat!("\0Element.get.",$property),concat!("\0Element.set.",$property))),*];
    }
}
attributes! {
    "role" => "role",
    "ariaAtomic" => "aria-atomic",
    "ariaAutoComplete" => "aria-autocomplete",
    "ariaBrailleLabel" => "aria-braillelabel",
    "ariaBrailleRoleDescription" => "aria-brailleroledescription",
    "ariaBusy" => "aria-busy",
    "ariaChecked" => "aria-checked",
    "ariaColCount" => "aria-colcount",
    "ariaColIndex" => "aria-colindex",
    "ariaColIndexText" => "aria-colindextext",
    "ariaColSpan" => "aria-colspan",
    "ariaCurrent" => "aria-current",
    "ariaDescription" => "aria-description",
    "ariaDisabled" => "aria-disabled",
    "ariaExpanded" => "aria-expanded",
    "ariaHasPopup" => "aria-haspopup",
    "ariaHidden" => "aria-hidden",
    "ariaInvalid" => "aria-invalid",
    "ariaKeyShortcuts" => "aria-keyshortcuts",
    "ariaLabel" => "aria-label",
    "ariaLevel" => "aria-level",
    "ariaLive" => "aria-live",
    "ariaModal" => "aria-modal",
    "ariaMultiLine" => "aria-multiline",
    "ariaMultiSelectable" => "aria-multiselectable",
    "ariaOrientation" => "aria-orientation",
    "ariaPlaceholder" => "aria-placeholder",
    "ariaPosInSet" => "aria-posinset",
    "ariaPressed" => "aria-pressed",
    "ariaReadOnly" => "aria-readonly",
    "ariaRelevant" => "aria-relevant",
    "ariaRequired" => "aria-required",
    "ariaRoleDescription" => "aria-roledescription",
    "ariaRowCount" => "aria-rowcount",
    "ariaRowIndex" => "aria-rowindex",
    "ariaRowIndexText" => "aria-rowindextext",
    "ariaRowSpan" => "aria-rowspan",
    "ariaSelected" => "aria-selected",
    "ariaSetSize" => "aria-setsize",
    "ariaSort" => "aria-sort",
    "ariaValueMax" => "aria-valuemax",
    "ariaValueMin" => "aria-valuemin",
    "ariaValueNow" => "aria-valuenow",
    "ariaValueText" => "aria-valuetext",
}
pub(crate) fn property(name: &str) -> Option<&'static str> {
    ATTRIBUTES
        .iter()
        .find_map(|&(key, attr, _, _)| (name == key).then_some(attr))
}
pub(crate) fn operation(method: &str) -> Option<(&'static str, &'static str, bool)> {
    ATTRIBUTES.iter().find_map(|&(key, attr, get, set)| {
        if method == get {
            Some((key, attr, false))
        } else if method == set {
            Some((key, attr, true))
        } else {
            None
        }
    })
}
