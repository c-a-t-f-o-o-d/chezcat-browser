//! One immutable generation of fully admitted stylesheet syntax. Node matches,
//! inline declarations, variables, form state and computed styles remain live.
use super::*;
use std::borrow::Cow;
use std::rc::Rc;
#[cfg(test)]
#[path = "css_stylesheet_cache_tests.rs"]
mod tests;

struct SourceKey {
    owner: usize,
    bytes: String,
}

struct Prepared {
    sources: Vec<SourceKey>,
    limits: StylesheetLimits,
    scripting: bool,
    environment: Option<MediaEnvironment>,
    rules: Vec<Rule>,
    has_transform: bool,
}

#[derive(Clone)]
pub(crate) struct StylesheetCache(Rc<Prepared>);

pub(super) struct ParsedSheets {
    pub cache: Option<StylesheetCache>,
    fresh: Vec<Rule>,
    pub has_transform: bool,
}

impl ParsedSheets {
    pub fn rules(&self) -> &[Rule] {
        self.cache
            .as_ref()
            .map_or(self.fresh.as_slice(), |cache| cache.0.rules.as_slice())
    }
}

impl StylesheetCache {
    fn matches(
        &self,
        doc: &Document,
        sources: &[SheetCandidate<'_>],
        environment: Option<MediaEnvironment>,
        mut meter: Option<&mut CssBudget<'_>>,
    ) -> Result<bool, String> {
        if self.0.limits != doc.stylesheet_limits
            || self.0.scripting != doc.scripting_enabled
            || self.0.environment != environment
            || self.0.sources.len() != sources.len()
        {
            return Ok(false);
        }
        for (key, source) in self.0.sources.iter().zip(sources) {
            if let Some(meter) = meter.as_deref_mut() {
                meter.work(1)?;
            }
            let SheetCandidate::Source(owner, source) = source else {
                return Ok(false);
            };
            if key.owner != *owner || key.bytes.len() != source.len() {
                return Ok(false);
            }
            // Equality inspects at most this many bytes, charged before it.
            if let Some(meter) = meter.as_deref_mut() {
                meter.work(source.len().max(1).div_ceil(64))?;
            }
            if key.bytes.as_bytes() != source.as_bytes() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn shares_rules(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    doc: &Document,
    external: &HashMap<usize, String>,
    environment: Option<MediaEnvironment>,
    previous: Option<&StylesheetCache>,
    retain: bool,
    mut meter: Option<&mut CssBudget<'_>>,
    mut transform_meter: Option<&mut CssBudget<'_>>,
) -> Result<ParsedSheets, String> {
    let sources = if previous.is_none() {
        stylesheet_sources_with(doc, external, None, None)?
    } else if let Some(budget) = meter.as_deref_mut() {
        stylesheet_sources_with(doc, external, None, Some(budget))?
    } else {
        stylesheet_sources_with(doc, external, None, transform_meter.as_deref_mut())?
    };
    if let Some(previous) = previous {
        let matched = if let Some(budget) = meter.as_deref_mut() {
            previous.matches(doc, &sources, environment, Some(budget))?
        } else {
            previous.matches(doc, &sources, environment, transform_meter.as_deref_mut())?
        };
        if matched {
            // Rc cloning copies no source, rule, selector or declaration.
            if let Some(budget) = meter.as_deref_mut() {
                budget.work(1)?;
                budget.copies(std::mem::size_of::<StylesheetCache>())?;
            } else if let Some(budget) = transform_meter.as_deref_mut() {
                budget.work(1)?;
                budget.copies(std::mem::size_of::<StylesheetCache>())?;
            }
            return Ok(ParsedSheets {
                cache: Some(previous.clone()),
                fresh: Vec::new(),
                has_transform: previous.0.has_transform,
            });
        }
        // A cold fully budgeted call reserved these sources before collecting.
        // A failed cache probe has only paid for collection/comparison so far.
        if let Some(budget) = meter.as_deref_mut() {
            budget.document_sources(doc, external)?;
        }
    }
    let (sources, errors, _) = admit_stylesheets(doc, sources);
    let mut rules = Vec::new();
    let mut complete = errors.is_empty();
    let mut has_transform = false;
    for (owner, source) in &sources {
        let first = rules.len();
        match parse_sheet_with_media_inner(
            source,
            &mut rules,
            environment,
            doc.stylesheet_limits.rules,
            transform_meter.as_deref_mut(),
        ) {
            Err(SheetParseFailure::TransformBudget(error)) => return Err(error),
            Err(_) => complete = false,
            Ok(()) => {}
        }
        for rule in &mut rules[first..] {
            for declaration in &mut rule.declarations {
                declaration.stylesheet = Some(*owner);
                has_transform |= declaration.name == "transform";
            }
        }
    }
    // Refused or partially parsed source sets retain the established bounded
    // path. They never become reusable syntax or fabricate a complete sheet.
    if !retain || !complete || sources.is_empty() {
        return Ok(ParsedSheets {
            cache: None,
            fresh: rules,
            has_transform,
        });
    }
    let copied = sources.iter().fold(0usize, |sum, (_, source)| {
        sum.saturating_add(if matches!(source, Cow::Borrowed(_)) {
            source.len()
        } else {
            0
        })
    });
    let storage = sources
        .len()
        .saturating_mul(std::mem::size_of::<SourceKey>())
        .saturating_add(std::mem::size_of::<Prepared>() + 2 * std::mem::size_of::<usize>())
        .saturating_add(copied);
    if let Some(budget) = meter.as_deref_mut() {
        budget.work(copied.max(1).div_ceil(64))?;
        budget.copies(storage)?;
    } else if let Some(budget) = transform_meter.as_deref_mut() {
        budget.work(copied.max(1).div_ceil(64))?;
        budget.copies(storage)?;
    }
    let mut keys = Vec::new();
    if keys.try_reserve_exact(sources.len()).is_err() {
        return Ok(ParsedSheets {
            cache: None,
            fresh: rules,
            has_transform,
        });
    }
    for (owner, source) in sources {
        let bytes = match source {
            Cow::Owned(source) => source,
            Cow::Borrowed(source) => {
                let mut copy = String::new();
                if copy.try_reserve_exact(source.len()).is_err() {
                    return Ok(ParsedSheets {
                        cache: None,
                        fresh: rules,
                        has_transform,
                    });
                }
                copy.push_str(source);
                copy
            }
        };
        keys.push(SourceKey { owner, bytes });
    }
    let cache = StylesheetCache(Rc::new(Prepared {
        sources: keys,
        limits: doc.stylesheet_limits,
        scripting: doc.scripting_enabled,
        environment,
        rules,
        has_transform,
    }));
    Ok(ParsedSheets {
        cache: Some(cache),
        fresh: Vec::new(),
        has_transform,
    })
}
