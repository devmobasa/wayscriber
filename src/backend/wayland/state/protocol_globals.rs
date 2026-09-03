use smithay_client_toolkit::{
    activation::ActivationState,
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState, pointer_constraints::PointerConstraintsState,
        relative_pointer::RelativePointerState,
    },
    shell::{wlr_layer::LayerShell, xdg::XdgShell},
    shm::Shm,
};

pub(in crate::backend::wayland) struct ProtocolGlobalsSeed {
    pub registry: RegistryState,
    pub compositor: CompositorState,
    pub layer_shell: Option<LayerShell>,
    pub xdg_shell: Option<XdgShell>,
    pub activation: Option<ActivationState>,
    pub shm: Shm,
    pub pointer_constraints: PointerConstraintsState,
    pub relative_pointer: RelativePointerState,
    pub output: OutputState,
    pub seat: SeatState,
}

/// Bound Wayland globals and the toolkit state that dispatches their events.
pub(in crate::backend::wayland) struct ProtocolGlobals {
    registry: RegistryState,
    compositor: CompositorState,
    layer_shell: Option<LayerShell>,
    xdg_shell: Option<XdgShell>,
    activation: Option<ActivationState>,
    shm: Shm,
    pointer_constraints: PointerConstraintsState,
    relative_pointer: RelativePointerState,
    output: OutputState,
    seat: SeatState,
}

impl ProtocolGlobals {
    pub(in crate::backend::wayland) fn from_seed(seed: ProtocolGlobalsSeed) -> Self {
        Self {
            registry: seed.registry,
            compositor: seed.compositor,
            layer_shell: seed.layer_shell,
            xdg_shell: seed.xdg_shell,
            activation: seed.activation,
            shm: seed.shm,
            pointer_constraints: seed.pointer_constraints,
            relative_pointer: seed.relative_pointer,
            output: seed.output,
            seat: seed.seat,
        }
    }

    pub(in crate::backend::wayland) fn registry_mut(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    pub(in crate::backend::wayland) fn compositor(&self) -> &CompositorState {
        &self.compositor
    }

    pub(in crate::backend::wayland) fn layer_shell(&self) -> Option<&LayerShell> {
        self.layer_shell.as_ref()
    }

    pub(in crate::backend::wayland) fn xdg_shell(&self) -> Option<&XdgShell> {
        self.xdg_shell.as_ref()
    }

    pub(in crate::backend::wayland) fn activation(&self) -> Option<&ActivationState> {
        self.activation.as_ref()
    }

    pub(in crate::backend::wayland) fn shm(&self) -> &Shm {
        &self.shm
    }

    pub(in crate::backend::wayland) fn shm_mut(&mut self) -> &mut Shm {
        &mut self.shm
    }

    pub(in crate::backend::wayland) fn pointer_constraints(&self) -> &PointerConstraintsState {
        &self.pointer_constraints
    }

    pub(in crate::backend::wayland) fn relative_pointer(&self) -> &RelativePointerState {
        &self.relative_pointer
    }

    pub(in crate::backend::wayland) fn output(&self) -> &OutputState {
        &self.output
    }

    pub(in crate::backend::wayland) fn output_mut(&mut self) -> &mut OutputState {
        &mut self.output
    }

    pub(in crate::backend::wayland) fn seat(&self) -> &SeatState {
        &self.seat
    }

    pub(in crate::backend::wayland) fn seat_mut(&mut self) -> &mut SeatState {
        &mut self.seat
    }
}
