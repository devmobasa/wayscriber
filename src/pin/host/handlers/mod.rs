use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer,
    delegate_pointer_constraints, delegate_registry, delegate_relative_pointer, delegate_seat,
    delegate_shm, delegate_touch,
};

use super::PinHost;

delegate_compositor!(PinHost);
delegate_output!(PinHost);
delegate_shm!(PinHost);
delegate_layer!(PinHost);
delegate_seat!(PinHost);
delegate_pointer!(PinHost);
delegate_touch!(PinHost);
delegate_pointer_constraints!(PinHost);
delegate_relative_pointer!(PinHost);
delegate_registry!(PinHost);

mod compositor;
mod dispatch;
mod layer;
mod output;
mod pointer;
mod pointer_constraints;
mod registry;
mod relative_pointer;
mod seat;
mod shm;
#[cfg(feature = "tablet-input")]
mod tablet;
mod touch;
