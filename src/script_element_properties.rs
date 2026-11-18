//! Ordinary expando families and reserved native interface names.
//! Custom keys use VM descriptors; native interfaces keep host routing.
//! https://dom.spec.whatwg.org/#interface-element
//! https://html.spec.whatwg.org/multipage/dom.html#htmlelement
use super::*;
const COMMON: &str = "
constructor __proto__ toString toLocaleString valueOf prototype setCapture releaseCapture
nodeType nodeName baseURI isConnected ownerDocument parentNode parentElement childNodes firstChild lastChild previousSibling nextSibling nodeValue textContent getRootNode
hasChildNodes normalize cloneNode isEqualNode isSameNode compareDocumentPosition contains lookupPrefix lookupNamespaceURI isDefaultNamespace insertBefore appendChild replaceChild removeChild
addEventListener removeEventListener dispatchEvent namespaceURI prefix localName tagName id className classList slot assignedSlot attributes hasAttributes getAttributeNames
getAttribute getAttributeNS setAttribute setAttributeNS removeAttribute removeAttributeNS toggleAttribute hasAttribute hasAttributeNS getAttributeNode getAttributeNodeNS setAttributeNode setAttributeNodeNS removeAttributeNode
attachShadow shadowRoot attachInternals getHTML setHTML setHTMLUnsafe innerHTML outerHTML matches webkitMatchesSelector closest getElementsByTagName getElementsByTagNameNS getElementsByClassName querySelector querySelectorAll
children firstElementChild lastElementChild childElementCount previousElementSibling nextElementSibling before after replaceWith remove prepend append replaceChildren insertAdjacentHTML insertAdjacentText insertAdjacentElement
getBoundingClientRect getClientRects getBoxQuads getAnimations animate checkVisibility scrollTop scrollLeft scrollWidth scrollHeight clientTop clientLeft clientWidth clientHeight scroll scrollTo scrollBy scrollIntoView
offsetParent offsetTop offsetLeft offsetWidth offsetHeight setPointerCapture releasePointerCapture hasPointerCapture requestFullscreen requestPointerLock
title lang translate dir hidden inert accessKey accessKeyLabel autocapitalize autocorrect autofocus tabIndex contentEditable isContentEditable enterKeyHint inputMode draggable spellcheck writingSuggestions nonce
style attributeStyleMap computedStyleMap dataset innerText outerText click focus blur popover showPopover hidePopover togglePopover
headingOffset headingReset part
";
const HANDLERS: &str = "onabort onauxclick onbeforeinput onbeforematch onbeforetoggle onblur oncancel oncanplay oncanplaythrough onchange onclick onclose oncommand oncontextlost oncontextmenu oncontextrestored oncopy oncuechange oncut ondblclick ondrag ondragend ondragenter ondragleave ondragover ondragstart ondrop ondurationchange onemptied onended onerror onfocus onformdata oninput oninvalid onkeydown onkeypress onkeyup onload onloadeddata onloadedmetadata onloadstart onmousedown onmouseenter onmouseleave onmousemove onmouseout onmouseover onmouseup onpaste onpause onplay onplaying onprogress onratechange onreset onresize onscroll onscrollend onsecuritypolicyviolation onseeked onseeking onselect onselectionchange onselectstart onslotchange onstalled onsubmit onsuspend ontimeupdate ontoggle onvolumechange onwaiting onwheel onanimationcancel onanimationend onanimationiteration onanimationstart ontransitioncancel ontransitionend ontransitionrun ontransitionstart ongotpointercapture onlostpointercapture onpointercancel onpointerdown onpointerenter onpointerleave onpointermove onpointerout onpointerover onpointerrawupdate onpointerup ontouchcancel ontouchend ontouchmove ontouchstart onfullscreenchange onfullscreenerror onwebkitanimationend onwebkitanimationiteration onwebkitanimationstart onwebkittransitionend";
const ARIA: &str = "role ariaAtomic ariaAutoComplete ariaBrailleLabel ariaBrailleRoleDescription ariaBusy ariaChecked ariaColCount ariaColIndex ariaColIndexText ariaColSpan ariaControlsElements ariaCurrent ariaDescribedByElements ariaDescription ariaDetailsElements ariaDisabled ariaErrorMessageElements ariaExpanded ariaFlowToElements ariaHasPopup ariaHidden ariaInvalid ariaKeyShortcuts ariaLabel ariaLabelledByElements ariaLevel ariaLive ariaModal ariaMultiLine ariaMultiSelectable ariaOrientation ariaOwnsElements ariaPlaceholder ariaPosInSet ariaPressed ariaReadOnly ariaRelevant ariaRequired ariaRoleDescription ariaRowCount ariaRowIndex ariaRowIndexText ariaRowSpan ariaSelected ariaSetSize ariaSort ariaValueMax ariaValueMin ariaValueNow ariaValueText ariaActiveDescendantElement ariaNotify";
const URL: &str = "href origin protocol username password host hostname port pathname search hash target download ping rel relList hreflang type referrerPolicy";
const VALIDATION: &str = "form name type value disabled willValidate validity validationMessage labels checkValidity reportValidity setCustomValidity";
const EDITING: &str = "defaultValue value selectionDirection selectionEnd selectionStart select setRangeText setSelectionRange maxLength minLength placeholder readOnly required autocomplete dirName";
pub(super) fn ordinary_interface_member(name: &str) -> bool {
    matches!(name, "toString" | "constructor" | "__proto__")
}

