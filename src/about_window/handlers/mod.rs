use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
};

use super::AboutWindowState;

mod compositor;
mod dispatch;
mod keyboard;
mod output;
mod pointer;
mod registry;
mod seat;
mod shm;
mod window;

delegate_compositor!(AboutWindowState);
delegate_output!(AboutWindowState);
delegate_shm!(AboutWindowState);
delegate_seat!(AboutWindowState);
delegate_keyboard!(AboutWindowState);
delegate_pointer!(AboutWindowState);
delegate_registry!(AboutWindowState);
delegate_xdg_shell!(AboutWindowState);
delegate_xdg_window!(AboutWindowState);
