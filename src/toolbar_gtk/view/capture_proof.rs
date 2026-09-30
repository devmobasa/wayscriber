use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

#[derive(Clone, Copy)]
pub(super) struct ProofRender {
    pub serial: u8,
    pub frame_counter: i64,
}

pub(super) type ProofObservation = Rc<Cell<Option<ProofRender>>>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct CaptureProof {
        pub(super) texture: RefCell<Option<gdk::MemoryTexture>>,
        pub(super) picture: glib::WeakRef<gtk4::Picture>,
        pub(super) observed: RefCell<Option<ProofObservation>>,
        pub(super) serial: Cell<u8>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CaptureProof {
        const NAME: &'static str = "WayscriberCaptureProof";
        type Type = super::CaptureProof;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for CaptureProof {}

    impl gdk::subclass::prelude::PaintableImpl for CaptureProof {
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let texture = self.texture.borrow();
            let Some(texture) = texture.as_ref() else {
                return;
            };

            texture.snapshot(snapshot, width, height);

            if let Some(picture) = self.picture.upgrade()
                && let Some(clock) = picture.frame_clock()
                && let Some(observed) = self.observed.borrow().as_ref()
            {
                let serial = self.serial.get();
                let frame = clock.frame_counter();
                observed.set(Some(ProofRender {
                    serial,
                    frame_counter: frame,
                }));
            }
        }

        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::STATIC_CONTENTS | gdk::PaintableFlags::STATIC_SIZE
        }

        fn intrinsic_width(&self) -> i32 {
            1
        }

        fn intrinsic_height(&self) -> i32 {
            1
        }

        fn intrinsic_aspect_ratio(&self) -> f64 {
            1.0
        }
    }
}

glib::wrapper! {
    pub struct CaptureProof(ObjectSubclass<imp::CaptureProof>) @implements gdk::Paintable;
}

impl CaptureProof {
    pub(super) fn new(
        texture: &gdk::MemoryTexture,
        picture: &gtk4::Picture,
        observed: ProofObservation,
        serial: u8,
    ) -> Self {
        let proof: Self = glib::Object::new();
        proof.imp().texture.replace(Some(texture.clone()));
        proof.imp().picture.set(Some(picture));
        proof.imp().observed.replace(Some(observed));
        proof.imp().serial.set(serial);

        proof
    }
}