fn contains(names: &str, name: &str) -> bool {
    names
        .split_ascii_whitespace()
        .any(|candidate| candidate == name)
}
fn specific(tag: &str) -> &'static str {
    match tag {
        "html" => "version",
        "head" => "profile",
        "body" => "text link vLink aLink bgColor background onafterprint onbeforeprint onbeforeunload onhashchange onlanguagechange onmessage onmessageerror onoffline ononline onpagehide onpagereveal onpageshow onpageswap onpopstate onrejectionhandled onstorage onunhandledrejection onunload",
        "iframe" => "src srcdoc sandbox contentWindow contentDocument referrerPolicy allow allowFullscreen credentialless loading name width height align frameBorder longDesc marginHeight marginWidth scrolling csp browsingTopics privateToken featurePolicy getSVGDocument",
        "img" => "alt src srcset sizes crossOrigin useMap isMap width height naturalWidth naturalHeight complete currentSrc referrerPolicy decoding loading fetchPriority decode name lowsrc longDesc align hspace vspace border",
        "a" => "text coords charset name rev shape",
        "area" => "alt coords shape noHref",
        "input" => "accept alt capture checked defaultChecked files height width src indeterminate list max min multiple pattern size step valueAsDate valueAsNumber useMap webkitdirectory webkitEntries stepDown stepUp showPicker formAction formEnctype formMethod formNoValidate formTarget popoverTargetElement popoverTargetAction alpha colorSpace align",
        "button" => "formAction formEnctype formMethod formNoValidate formTarget popoverTargetElement popoverTargetAction command commandForElement",
        "label" => "control form htmlFor",
        "textarea" => "cols rows wrap textLength",
        "script" => "src type noModule async defer crossOrigin text integrity referrerPolicy blocking fetchPriority charset event htmlFor readyState",
        "link" => "disabled href crossOrigin rel relList media hreflang type as referrerPolicy sizes imageSrcset imageSizes integrity blocking fetchPriority color charset rev target sheet",
        "style" => "disabled media type sheet blocking",
        "meta" => "name httpEquiv content media charset scheme",
        "title" => "text",
        "ol" => "reversed start type compact",
        "ul" => "type compact",
        "li" => "value type",
        "dl" => "compact",
        "blockquote" => "cite",
        "p"|"div"|"h1"|"h2"|"h3"|"h4"|"h5"|"h6" => "align",
        "br" => "clear",
        "hr" => "align color noShade size width",
        "pre" => "width",
        _=>"",
    }
}
impl Bridge<'_> {
    pub(super) fn concrete_html_interface(&self, object: u32) -> Option<&'static str> {
        if let Some(name) = self.frame_concrete_html_interface(object) {
            return Some(name);
        }
        let node = node_index(object)?;
        if !self.class_list_element_receiver(object) {
            return None;
        }
        crate::javascript::ordinary_html_interface(tag(self.doc, node))
    }

    pub(super) fn element_expando_receiver(&self, object: u32) -> bool {
        self.class_list_element_receiver(object)
            && node_index(object).is_some_and(|node| {
                let name = tag(self.doc, node);
                if name == "iframe" {
                    // Parsed inactive frames expose their interface identity.
                    // Only script-created frames have ordinary own-property storage.
                    return self.state.dom.created.contains(&node);
                }
                matches!(
                    name,
                    "html"
                        | "head"
                        | "body"
                        | "div"
                        | "span"
                        | "p"
                        | "a"
                        | "area"
                        | "b"
                        | "i"
                        | "em"
                        | "strong"
                        | "small"
                        | "s"
                        | "u"
                        | "mark"
                        | "main"
                        | "article"
                        | "section"
                        | "header"
                        | "footer"
                        | "aside"
                        | "nav"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "br"
                        | "wbr"
                        | "hr"
                        | "ul"
                        | "ol"
                        | "li"
                        | "dl"
                        | "dt"
                        | "dd"
                        | "blockquote"
                        | "pre"
                        | "code"
                        | "kbd"
                        | "samp"
                        | "var"
                        | "sub"
                        | "sup"
                        | "button"
                        | "label"
                        | "input"
                        | "textarea"
                        | "img"
                        | "video"
                        | "script"
                        | "style"
                        | "link"
                        | "meta"
                        | "title"
                )
            })
    }

    pub(super) fn element_native_property(&self, object: u32, name: &str) -> bool {
        if dom_tree::node_constant(name).is_some()
            || self.method(object, name).is_some()
            || contains(COMMON, name)
            || contains(HANDLERS, name)
            || contains(ARIA, name)
        {
            return true;
        }
        node_index(object).is_some_and(|node| {
            let tag = tag(self.doc, node);
            contains(specific(tag), name)
                || self.html_text_reflection(node,name).is_some()
                || self.html_spacing_property(node, name)
                || matches!(tag, "a" | "area") && contains(URL, name)
                || matches!(tag, "input" | "button" | "textarea") && contains(VALIDATION, name)
                || matches!(tag, "input" | "textarea") && contains(EDITING, name)
        })
    }
}
