//! Observer preflight reads the genuine inert tree, without a Window or loader.
use super::*;
use crate::javascript::{NativeAttributeChange, NativeChildListChange};

impl Bridge<'_> {
    pub(in crate::scripting) fn write_inert_character_data(&mut self,object:u32,text:DomString)->Result<(),String>{
        let removed=self.character_data_source(object).ok_or("Inert CharacterData receiver is unavailable.")?.utf16_len();let inserted=text.utf16_len();
        self.write_inert_character_splice(object,text,0,removed,inserted)
    }
    pub(in crate::scripting) fn write_inert_character_splice(
        &mut self,object:u32,text:DomString,offset:usize,removed:usize,inserted:usize,
    ) -> Result<(), String> {
        self.state.prepare_pi_invalidation()?;
        self.state.prepare_range_changes(1,0)?;
        if !matches!(self.inert_node_type(object), Some(3 | 4 | 7 | 8)) {
            return Err("Inert CharacterData receiver is unavailable.".into());
        }
        let kind = &self.state.inert_documents.node(object).unwrap().kind;
        let direct = match kind {
            Kind::Text(old) => Some(old.capacity()),
            Kind::ProcessingInstruction { data, .. } => Some(data.capacity()),
            _ => None,
        };
        if let Some(old) = direct {
            self.state
                .replace_storage(old, text.capacity(), text.used_bytes())?;
            match &mut self.state.inert_documents.node_mut(object).unwrap().kind {
                Kind::Text(old) => *old = text,
                Kind::ProcessingInstruction { data, .. } => *data = text,
                _ => unreachable!(),
            }
        } else {
            let Kind::Parsed {
                source,
                initial_template,
            } = *kind
            else {
                unreachable!()
            };
            let source_node = self
                .parsed_source(source)
                .ok_or("Inert CharacterData source is unavailable.")?
                .0;
            let target_bytes = match &source_node.kind {
                NodeKind::ProcessingInstruction { target, .. } => target.len(),
                _ => 0,
            };
            self.state.copies(text.used_bytes().saturating_add(target_bytes))?;
            let source_node = self.parsed_source(source).unwrap().0;
            let kind = match &source_node.kind {
                NodeKind::Text(_) => NodeKind::Text(text),
                NodeKind::Cdata(_)=>NodeKind::Cdata(text),
                NodeKind::Comment(_) => NodeKind::Comment(text),
                NodeKind::ProcessingInstruction { target, .. } => NodeKind::ProcessingInstruction {
                    target: script_clone_node::copy_text(target)?,
                    data: text,
                },
                _ => unreachable!(),
            };
            let node = Node {
                kind,
                children: Vec::new(),
                namespace: crate::html::Namespace::None,
            };
            let payload = node_bytes(&node)
                .checked_add(size_of::<Node>() + 2 * size_of::<usize>())
                .ok_or("Inert CharacterData source storage overflow.")?;
            self.reserve_inert_entries(0, payload, 1)?;
            let arena = &mut self.state.inert_documents;
            let source = arena.generated.len();
            arena.generated.push(GeneratedNode {
                node: Rc::new(node),
                unsafe_nonce: false,
            });
            arena.node_mut(object).unwrap().kind = Kind::Parsed {
                source: ParsedSource::Generated(source),
                initial_template,
            };
        }
        self.state.ranges_splice_data(object,offset,removed,inserted);
        self.state.dom_committed();
        self.state.invalidate_pi_attributes(object);
        Ok(())
    }

    pub(in crate::scripting) fn inert_observation_parent(&self, object: u32) -> Option<u32> {
        self.state.inert_documents.node(object)?.parent
    }

    pub(in crate::scripting) fn inert_child_list_change_plan(
        &mut self,
        parent: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Vec<NativeChildListChange>, String> {
        if !matches!(method, "appendChild" | "insertBefore" | "removeChild") {
            return Ok(Vec::new());
        }
        let Some(Value::Host(child)) = args.first() else {
            return Ok(Vec::new());
        };
        let reference = if method == "insertBefore" {
            match args.get(1) {
                Some(Value::Host(id)) => Some(*id),
                Some(Value::Null | Value::Undefined) => None,
                _ => return Ok(Vec::new()),
            }
        } else {
            None
        };
        if reference == Some(*child) {
            return Ok(Vec::new());
        }
        let siblings = self
            .inert_children(parent)
            .ok_or("Inert observer parent is unavailable.")?;
        let length = siblings.len();
        let position = if let Some(reference) = reference {
            let Some(at) = siblings.iter().position(|&id| id == reference) else {
                return Ok(Vec::new());
            };
            at
        } else {
            length
        };
        let old_parent = self.inert_observation_parent(*child);
        if method == "removeChild" && old_parent != Some(parent) {
            return Ok(Vec::new());
        }
        if method == "appendChild" && old_parent == Some(parent) && siblings.last() == Some(child) {
            return Ok(Vec::new());
        }
        let donor = self.inert_node_type(*child) == Some(11) && method != "removeChild";
        let count = if donor {
            self.inert_children(*child)
                .ok_or("Inert observer fragment is unavailable.")?
                .len()
        } else {
            1
        };
        if donor && count == 0 {
            return Ok(Vec::new());
        }
        self.state.work(length.saturating_add(count))?;
        self.state.copies(
            count
                .saturating_mul(size_of::<u32>())
                .saturating_add(2 * size_of::<NativeChildListChange>()),
        )?;
        let mut records = Vec::new();
        records
            .try_reserve(2)
            .map_err(|_| "Inert observer record reservation failed.")?;
        let mut added = Vec::new();
        added
            .try_reserve_exact(count)
            .map_err(|_| "Inert observer child snapshot reservation failed.")?;
        if donor {
            added.extend_from_slice(self.inert_children(*child).unwrap());
            let mut removed = Vec::new();
            removed
                .try_reserve_exact(count)
                .map_err(|_| "Inert observer donor snapshot reservation failed.")?;
            self.state.copies(count.saturating_mul(size_of::<u32>()))?;
            removed.extend_from_slice(&added);
            records.push(NativeChildListChange {
                target: *child,
                added: Vec::new(),
                removed,
                previous: None,
                next: None,
            });
        } else {
            added.push(*child);
            if let Some(old) = old_parent {
                let siblings = self
                    .inert_children(old)
                    .ok_or("Inert observer source parent is unavailable.")?;
                let count = siblings.len();
                self.state.work(count)?;
                let siblings = self.inert_children(old).unwrap();
                let at = siblings
                    .iter()
                    .position(|id| id == child)
                    .ok_or("Inert observer source child is unavailable.")?;
                let previous = at.checked_sub(1).and_then(|at| siblings.get(at)).copied();
                let next = siblings.get(at + 1).copied();
                let mut removed = Vec::new();
                removed
                    .try_reserve_exact(1)
                    .map_err(|_| "Inert observer removal reservation failed.")?;
                removed.push(*child);
                records.push(NativeChildListChange {
                    target: old,
                    added: Vec::new(),
                    removed,
                    previous,
                    next,
                });
            }
            if method == "removeChild" {
                return Ok(records);
            }
        }
        let siblings = self.inert_children(parent).unwrap();
        let previous = siblings[..position]
            .iter()
            .rev()
            .copied()
            .find(|id| id != child);
        let next = siblings[position..].iter().copied().find(|id| id != child);
        records.push(NativeChildListChange {
            target: parent,
            added,
            removed: Vec::new(),
            previous,
            next,
        });
        Ok(records)
    }

    pub(in crate::scripting) fn inert_attribute_observation_plan(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Option<NativeAttributeChange>, String> {
        if !matches!(
            method,
            "setAttribute" | "removeAttribute" | "setAttributeNS" | "removeAttributeNS"
        ) {
            return Ok(None);
        }
        let expanded = matches!(method, "setAttributeNS" | "removeAttributeNS");
        let Some(value) = args.get(usize::from(expanded)) else {
            return Ok(None);
        };
        let mut local = self.dom_text(value)?;
        let namespace=if expanded{script_attr::namespace(args)?}else{None};
        if expanded && method == "setAttributeNS" && namespace.is_some() {
            if let(Some(_),name)=local.view().split_colon(){self.state.copies(name.copy_bytes()?)?;local=name.try_owned()?;}
        }
        if !expanded
            && self.inert_document_kind_for_node(object)==Some(crate::html::DocumentKind::Html)
            && matches!(self.inert_get(object,"namespaceURI")?,Value::String(uri) if uri==HTML_NAMESPACE)
        {
            local.make_ascii_lowercase();
        }
        let attrs = self
            .inert_element_attributes(object)
            .ok_or("Inert observer attributes are unavailable.")?;
        let work = attrs
            .iter()
            .fold(attrs.len(), |n, a| n.saturating_add(a.name.bytes()));
        self.state.work(work)?;
        let slot = self
            .inert_element_attributes(object)
            .unwrap()
            .iter()
            .position(|attr| {
                if expanded {
                    attr.name.matches_names(namespace,local.view())
                } else {
                    attr.name.qualified == local
                }
            });
        if matches!(method, "removeAttribute" | "removeAttributeNS") && slot.is_none() {
            return Ok(None);
        }
        let prior = slot.map(|slot| &self.inert_element_attributes(object).unwrap()[slot]);
        let name=prior.map_or(local.view(),|attr|attr.name.local().view());
        let uri = prior.map_or(namespace, |attr| attr.name.namespace_uri().map(DomString::view));
        let old = prior.map(|attr| &attr.value);
        self.state.copies(
            name.copy_bytes()?
                .saturating_add(uri.map(crate::dom_string::DomStringRef::copy_bytes).transpose()?.unwrap_or(0))
                .saturating_add(old.map_or(0,DomString::logical_bytes)),
        )?;
        let prior = slot.map(|slot| &self.inert_element_attributes(object).unwrap()[slot]);
        let name=prior.map_or(local.view(),|attr|attr.name.local().view());
        let uri = prior.map_or(namespace, |attr| attr.name.namespace_uri().map(DomString::view));
        let old = prior.map(|attr| &attr.value);
        Ok(Some(NativeAttributeChange {
            local_name: name.try_owned()?,
            namespace: uri.map(crate::dom_string::DomStringRef::try_owned).transpose()?,
            old_value: old.map(Self::dom_string_copy).transpose()?,
        }))
    }
}
