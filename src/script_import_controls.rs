//! Default-state import over the existing real native control planner.
//! The staged nodes are moved into a checked local view, never serialized.
use super::*;
fn empty() -> Node {
    Node {
        kind: NodeKind::DocumentFragment,
        children: Vec::new(),
        namespace: crate::html::Namespace::None,
    }
}
impl Bridge<'_> {
    pub(super) fn prepare_imported_controls(
        &mut self,
        staged: &mut Vec<DetachedCopy>,
        payload: usize,
    ) -> Result<Option<forms::SelectFormPlan>, String> {
        if !staged.iter().any(|copy|matches!(&copy.node.kind,NodeKind::Element{tag,..} if matches!(tag.as_str(),"input"|"textarea"|"select"|"option"|"optgroup"|"button"))){return Ok(None);}
        let base = self.doc.nodes.len();
        let count = staged.len();
        self.state.rebuild(self.doc)?;
        let mut links = 0usize;
        for copy in staged.iter() {
            links = links
                .checked_add(copy.node.children.len())
                .ok_or("Import control topology overflow.")?;
            if copy
                .node
                .children
                .iter()
                .any(|&child| child < base || child >= base + count)
                || copy
                    .parent
                    .is_some_and(|parent| parent < base || parent >= base + count)
            {
                return Err("Import control topology escaped its staged forest.".into());
            }
        }
        let minimum = (count + 1)
            .checked_mul(size_of::<Node>())
            .and_then(|bytes| {
                bytes.checked_add(staged.capacity().checked_mul(size_of::<DetachedCopy>())?)
            })
            .and_then(|bytes| bytes.checked_add(payload))
            .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
            .ok_or("Import control temporary size overflow.")?;
        if minimum
            > self
                .state
                .retained_limit
                .saturating_sub(self.state.retained)
        {
            return Err("Import control temporary allowance exhausted.".into());
        }
        self.state
            .work(count.saturating_add(links).saturating_mul(2))?;
        self.state.copies(
            count
                .checked_mul(size_of::<Node>())
                .and_then(|bytes| {
                    links
                        .checked_mul(2 * size_of::<usize>())
                        .and_then(|links| bytes.checked_add(links))
                })
                .ok_or("Import control view work overflow.")?,
        )?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count + 1)
            .map_err(|_| "Cannot reserve import control view.")?;
        let mut root = empty();
        root.children
            .try_reserve_exact(1)
            .map_err(|_| "Cannot reserve import control root.")?;
        root.children.push(1);
        let temporary = nodes
            .capacity()
            .checked_mul(size_of::<Node>())
            .and_then(|bytes| {
                bytes.checked_add(staged.capacity().checked_mul(size_of::<DetachedCopy>())?)
            })
            .and_then(|bytes| bytes.checked_add(payload))
            .and_then(|bytes| bytes.checked_add(root.children.capacity() * size_of::<usize>()))
            .filter(|bytes| {
                *bytes
                    <= self
                        .state
                        .retained_limit
                        .saturating_sub(self.state.retained)
            })
            .ok_or("Import control allocated view allowance exhausted.")?;
        nodes.push(root);
        for copy in staged.iter_mut() {
            let mut node = std::mem::replace(&mut copy.node, empty());
            for child in &mut node.children {
                *child = *child - base + 1;
            }
            nodes.push(node);
        }
        let document = Document {
            kind:self.doc.kind,mode:self.doc.mode,
            stylesheet_limits: crate::css::StylesheetLimits::default(),
            media_environment: None,
            title: String::new(),
            content_type: String::new(),
            nodes,
            scripting_enabled: false,
            nonce_unsafe_nodes: HashSet::new(),
            withheld_inline_styles: HashSet::new(),
            native_focus: crate::html::NativeNode::default(),
            native_hover: crate::html::NativeNode::default(),
        };
        self.state.retained += temporary;
        let result = (|| {
            for local in 1..document.nodes.len() {
                let name = tag(&document, local);
                let parent = staged[local - 1].parent.map(|parent| parent - base + 1);
                self.state.work(1)?;
                if name == "input"
                    && (!document.nodes[local].children.is_empty()
                        || !(forms::fragment_text_input(&document, local)
                            || forms::fragment_checkbox_input(&document, local)))
                {
                    return Err(
                        "Import currently supports childless text/checkbox INPUT controls.".into(),
                    );
                }
                if name == "textarea" && forms::textarea_content(&document, local).is_none() {
                    return Err("Imported TEXTAREA requires supported plain RCDATA content.".into());
                }
                if matches!(name, "option" | "optgroup") {
                    let valid = parent.is_some_and(|parent| {
                        if name == "optgroup" {
                            tag(&document, parent) == "select"
                        } else {
                            matches!(tag(&document, parent), "select" | "optgroup")
                        }
                    });
                    if !valid {
                        return Err(
                            "Imported OPTION/OPTGROUP requires its conventional SELECT context."
                                .into(),
                        );
                    }
                }
                if matches!(name, "select" | "optgroup" | "option") {
                    for &child in &document.nodes[local].children {
                        self.state.work(1)?;
                        let valid = match &document.nodes[child].kind {
                            NodeKind::Text(_) | NodeKind::Cdata(_) | NodeKind::Comment(_) => true,
                            NodeKind::Element { tag, .. } => match name {
                                "select" => matches!(tag.as_str(), "option" | "optgroup"),
                                "optgroup" => tag == "option",
                                _ => false,
                            },
                            _ => false,
                        };
                        if !valid {
                            return Err("Import SELECT descendants require conventional option/group/text nodes.".into());
                        }
                    }
                }
            }
            self.prepare_select_form_change(Some((&document, base)), &[(base, None)])
        })();
        self.state.retained -= temporary;
        // Restore owned buffers on both success and refusal. Preparation never
        // publishes a DOM identity or commits the returned native control plan.
        for (copy, mut node) in staged.iter_mut().zip(document.nodes.into_iter().skip(1)) {
            for child in &mut node.children {
                *child += base - 1;
            }
            copy.node = node;
        }
        let plan = result?;
        self.state.copies(plan.retained_bytes)?;
        Ok(Some(plan))
    }
}
