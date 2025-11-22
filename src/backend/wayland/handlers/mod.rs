// Aggregates smithay handler implementations split across focused submodules and
// wires them to `WaylandState` via the delegate macros.
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
};

use super::state::WaylandState;

delegate_compositor!(WaylandState);
delegate_output!(WaylandState);
delegate_shm!(WaylandState);
delegate_layer!(WaylandState);
delegate_seat!(WaylandState);
delegate_keyboard!(WaylandState);
delegate_pointer!(WaylandState);
delegate_registry!(WaylandState);

mod buffer;
mod compositor;
mod keyboard;
mod layer;
mod output;
mod pointer;
mod registry;
mod screencopy;
mod seat;
mod shm;
