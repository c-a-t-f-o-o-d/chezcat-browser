//! Native whole declaration serialization and atomic attribute replacement.
use super::*;
impl Bridge<'_> {
    fn css_text_block(&mut self, source: &str, cssom: bool) -> Result<String, String> {
        if source.len() > self.state.limits.host.string_bytes {
            return Err("Inline CSS exceeds the string limit.".into());
        }
        self.state.work(1 + source.len().saturating_mul(16))?;
        self.state.copies(source.len().saturating_mul(64))?;
        let declarations = {
            let mut meter =
                crate::css::CssBudget::new(&mut self.state.host_work, &mut self.state.copy_work);
            crate::css::inline_text_declarations(
                source,
                script_inline_style::PROPERTIES,
                &mut meter,
            )?
        };
        let mut winners: Vec<(String, String, bool)> = Vec::new();
        winners
            .try_reserve_exact(declarations.len())
            .map_err(|_| "Cannot reserve CSSOM declaration winners.")?;
        for (name, raw, important) in declarations {
            validate_css_value(&name, &raw, self.state.limits.host.string_bytes)?;
            let value = if name == "transform" {
                match script_transform::normalize(&raw, self.state)? {
                    Ok(value) => value,
                    Err(crate::css::TransformSyntax::Invalid) => continue,
                    Err(crate::css::TransformSyntax::Unsupported) => {
                        return Err("Whole CSS text contains an unsupported transform.".into())
                    }
                }
            } else if name == "opacity" {
                let Some(value) = metered_opacity(&raw, self.state)? else {
                    continue;
                };
                value.serialize()?
            } else if name == "animation-timing-function" {
                let Some(value) = script_inline_style::normalize_easing(
                    &raw,
                    self.state,
                    if cssom {
                        script_inline_style::EasingText::Cssom
                    } else {
                        script_inline_style::EasingText::Specified
                    },
                )?
                else {
                    continue;
                };
                value
            } else {
                raw
            };
            self.state.work(winners.len())?;
            if let Some(index) = winners.iter().position(|(key, _, _)| key == &name) {
                if winners[index].2 && !important {
                    continue;
                }
                winners.remove(index);
                winners.push((name, value, important));
            } else {
                winners.push((name, value, important));
            }
        }
        let bytes = winners
            .iter()
            .try_fold(0usize, |n, (name, value, important)| {
                n.checked_add(name.len())?
                    .checked_add(value.len())?
                    .checked_add(if *important { 15 } else { 4 })
            })
            .ok_or("CSSOM serialized length overflow.")?;
        if bytes > self.state.limits.host.string_bytes {
            return Err("CSSOM serialization exceeds the string limit.".into());
        }
        self.state.copies(bytes)?;
        let mut result = String::new();
        result
            .try_reserve_exact(bytes)
            .map_err(|_| "Cannot reserve CSSOM serialization.")?;
        for (name, value, important) in winners {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(&name);
            result.push_str(": ");
            result.push_str(&value);
            if important {
                result.push_str(" !important");
            }
            result.push(';');
        }
        Ok(result)
    }
    pub(super) fn get_css_text(&mut self, node: usize) -> Result<Value, String> {
        let raw = self.doc.attr(node, "style").unwrap_or("");
        self.state.copies(raw.len())?;
        let raw = script_clone_node::copy_text(raw)?;
        let text = self.css_text_block(&raw, true)?;
        self.string(&text)
    }
    pub(super) fn set_css_text(&mut self, node: usize, source: &str) -> Result<(), String> {
        let text = self.css_text_block(source, false)?;
        self.attribute(node, "style", Some(text))
    }
}
