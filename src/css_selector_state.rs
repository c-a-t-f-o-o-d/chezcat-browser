//! HTML actual disabledness for :disabled, with the matcher's shared budget.
//! https://html.spec.whatwg.org/multipage/semantics-other.html#disabled-elements
//! https://html.spec.whatwg.org/multipage/form-elements.html#concept-option-disabled
use super::*;

fn work(budget: &mut usize, cost: usize) -> bool {
    if cost > *budget {
        *budget = 0;
        false
    } else {
        *budget -= cost;
        true
    }
}

impl MatchContext<'_> {
    pub(super) fn actually_disabled(&self, node: usize, budget: &mut usize) -> bool {
        let Some(entry) = self.doc.nodes.get(node) else {
            *budget = 0;
            return false;
        };
        if entry.namespace != crate::html::Namespace::Html {
            return false;
        }
        let NodeKind::Element { tag, .. } = &entry.kind else {
            return false;
        };
        match tag.as_str() {
            "button" | "input" | "select" | "textarea" | "fieldset" => {
                let result =
                    crate::forms::disabled_with_work(self.doc, node, self.parents, &mut |cost| {
                        work(budget, cost)
                    });
                result.unwrap_or_else(|| {
                    *budget = 0;
                    false
                })
            }
            "option" | "optgroup" => {
                if self.attribute(node, "disabled", budget).is_some() {
                    return true;
                }
                if *budget == 0 {
                    return false;
                }
                // Option's own disabledness stops at the first optgroup or
                // select/datalist/hr/option boundary; fieldsets are not groups.
                if tag == "option" {
                    let mut parent = self.parents.get(node).copied().flatten();
                    for _ in 0..self.doc.nodes.len() {
                        let Some(index) = parent else {
                            break;
                        };
                        if !work(budget, 1) {
                            return false;
                        }
                        let Some(item) = self.doc.nodes.get(index) else {
                            *budget = 0;
                            return false;
                        };
                        if item.namespace == crate::html::Namespace::Html {
                            let NodeKind::Element { tag, .. } = &item.kind else {
                                *budget = 0;
                                return false;
                            };
                            match tag.as_str() {
                                "select" | "datalist" | "hr" | "option" => break,
                                "optgroup" => {
                                    if self.attribute(index, "disabled", budget).is_some() {
                                        return true;
                                    }
                                    break;
                                }
                                _ => {}
                            }
                        }
                        parent = self.parents.get(index).copied().flatten();
                    }
                }
                // HTML actual disabledness additionally includes the nearest
                // eligible select's disabled state for option and optgroup.
                let mut parent = self.parents.get(node).copied().flatten();
                let mut group = false;
                for _ in 0..self.doc.nodes.len() {
                    let Some(index) = parent else {
                        return false;
                    };
                    if !work(budget, 1) {
                        return false;
                    }
                    let Some(item) = self.doc.nodes.get(index) else {
                        *budget = 0;
                        return false;
                    };
                    if item.namespace == crate::html::Namespace::Html {
                        let NodeKind::Element { tag, .. } = &item.kind else {
                            *budget = 0;
                            return false;
                        };
                        match tag.as_str() {
                            "datalist" | "hr" | "option" => return false,
                            "optgroup" => {
                                if group {
                                    return false;
                                }
                                group = true;
                            }
                            "select" => {
                                return crate::forms::disabled_with_work(
                                    self.doc,
                                    index,
                                    self.parents,
                                    &mut |cost| work(budget, cost),
                                )
                                .unwrap_or_else(|| {
                                    *budget = 0;
                                    false
                                })
                            }
                            _ => {}
                        }
                    }
                    parent = self.parents.get(index).copied().flatten();
                }
                *budget = 0;
                false
            }
            _ => false,
        }
    }
}
