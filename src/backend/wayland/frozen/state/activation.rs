//! Validate and install completed output images; portal pixels are processed on the worker.
use super::*;
use crate::backend::wayland::portal_capture::layout_token_matches;
use log::info;

impl FrozenState {
    pub fn set_pending_output_image(
        &mut self,
        image: FrozenImage,
        target_output_id: u32,
        source_geometry: OutputGeometry,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            target_output_id: Some(target_output_id),
            layout_generation: self.output_layout_generation,
            portal_layout_generation: None,
            source_geometry: Some(source_geometry),
            output_transform: None,
            source: FrozenCaptureSource::ActiveOutput,
        });
    }

    pub(in crate::backend::wayland::frozen) fn set_pending_output_image_with_transform(
        &mut self,
        image: FrozenImage,
        target_output_id: u32,
        source_geometry: OutputGeometry,
        output_transform: Option<wl_output::Transform>,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            target_output_id: Some(target_output_id),
            layout_generation: self.output_layout_generation,
            portal_layout_generation: None,
            source_geometry: Some(source_geometry),
            output_transform,
            source: FrozenCaptureSource::ActiveOutput,
        });
    }

    pub(in crate::backend::wayland::frozen) fn set_pending_portal_image(
        &mut self,
        image: FrozenImage,
        target_output_id: Option<u32>,
        source_geometry: Option<OutputGeometry>,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            target_output_id,
            layout_generation: self.output_layout_generation,
            portal_layout_generation: Some(self.portal_layout_generation),
            source_geometry,
            output_transform: None,
            source: FrozenCaptureSource::Portal,
        });
    }

    pub(in crate::backend::wayland::frozen) fn discard_pending_image_for_retry(&mut self) {
        self.pending_image = None;
    }

    pub fn has_pending_image(&self) -> bool {
        self.pending_image.is_some()
    }

    #[cfg(test)]
    pub fn activate_pending_image(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
    ) -> Result<bool, String> {
        self.activate_pending_image_with_live_outputs(phys_width, phys_height, input_state, None)
    }

    pub fn activate_pending_image_with_live_outputs(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
        live_output_count: Option<u32>,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending_image.take() else {
            return Ok(false);
        };
        if !layout_token_matches(
            pending.target_output_id,
            pending.layout_generation,
            self.active_output_id,
            self.output_layout_generation,
        ) || pending
            .portal_layout_generation
            .is_some_and(|generation| generation != self.portal_layout_generation)
        {
            if matches!(pending.source, FrozenCaptureSource::Portal)
                && self.queue_portal_layout_retry(
                    pending.target_output_id,
                    pending.portal_layout_generation != Some(self.portal_layout_generation),
                )
            {
                return Ok(false);
            }
            return self.reject_pending_image(
                input_state,
                "Freeze failed after the display layout changed",
            );
        }

        let mut image = pending.image;
        let mut provenance = None;

        if matches!(pending.source, FrozenCaptureSource::ActiveOutput) {
            let Some(geometry) = pending.source_geometry.as_ref() else {
                return self
                    .reject_pending_image(input_state, "Freeze capture geometry is unavailable");
            };
            let output_transform = pending.output_transform.unwrap_or(geometry.transform);

            provenance = pending.target_output_id.and_then(|output_id| {
                ScreenImageProvenance::new(
                    output_id,
                    pending.layout_generation,
                    geometry.scale,
                    output_transform,
                )
            });

            image = match image.with_output_transform(output_transform) {
                Ok(image) => image,
                Err(error) => {
                    return self.reject_pending_image(
                        input_state,
                        format!("Freeze capture transform failed: {error}"),
                    );
                }
            };
            if !geometry.accepts_transformed_pixel_size(image.width, image.height) {
                return self.reject_pending_image(
                    input_state,
                    "Freeze capture dimensions do not match the active output",
                );
            }
        }

        if let FrozenCaptureSource::Portal = pending.source {
            let Some(geometry) = pending
                .source_geometry
                .as_ref()
                .cloned()
                .and_then(|geometry| geometry.with_revalidated_output_count(live_output_count))
            else {
                let topology_changed = pending
                    .source_geometry
                    .as_ref()
                    .is_some_and(|geo| geo.output_count_conflicts_with_live(live_output_count));
                if self.queue_portal_layout_retry(pending.target_output_id, topology_changed) {
                    return Ok(false);
                }
                return self.reject_pending_image(
                    input_state,
                    "Freeze failed after the output layout changed",
                );
            };

            provenance = pending.target_output_id.and_then(|output_id| {
                ScreenImageProvenance::new(
                    output_id,
                    pending.layout_generation,
                    geometry.scale,
                    geometry.transform,
                )
            });

            if geometry.verified_pixel_size().is_none() {
                return self.reject_pending_image(
                    input_state,
                    "Freeze failed after the display changed size",
                );
            }
            if !geometry.accepts_transformed_pixel_size(image.width, image.height) {
                return self.reject_pending_image(
                    input_state,
                    "Freeze portal crop dimensions do not match the active output",
                );
            }
        }

        let Some(provenance) = provenance else {
            return self.reject_pending_image(
                input_state,
                "Freeze capture source identity is unavailable",
            );
        };

        if !OutputGeometry::dimensions_have_compatible_aspect(
            (image.width, image.height),
            (phys_width, phys_height),
        ) {
            return self.reject_pending_image(
                input_state,
                "Freeze capture aspect does not match the overlay surface",
            );
        }

        self.image_target_dimensions = Some((phys_width, phys_height));
        self.image = Some(Arc::new(image));
        self.image_provenance = Some(provenance);
        self.bump_image_generation();
        input_state.set_frozen_active(true);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
        self.finish_ready_acquisition(input_state);

        // Legacy tests and late callbacks without an acquisition still use the
        // capture-done wakeup as their resource-restoration boundary.
        self.capture_done = true;

        Ok(true)
    }

    fn reject_pending_image(
        &mut self,
        input_state: &mut InputState,
        error: impl Into<String>,
    ) -> Result<bool, String> {
        let error = error.into();
        self.finish_acquisition(ScreenAcquisitionOutcome::Failed(error.clone()), input_state);
        self.capture_done = true;
        input_state.set_frozen_active(false);
        input_state.needs_redraw = true;

        Err(error)
    }

    /// Drop frozen image if the surface size no longer matches.
    pub fn handle_resize(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
    ) {
        if let Some(target_dimensions) = self.image_target_dimensions
            && target_dimensions != (phys_width, phys_height)
        {
            info!("Surface resized; clearing frozen image");
            self.clear_image();
            input_state.set_frozen_active(false);
        }
    }

    /// Toggle unfreeze: drop the image and mark redraw.
    pub fn unfreeze(&mut self, input_state: &mut InputState) {
        self.clear_image();
        input_state.set_frozen_active(false);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }

    pub fn cancel(&mut self, input_state: &mut InputState) {
        self.abandon_acquisition(input_state);
    }

    pub(super) fn clear_image(&mut self) -> bool {
        let had_image = self.image.take().is_some();
        self.image_provenance = None;
        self.image_target_dimensions = None;
        if had_image {
            self.bump_image_generation();
        }

        had_image
    }

    pub(super) fn bump_image_generation(&mut self) {
        self.image_generation = self.image_generation.wrapping_add(1).max(1);
    }
}
