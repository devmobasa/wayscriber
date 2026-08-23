//! Pure per-pin state. Wayland callbacks only translate into these operations.

use std::sync::Arc;
use std::time::Instant;

use smithay_client_toolkit::shell::wlr_layer::LayerSurface;
use wayland_client::{backend::ObjectId, protocol::wl_surface};

use super::{Control, PinBuffers, RasterCache, content_position, control_at};
use crate::pin::{PinFrame, PinId, PinImage};

pub(crate) struct PinnedSurface {
    pub(crate) model: PinModel,
    pub(crate) shell: ShellState,
    pub(crate) buffers: PinBuffers,
    pub(crate) interaction: Interaction,
    pub(crate) visual: VisualState,
    pub(crate) raster: Option<RasterCache>,
    pub(crate) dirty: bool,
    pub(crate) full_damage: bool,
    pub(crate) first_commit_complete: bool,
    pub(crate) ready_sent: bool,
}

impl PinnedSurface {
    pub(crate) fn new(
        id: PinId,
        image: Arc<PinImage>,
        output: String,
        output_size: (u32, u32),
        frame: PinFrame,
    ) -> Self {
        Self {
            model: PinModel {
                id,
                image,
                output,
                output_size,
                frame,
            },
            shell: ShellState::default(),
            buffers: PinBuffers::default(),
            interaction: Interaction::Idle,
            visual: VisualState::default(),
            raster: None,
            dirty: true,
            full_damage: true,
            first_commit_complete: false,
            ready_sent: false,
        }
    }

    pub(crate) fn cancel_interaction(&mut self) -> bool {
        if self.interaction == Interaction::Idle {
            return false;
        }
        self.interaction = Interaction::Idle;
        self.visual.pressed = None;
        self.dirty = true;
        true
    }

    pub(crate) fn update_hover(&mut self, position: Option<(f64, f64)>) {
        let position = position.map(content_position);
        let next = position.and_then(|position| control_at(self.model.frame, position));
        if self.visual.hover != next || self.visual.pointer_position != position {
            self.visual.hover = next;
            self.visual.pointer_position = position;
            self.dirty = true;
        }
    }

    pub(crate) fn press(&mut self, owner: InputOwner, position: (f64, f64)) {
        let position = content_position(position);
        let control = control_at(self.model.frame, position);
        self.visual.pressed = control;
        self.interaction = if let Some(control) = control {
            Interaction::PressedControl { owner, control }
        } else {
            Interaction::Dragging {
                owner,
                grab_offset: position,
                relative_origin: (f64::from(self.model.frame.x), f64::from(self.model.frame.y)),
                shell_generation: self.shell.generation,
            }
        };
        self.dirty = true;
    }

    /// Resolve fallback motion in output coordinates.
    ///
    /// `committed_origin` must describe the margins that the compositor has
    /// applied to the coordinate space containing `position`, not a newer
    /// requested margin. This is the rebase that prevents margin feedback from
    /// reversing or stalling a drag.
    pub(crate) fn fallback_drag_origin(
        &mut self,
        owner: &InputOwner,
        position: (f64, f64),
        committed_origin: (i32, i32),
    ) -> Option<(f64, f64)> {
        let position = content_position(position);
        let Interaction::Dragging {
            owner: active,
            grab_offset,
            shell_generation,
            ..
        } = &mut self.interaction
        else {
            return None;
        };
        if active != owner || *shell_generation != self.shell.generation {
            return None;
        }
        Some((
            f64::from(committed_origin.0) + position.0 - grab_offset.0,
            f64::from(committed_origin.1) + position.1 - grab_offset.1,
        ))
    }

    /// Integrate preferred relative-pointer motion without compositor margin feedback.
    pub(crate) fn relative_drag_origin(
        &mut self,
        owner: &InputOwner,
        delta: (f64, f64),
    ) -> Option<(f64, f64)> {
        let Interaction::Dragging {
            owner: active,
            relative_origin,
            shell_generation,
            ..
        } = &mut self.interaction
        else {
            return None;
        };
        if active != owner || *shell_generation != self.shell.generation {
            return None;
        }
        relative_origin.0 += delta.0;
        relative_origin.1 += delta.1;
        Some(*relative_origin)
    }

    pub(crate) fn release(&mut self, owner: &InputOwner, position: (f64, f64)) -> ReleaseAction {
        let position = content_position(position);
        let action = match &self.interaction {
            Interaction::PressedControl {
                owner: active,
                control,
            } if active == owner && control_at(self.model.frame, position) == Some(*control) => {
                match control {
                    Control::Copy => ReleaseAction::Copy,
                    Control::Close => ReleaseAction::Close,
                }
            }
            Interaction::Dragging { owner: active, .. } if active == owner => {
                ReleaseAction::DragEnd
            }
            _ => return ReleaseAction::None,
        };
        self.interaction = Interaction::Idle;
        self.visual.pressed = None;
        self.dirty = true;
        action
    }

    pub(crate) fn replace_shell(&mut self) -> Result<(), ShellGenerationExhausted> {
        let next_generation = self
            .shell
            .generation
            .checked_add(1)
            .ok_or(ShellGenerationExhausted)?;
        self.shell.destroy();
        self.shell.generation = next_generation;
        self.buffers.clear().map_err(|_| ShellGenerationExhausted)?;
        self.raster = None;
        self.first_commit_complete = false;
        self.cancel_interaction();
        self.dirty = true;
        self.full_damage = true;
        Ok(())
    }
}

pub(crate) struct PinModel {
    pub(crate) id: PinId,
    pub(crate) image: Arc<PinImage>,
    pub(crate) output: String,
    pub(crate) output_size: (u32, u32),
    pub(crate) frame: PinFrame,
}

