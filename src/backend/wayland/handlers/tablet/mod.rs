//! Wayland tablet/stylus protocol handling (zwp_tablet_v2).

use std::os::fd::OwnedFd;
use std::sync::Arc;
use wayland_client::backend::protocol::Message;
use wayland_client::backend::{Backend, ObjectData, ObjectId};

mod device;
mod manager;
mod pad;
mod pad_group;
mod pad_ring;
mod pad_strip;
mod seat;
mod tool;

#[derive(Debug)]
struct IgnoredObjectData;

impl ObjectData for IgnoredObjectData {
    fn event(
        self: Arc<Self>,
        _backend: &Backend,
        _msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>> {
        None
    }

    fn destroyed(&self, _object_id: ObjectId) {}
}
