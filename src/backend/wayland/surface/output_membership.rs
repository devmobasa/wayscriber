//! Compositor membership and explicit placement of one overlay surface.
use wayland_client::protocol::wl_output::WlOutput;

#[derive(Default)]
pub(super) struct OutputMembership {
    current: Option<WlOutput>,
    entered: Vec<WlOutput>,
    layer_output: Option<WlOutput>,
    awaiting_enter: bool,
}

impl OutputMembership {
    pub(super) fn for_layer(output: Option<WlOutput>) -> Self {
        Self {
            current: output.clone(),
            awaiting_enter: output.is_some(),
            layer_output: output,
            entered: Vec::new(),
        }
    }

    pub(super) fn current(&self) -> Option<WlOutput> {
        self.current.clone()
    }

    /// Keep an explicit XDG fullscreen target until it enters or is removed.
    pub(super) fn request_output(&mut self, output: WlOutput) {
        self.current = Some(output);
        self.awaiting_enter = true;
    }

    /// Use a guess only while nothing is selected. Unlike an explicit request,
    /// it never outlives the compositor's first enter for another output.
    pub(super) fn assume(&mut self, output: WlOutput) {
        if self.current.is_none() {
            self.current = Some(output);
        }
    }

    pub(super) fn enter(&mut self, output: WlOutput) {
        if self.current.as_ref() == Some(&output) {
            self.awaiting_enter = false;
        }
        if !self.entered.contains(&output) {
            self.entered.push(output);
        }

        self.select_output();
    }

    pub(super) fn remove(&mut self, output: &WlOutput) {
        self.entered.retain(|entered| entered != output);
        if self.current.as_ref() == Some(output) {
            self.current = None;
            self.awaiting_enter = false;
        }

        self.select_output();
    }

    fn select_output(&mut self) {
        self.current = self
            .layer_output
            .as_ref()
            .filter(|output| {
                self.entered.contains(output)
                    || (self.awaiting_enter && self.current.as_ref() == Some(output))
            })
            .or_else(|| {
                self.current.as_ref().filter(|output| {
                    self.awaiting_enter || self.entered.contains(output) || self.entered.is_empty()
                })
            })
            .or_else(|| self.entered.first())
            .cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::handlers::test_support::HandlerFixture;

    #[test]
    fn layer_placement_takes_precedence_and_falls_back_to_remaining_output() {
        let fixture = HandlerFixture::with_outputs(crate::config::Config::default(), 2);
        let outputs: Vec<_> = fixture.state.protocol.output().outputs().collect();
        let mut membership = OutputMembership::for_layer(Some(outputs[1].clone()));

        membership.enter(outputs[0].clone());
        assert_eq!(membership.current(), Some(outputs[1].clone()));

        membership.enter(outputs[1].clone());
        membership.enter(outputs[0].clone());
        assert_eq!(membership.current(), Some(outputs[1].clone()));

        membership.remove(&outputs[1]);
        assert_eq!(membership.current(), Some(outputs[0].clone()));

        membership.enter(outputs[1].clone());
        assert_eq!(membership.current(), Some(outputs[1].clone()));

        membership.remove(&outputs[1]);
        membership.remove(&outputs[0]);
        assert!(membership.current().is_none());
    }

    #[test]
    fn assumed_output_survives_unrelated_removal_until_the_first_enter() {
        let fixture = HandlerFixture::with_outputs(crate::config::Config::default(), 3);
        let outputs: Vec<_> = fixture.state.protocol.output().outputs().collect();
        let mut membership = OutputMembership::default();
        membership.assume(outputs[0].clone());

        membership.remove(&outputs[2]);
        assert_eq!(membership.current(), Some(outputs[0].clone()));

        membership.enter(outputs[1].clone());
        assert_eq!(membership.current(), Some(outputs[1].clone()));

        membership.assume(outputs[0].clone());
        assert_eq!(membership.current(), Some(outputs[1].clone()));
    }

    #[test]
    fn unrelated_removal_preserves_pre_enter_placement_but_target_loss_releases_it() {
        let fixture = HandlerFixture::with_outputs(crate::config::Config::default(), 2);
        let outputs: Vec<_> = fixture.state.protocol.output().outputs().collect();
        let mut membership = OutputMembership::default();
        membership.request_output(outputs[0].clone());

        membership.remove(&outputs[1]);
        assert_eq!(membership.current(), Some(outputs[0].clone()));

        membership.enter(outputs[1].clone());
        membership.remove(&outputs[0]);
        assert_eq!(membership.current(), Some(outputs[1].clone()));
    }
}