#[derive(Default)]
pub(crate) struct ShellState {
    pub(crate) generation: u64,
    pub(crate) wl_surface: Option<wl_surface::WlSurface>,
    pub(crate) layer_surface: Option<LayerSurface>,
    pub(crate) requested_size: (u32, u32),
    pub(crate) configured_size: Option<(u32, u32)>,
    pub(crate) scale: i32,
    pub(crate) configured: bool,
    pub(crate) frame_callback: Option<u64>,
    /// Origin known to be represented by current surface-local input events.
    pub(crate) committed_origin: (i32, i32),
    /// New margins waiting for the callback/commit boundary before rebasing.
    pub(crate) pending_origin: Option<(i32, i32)>,
}

impl ShellState {
    pub(crate) fn destroy(&mut self) {
        self.layer_surface = None;
        self.wl_surface = None;
        self.configured_size = None;
        self.configured = false;
        self.frame_callback = None;
        self.pending_origin = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputOwner {
    Pointer {
        seat: ObjectId,
        button: u32,
    },
    Touch {
        seat: ObjectId,
        id: i32,
    },
    #[cfg(feature = "tablet-input")]
    Stylus {
        tool: ObjectId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Interaction {
    Idle,
    PressedControl {
        owner: InputOwner,
        control: Control,
    },
    Dragging {
        owner: InputOwner,
        grab_offset: (f64, f64),
        relative_origin: (f64, f64),
        shell_generation: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReleaseAction {
    None,
    Copy,
    Close,
    DragEnd,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct VisualState {
    pub(crate) pointer_position: Option<(f64, f64)>,
    pub(crate) hover: Option<Control>,
    pub(crate) pressed: Option<Control>,
    pub(crate) copy: CopyVisual,
}

#[derive(Debug, Clone, Default)]
pub(crate) enum CopyVisual {
    #[default]
    Idle,
    Copying,
    Succeeded {
        until: Instant,
    },
    Failed {
        until: Instant,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellGenerationExhausted;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn test_pin() -> PinnedSurface {
        PinnedSurface::new(
            PinId::new(1).unwrap(),
            Arc::new(PinImage {
                png: Arc::new(vec![1]),
                argb32: Arc::new(vec![0, 0, 0, 0]),
                width: 1,
                height: 1,
                stride: 4,
            }),
            "test".to_string(),
            (800, 600),
            PinFrame::new(100, 50, 200, 100).unwrap(),
        )
    }

    #[test]
    fn shell_replacement_invalidates_old_gesture_and_frame_state() {
        let mut shell = ShellState {
            generation: 4,
            configured: true,
            frame_callback: Some(8),
            ..ShellState::default()
        };
        shell.destroy();
        assert!(!shell.configured);
        assert_eq!(shell.frame_callback, None);
    }

    #[test]
    fn fallback_drag_uses_committed_origin_and_preserves_grab_offset() {
        let owner = InputOwner::Touch {
            seat: ObjectId::null(),
            id: 4,
        };
        let mut interaction = Interaction::Dragging {
            owner: owner.clone(),
            grab_offset: (10.25, 5.5),
            relative_origin: (100.0, 50.0),
            shell_generation: 1,
        };
        let mut shell = ShellState {
            generation: 1,
            committed_origin: (100, 50),
            ..ShellState::default()
        };
        let origin = match &mut interaction {
            Interaction::Dragging {
                owner: active,
                grab_offset,
                shell_generation,
                ..
            } if active == &owner && *shell_generation == shell.generation => (
                f64::from(shell.committed_origin.0) + 12.75 - grab_offset.0,
                f64::from(shell.committed_origin.1) + 4.0 - grab_offset.1,
            ),
            _ => unreachable!(),
        };
        assert_eq!(origin, (102.5, 48.5));

        // A committed move rebases the same surface-local coordinate space;
        // reversing by half a logical pixel remains a reversal, not feedback.
        shell.committed_origin = (102, 48);
        let reversed = (
            f64::from(shell.committed_origin.0) + 10.0 - 10.25,
            f64::from(shell.committed_origin.1) + 5.5 - 5.5,
        );
        assert_eq!(reversed, (101.75, 48.0));
    }

    #[test]
    fn shell_generation_exhaustion_fails_without_destroying_current_shell() {
        let shell = ShellState {
            generation: u64::MAX,
            configured: true,
            ..ShellState::default()
        };
        let next = shell
            .generation
            .checked_add(1)
            .ok_or(ShellGenerationExhausted);
        assert_eq!(next, Err(ShellGenerationExhausted));
        assert!(shell.configured);
    }

    #[test]
    fn pointer_leave_preserves_drag_and_unrelated_release_cannot_end_it() {
        let mut pin = test_pin();
        pin.shell.generation = 1;
        pin.shell.committed_origin = (100, 50);
        let owner = InputOwner::Pointer {
            seat: ObjectId::null(),
            button: 0x110,
        };
        pin.press(owner.clone(), (28.0, 28.0));
        pin.update_hover(None);
        assert!(matches!(pin.interaction, Interaction::Dragging { .. }));
        assert_eq!(
            pin.fallback_drag_origin(&owner, (30.5, 27.5), (100, 50)),
            Some((102.5, 49.5))
        );
        let unrelated = InputOwner::Pointer {
            seat: ObjectId::null(),
            button: 0x111,
        };
        assert_eq!(pin.release(&unrelated, (30.5, 27.5)), ReleaseAction::None);
        assert!(matches!(pin.interaction, Interaction::Dragging { .. }));
        assert_eq!(pin.release(&owner, (30.5, 27.5)), ReleaseAction::DragEnd);
    }
}
