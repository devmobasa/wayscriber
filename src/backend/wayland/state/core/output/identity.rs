use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn preferred_fullscreen_output(
        &self,
    ) -> Option<wl_output::WlOutput> {
        if let Some(preferred) = self.surface.placement().preferred_output_identity()
            && let Some(output) = self.protocol.output().outputs().find(|output| {
                self.output_identity_for(output)
                    .map(|id| id.eq_ignore_ascii_case(preferred))
                    .unwrap_or(false)
            })
        {
            return Some(output);
        }

        self.surface
            .current_output()
            .or_else(|| self.protocol.output().outputs().next())
    }

    pub(in crate::backend::wayland) fn output_identity_for(
        &self,
        output: &wl_output::WlOutput,
    ) -> Option<String> {
        let info = self.protocol.output().info(output)?;

        let mut components: Vec<String> = Vec::new();

        if let Some(name) = info.name.filter(|s| !s.is_empty()) {
            components.push(name);
        }

        if !info.make.is_empty() {
            components.push(info.make);
        }

        if !info.model.is_empty() {
            components.push(info.model);
        }

        if components.is_empty() {
            components.push(format!("id{}", info.id));
        }

        Some(components.join("-"))
    }

    pub(super) fn sorted_known_outputs(&self) -> Vec<wl_output::WlOutput> {
        let mut outputs: Vec<(u32, wl_output::WlOutput)> = self
            .protocol
            .output()
            .outputs()
            .filter_map(|output| {
                self.protocol
                    .output()
                    .info(&output)
                    .map(|info| (info.id, output))
            })
            .collect();

        outputs.sort_by_key(|(id, _)| *id);
        outputs.into_iter().map(|(_, output)| output).collect()
    }

    pub(super) fn output_badge_label_for(&self, output: &wl_output::WlOutput) -> Option<String> {
        let info = self.protocol.output().info(output)?;

        if let Some(name) = info.name.as_deref().filter(|name| !name.is_empty()) {
            return Some(crate::util::truncate_with_ellipsis(
                name,
                OUTPUT_BADGE_MAX_LEN,
            ));
        }

        let label = match (info.make.trim(), info.model.trim()) {
            ("", "") => format!("Output {}", info.id),
            (make, "") => make.to_string(),
            ("", model) => model.to_string(),
            (make, model) => format!("{make} {model}"),
        };

        Some(crate::util::truncate_with_ellipsis(
            &label,
            OUTPUT_BADGE_MAX_LEN,
        ))
    }

    pub(in crate::backend::wayland) fn refresh_active_output_label(&mut self) {
        let next_label = self
            .surface
            .current_output()
            .as_ref()
            .and_then(|output| self.output_badge_label_for(output))
            .or_else(|| {
                self.sorted_known_outputs()
                    .first()
                    .and_then(|output| self.output_badge_label_for(output))
            });

        if self.input_state.set_active_output_label(next_label) {
            self.input_state.needs_redraw = true;
        }
    }
}
