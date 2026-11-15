//! Document-owned CSS image bindings using the existing image worker.
//! Background pixels do not participate in replaced-element natural dimensions.
use super::*;
use crate::scripting::DynamicImage;

pub(super) struct Request {
    key: (String, String, bool),
    generation: u64,
}

impl App {
    fn discard_background_plan(&mut self, reason: &str) {
        self.cancel_background_requests();
        self.backgrounds.clear();
        self.background_requests.clear();
        // Clearing retained pixels first makes selection failure fail closed.
        if self.events.try_reserve(1).is_ok() {
            self.events.push(reason.into());
        }
    }

    pub(super) fn cancel_background_requests(&mut self) {
        for &node in self.pending_backgrounds.keys() {
            self.cancel_background_image(node);
        }
        self.pending_backgrounds.clear();
        if let Some(page) = &mut self.scripting {
            page.set_background_pending(false);
        }
        // Retain selected bindings: Stop must not silently retry on reflow.
    }

    pub(super) fn sync_background_images(&mut self) {
        if self.alternate.is_some() {
            return;
        }
        let mut selected = HashMap::new();
        for item in &self.page.items {
            if let PaintItem::Background { node, image, .. } = item {
                if selected.try_reserve(1).is_err() {
                    self.discard_background_plan(
                        "BACKGROUND IMAGE: cannot reserve selection storage.",
                    );
                    return;
                }
                selected.insert(*node, image.clone());
            }
        }
        let removals = self
            .background_requests
            .keys()
            .filter(|node| !selected.contains_key(node))
            .count();
        let mut removed = Vec::new();
        if removed.try_reserve_exact(removals).is_err() {
            self.discard_background_plan("BACKGROUND IMAGE: cannot reserve retirement storage.");
            return;
        }
        removed.extend(
            self.background_requests
                .keys()
                .filter(|node| !selected.contains_key(node))
                .copied(),
        );
        for node in removed {
            self.cancel_background_image(node);
            self.pending_backgrounds.remove(&node);
            self.background_requests.remove(&node);
            self.backgrounds.remove(&node);
        }
        for (node, image) in selected {
            let base = image
                .stylesheet
                .and_then(|owner| self.style_bases.get(&owner))
                .unwrap_or(&self.base_location)
                .clone();
            let key = (image.url.clone(), base.display(), self.images_enabled);
            if self
                .background_requests
                .get(&node)
                .is_some_and(|request| request.key == key)
            {
                continue;
            }
            self.cancel_background_image(node);
            self.pending_backgrounds.remove(&node);
            self.backgrounds.remove(&node);
            let admitted = (|| -> Result<DynamicImage, String> {
                let next_generation = self
                    .background_generation
                    .checked_add(1)
                    .filter(|&next| next <= self.background_request_limit);
                self.background_requests
                    .try_reserve(1)
                    .map_err(|_| "Cannot reserve CSS image binding.")?;
                // A denied source still has a binding identity. Reflow must
                // neither repeat its failed admission nor resurrect old pixels.
                let generation = next_generation.unwrap_or(self.background_generation);
                self.background_requests
                    .insert(node, Request { key, generation });
                let Some(generation) = next_generation else {
                    return Err(format!(
                        "CSS image request limit reached ({} per document).",
                        self.background_request_limit
                    ));
                };
                self.pending_backgrounds
                    .try_reserve(1)
                    .map_err(|_| "Cannot reserve pending CSS image binding.")?;
                self.background_generation = generation;
                if !self.images_enabled {
                    return Err("Images disabled by user.".into());
                }
                Ok(DynamicImage {
                    node,
                    generation,
                    source: image.url,
                    base,
                    origin: self.location.clone(),
                    policy: self.document_policy.clone(),
                })
            })();
            match admitted.and_then(|request| {
                self.submit_background_image(request.clone())?;
                self.pending_backgrounds.insert(node, request.generation);
                Ok(())
            }) {
                Ok(()) => {}
                Err(error) => self
                    .events
                    .push(format!("BACKGROUND IMAGE node {node}: {error}")),
            }
        }
        if let Some(page) = &mut self.scripting {
            page.set_background_pending(!self.pending_backgrounds.is_empty());
        }
    }

    pub(super) fn finish_background_image(
        &mut self,
        request: &DynamicImage,
        result: Result<crate::image::DecodedImage, String>,
    ) {
        if self.pending_backgrounds.get(&request.node) != Some(&request.generation)
            || self
                .background_requests
                .get(&request.node)
                .is_none_or(|current| current.generation != request.generation)
        {
            return;
        }
        self.pending_backgrounds.remove(&request.node);
        let result = result.and_then(|mut asset| {
            if asset.animation.take().is_some() {
                self.events.push(format!("BACKGROUND IMAGE node {}: animation is not implemented; displaying its decoded initial frame.", request.node));
            }
            if self.image_union_bytes(request.node, true, &asset)? > loader::MAX_DECODED_IMAGE_BYTES {
                return Err("Decoded document image budget exceeded (64 MiB).".into());
            }
            self.backgrounds.try_reserve(1).map_err(|_| "Cannot retain decoded CSS image.")?;
            Ok(asset.bitmap)
        });
        match result {
            Ok(bitmap) => {
                self.backgrounds.insert(request.node, bitmap);
            }
            Err(error) => {
                self.backgrounds.remove(&request.node);
                self.events
                    .push(format!("BACKGROUND IMAGE node {}: {error}", request.node));
            }
        }
        if let Some(page) = &mut self.scripting {
            page.set_background_pending(!self.pending_backgrounds.is_empty());
        }
        if self.view == View::Resources {
            self.refresh_resource_view();
        }
    }

    pub(super) fn resume_background_loading(&mut self) -> bool {
        if self.script_jobs_paused() { return false; }
        let outcome = self
            .scripting
            .as_mut()
            .map(|page| page.finish_background_loading(&mut self.document, &mut self.forms));
        if outcome.is_some_and(|outcome| outcome.layout_changed) {
            self.reflow();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[path = "app_background_tests.rs"]
mod tests;
