//! Dedicated multi-surface pin host state and Wayland owner.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorState, Region},
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState, pointer::cursor_shape::CursorShapeManager,
        pointer_constraints::PointerConstraintsState, relative_pointer::RelativePointerState,
    },
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    },
    shm::Shm,
};
use wayland_client::{
    Connection, EventQueue, Proxy, QueueHandle,
    backend::ObjectId,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_touch},
};
#[cfg(feature = "tablet-input")]
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2,
};
use wayland_protocols::wp::{
    cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1,
    pointer_constraints::zv1::client::zwp_locked_pointer_v1::ZwpLockedPointerV1,
    relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
};

use crate::pin::{PinFrame, PinId, PinImage, PinMemoryCharge, PinMemoryLedger};

use super::surface::{PinnedSurface, ShellEventIdentity};

mod client_runtime;
mod clipboard;
mod event_loop;
mod exit_state;
mod handlers;
mod input;
mod output;
mod output_runtime;
mod ownership;
mod proxy_identity;
mod rendering;
mod timing;
mod worker_runtime;

use clipboard::ClipboardWorker;
use output::OutputInventory;

pub(crate) use event_loop::run_host;

const COPY_NOTICE_DURATION: Duration = Duration::from_secs(2);

pub(crate) struct PinHost {
    registry_state: RegistryState,
    compositor: CompositorState,
    shm: Shm,
    layer_shell: LayerShell,
    output_state: OutputState,
    seat_state: SeatState,
    pointer_constraints: PointerConstraintsState,
    relative_pointer_state: RelativePointerState,
    outputs: OutputInventory,
    pins: BTreeMap<PinId, PinnedSurface>,
    pin_charges: BTreeMap<PinId, PinCharge>,
    memory: PinMemoryLedger,
    by_wl_surface: HashMap<ObjectId, PinId>,
    pointers: HashMap<ObjectId, (wl_pointer::WlPointer, wl_seat::WlSeat)>,
    cursor_shape_manager: Option<CursorShapeManager>,
    cursor_shape_devices: HashMap<ObjectId, WpCursorShapeDeviceV1>,
    cursor_serials: HashMap<ObjectId, u32>,
    touches: HashMap<ObjectId, (wl_touch::WlTouch, wl_seat::WlSeat)>,
    active_touches: HashMap<(ObjectId, i32), ActiveTouch>,
    #[cfg(feature = "tablet-input")]
    tablet_manager: Option<ZwpTabletManagerV2>,
    #[cfg(feature = "tablet-input")]
    tablet_seats: HashMap<ObjectId, ZwpTabletSeatV2>,
    #[cfg(feature = "tablet-input")]
    tablet_tools: HashMap<ObjectId, ZwpTabletToolV2>,
    #[cfg(feature = "tablet-input")]
    tablet_tool_seats: HashMap<ObjectId, ObjectId>,
    #[cfg(feature = "tablet-input")]
    stylus_tools: HashMap<ObjectId, StylusToolState>,
    relative_pointer: Option<ZwpRelativePointerV1>,
    locked_pointer: Option<ZwpLockedPointerV1>,
    locked_pin: Option<PinId>,
    next_frame_token: Option<u64>,
    clipboard: ClipboardWorker,
    copy_generation: u64,
    newly_ready: Vec<PinId>,
    timings: HashMap<PinId, timing::PinTiming>,
    shutdown_armed: bool,
    should_exit: bool,
}

impl PinHost {
    pub(crate) fn connect() -> Result<(Connection, EventQueue<Self>, Self)> {
        let conn = Connection::connect_to_env().context("connect pin host to Wayland")?;
        let (globals, event_queue) =
            registry_queue_init(&conn).context("initialize pin host Wayland registry")?;
        let qh = event_queue.handle();
        let compositor =
            CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
        let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;
        let layer_shell =
            LayerShell::bind(&globals, &qh).context("layer-shell not available for Pin")?;
        let output_state = OutputState::new(&globals, &qh);
        let seat_state = SeatState::new(&globals, &qh);
        let cursor_shape_manager = CursorShapeManager::bind(&globals, &qh).ok();
        let registry_state = RegistryState::new(&globals);
        let pointer_constraints = PointerConstraintsState::bind(&globals, &qh);
        let relative_pointer_state = RelativePointerState::bind(&globals, &qh);
        #[cfg(feature = "tablet-input")]
        let tablet_manager = globals
            .bind::<ZwpTabletManagerV2, _, _>(&qh, 1..=2, ())
            .map_err(|error| log::debug!("Tablet protocol unavailable for pin host: {error}"))
            .ok();
        let state = Self {
            registry_state,
            compositor,
            shm,
            layer_shell,
            output_state,
            seat_state,
            pointer_constraints,
            relative_pointer_state,
            outputs: OutputInventory::default(),
            pins: BTreeMap::new(),
            pin_charges: BTreeMap::new(),
            memory: PinMemoryLedger::new(),
            by_wl_surface: HashMap::new(),
            pointers: HashMap::new(),
            cursor_shape_manager,
            cursor_shape_devices: HashMap::new(),
            cursor_serials: HashMap::new(),
            touches: HashMap::new(),
            active_touches: HashMap::new(),
            #[cfg(feature = "tablet-input")]
            tablet_manager,
            #[cfg(feature = "tablet-input")]
            tablet_seats: HashMap::new(),
            #[cfg(feature = "tablet-input")]
            tablet_tools: HashMap::new(),
            #[cfg(feature = "tablet-input")]
            tablet_tool_seats: HashMap::new(),
            #[cfg(feature = "tablet-input")]
            stylus_tools: HashMap::new(),
            relative_pointer: None,
            locked_pointer: None,
            locked_pin: None,
            next_frame_token: Some(0),
            clipboard: ClipboardWorker::default(),
            copy_generation: 0,
            newly_ready: Vec::new(),
            timings: HashMap::new(),
            shutdown_armed: false,
            should_exit: false,
        };
        Ok((conn, event_queue, state))
    }

    /// Insert an already validated and memory-admitted image into the runtime.
    pub(crate) fn insert_pin(
        &mut self,
        admission: PinAdmission,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let PinAdmission {
            id,
            image,
            connector,
            output_size,
            frame,
            charge,
        } = admission;
        if self.pins.contains_key(&id) {
            anyhow::bail!("pin id {id} is already live");
        }
        self.shutdown_armed = false;
        let mut pin = PinnedSurface::new(id, image, connector.clone(), output_size, frame);
        pin.shell.generation = 1;
        pin.shell.scale = 1;
        self.pins.insert(id, pin);
        self.pin_charges.insert(id, charge);
        if let Some(output) = self.outputs.by_connector(&connector).cloned()
            && let Err(error) = self.create_shell(id, &output.proxy, output.scale, qh)
        {
            self.remove_proxy_routes(id);
            if let Some(mut pin) = self.pins.remove(&id) {
                pin.shell.destroy();
            }
            self.pin_charges.remove(&id);
            return Err(error);
        }
        Ok(())
    }

    fn create_shell(
        &mut self,
        id: PinId,
        output: &wl_output::WlOutput,
        scale: i32,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let pin = self
            .pins
            .get_mut(&id)
            .context("pin disappeared before shell creation")?;
        let frame = pin.model.frame;
        let surface_size =
            super::surface::surface_size(frame).context("pin chrome size overflow")?;
        let surface_origin = super::surface::surface_origin(frame);
        let wl_surface = self.compositor.create_surface(qh);
        wl_surface.set_buffer_scale(scale.max(1));
        let input_region = Region::new(&self.compositor).context("create pin input region")?;
        input_region.add(
            i32::try_from(super::surface::CHROME_PADDING)?,
            i32::try_from(super::surface::CHROME_PADDING)?,
            i32::try_from(frame.width)?,
            i32::try_from(frame.height)?,
        );
        wl_surface.set_input_region(Some(input_region.wl_region()));
        let layer = self.layer_shell.create_layer_surface(
            qh,
            wl_surface.clone(),
            Layer::Overlay,
            Some("wayscriber-pin"),
            Some(output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(-1);
        layer.set_margin(surface_origin.1, 0, 0, surface_origin.0);
        layer.set_size(surface_size.0, surface_size.1);
        layer.commit();

        pin.shell.wl_surface = Some(wl_surface.clone());
        pin.shell.layer_surface = Some(layer.clone());
        pin.shell.requested_size = surface_size;
        pin.shell.configured_size = None;
        pin.shell.scale = scale.max(1);
        pin.shell.configured = false;
        pin.shell.committed_origin = (frame.x, frame.y);
        pin.shell.pending_origin = None;
        self.by_wl_surface.insert(wl_surface.id(), id);
        Ok(())
    }

    fn unlock_pointer(&mut self) {
        if let Some(relative) = self.relative_pointer.take() {
            relative.destroy();
        }
        if let Some(locked) = self.locked_pointer.take() {
            locked.destroy();
        }
        self.locked_pin = None;
    }

    fn next_callback_identity(&mut self, pin_id: PinId) -> Result<ShellEventIdentity> {
        let token = self
            .next_frame_token
            .and_then(|token| token.checked_add(1))
            .context("pin frame callback identity exhausted")?;
        self.next_frame_token = Some(token);
        let shell_generation = self
            .pins
            .get(&pin_id)
            .context("pin disappeared before callback request")?
            .shell
            .generation;
        Ok(ShellEventIdentity {
            pin_id,
            shell_generation,
            token,
        })
    }

    pub(super) fn commit_frame(
        &mut self,
        id: PinId,
        frame: PinFrame,
        resized: bool,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let needs_callback = self
            .pins
            .get(&id)
            .context("pin disappeared before geometry commit")?
            .shell
            .frame_callback
            .is_none();
        let identity = needs_callback
            .then(|| self.next_callback_identity(id))
            .transpose()?;
        let mut replacement_surface = None;
        if resized {
            let scale = self
                .pins
                .get(&id)
                .context("pin disappeared before resize reservation")?
                .shell
                .scale
                .max(1) as u32;
            let size = super::surface::surface_size(frame).context("pin chrome size overflow")?;
            let charge = PinMemoryCharge::for_surface(size.0, size.1, scale)
                .map_err(|error| anyhow::anyhow!(error))?;
            self.memory
                .try_reserve(charge)
                .map_err(|error| anyhow::anyhow!(error))?;
            replacement_surface = Some(charge);
        }
        let pin = self
            .pins
            .get_mut(&id)
            .context("pin disappeared before geometry commit")?;
        let mut retained = false;
        pin.model.frame = frame;
        let layer = pin
            .shell
            .layer_surface
            .as_ref()
            .context("cannot move dormant pin")?;
        let surface_origin = super::surface::surface_origin(frame);
        layer.set_margin(surface_origin.1, 0, 0, surface_origin.0);
        if resized {
            let size = super::surface::surface_size(frame).context("pin chrome size overflow")?;
            layer.set_size(size.0, size.1);
            pin.shell.requested_size = size;
            pin.shell.configured = false;
            pin.raster = None;
            retained = match pin.buffers.clear_and_report_retention() {
                Ok(retained) => retained,
                Err(error) => {
                    if let Some(charge) = replacement_surface {
                        self.memory
                            .release(charge)
                            .map_err(|reason| anyhow::anyhow!(reason))?;
                    }
                    return Err(error);
                }
            };
            pin.full_damage = true;
            pin.dirty = true;
        }
        let wl_surface = layer.wl_surface();
        if let Some(identity) = identity {
            wl_surface.frame(qh, identity);
            pin.shell.frame_callback = Some(identity.token);
        }
        wl_surface.commit();
        pin.shell.pending_origin = Some((frame.x, frame.y));
        if let Some(surface) = replacement_surface {
            let charges = self
                .pin_charges
                .get_mut(&id)
                .context("pin memory charge disappeared")?;
            let old = std::mem::replace(&mut charges.surface, surface);
            if retained {
                charges.retired_surfaces.push(old);
            } else {
                self.memory
                    .release(old)
                    .map_err(|error| anyhow::anyhow!(error))?;
            }
            log::debug!(
                "Pin {id} interactive surface replaced: resident={} peak={}",
                self.memory.resident_bytes(),
                self.memory.peak_bytes()
            );
        }
        Ok(())
    }

    pub(super) fn cancel_owner_for_pointer(&mut self, pointer: &wl_pointer::WlPointer) {
        let Some((_, seat)) = self.pointers.get(&pointer.id()) else {
            return;
        };
        let seat_id = seat.id();
        let mut cancelled_pins = Vec::new();
        for (pin_id, pin) in &mut self.pins {
            let owned = matches!(
                &pin.interaction,
                super::surface::Interaction::PressedControl {
                    owner: super::surface::InputOwner::Pointer { seat, .. },
                    ..
                } | super::surface::Interaction::Dragging {
                    owner: super::surface::InputOwner::Pointer { seat, .. },
                    ..
                } if *seat == seat_id
            );
            if owned {
                cancelled_pins.push(*pin_id);
                pin.cancel_interaction();
            }
        }
        if self
            .locked_pin
            .is_some_and(|pin| cancelled_pins.contains(&pin))
        {
            self.unlock_pointer();
        }
    }
}

pub(crate) struct PinAdmission {
    id: PinId,
    image: Arc<PinImage>,
    connector: String,
    output_size: (u32, u32),
    frame: PinFrame,
    charge: PinCharge,
}

impl PinAdmission {
    pub(crate) fn new(
        id: PinId,
        image: Arc<PinImage>,
        connector: String,
        output_size: (u32, u32),
        frame: PinFrame,
        charge: PinCharge,
    ) -> Self {
        Self {
            id,
            image,
            connector,
            output_size,
            frame,
            charge,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PinCharge {
    image: PinMemoryCharge,
    surface: PinMemoryCharge,
    retired_surfaces: Vec<PinMemoryCharge>,
}

impl PinCharge {
    pub(crate) fn new(image: PinMemoryCharge, surface: PinMemoryCharge) -> Self {
        Self {
            image,
            surface,
            retired_surfaces: Vec::new(),
        }
    }

    fn combined(self) -> Result<PinMemoryCharge, crate::pin::PinRefusal> {
        self.retired_surfaces.into_iter().try_fold(
            self.image.checked_combined(self.surface)?,
            |total, charge| total.checked_combined(charge),
        )
    }

    fn retained_png_charge(&self) -> PinMemoryCharge {
        PinMemoryCharge::from_parts(self.image.retained_png, 0, 0, 0, 0)
    }

    fn without_retained_png(self) -> Result<PinMemoryCharge, crate::pin::PinRefusal> {
        let image =
            PinMemoryCharge::from_parts(0, self.image.decoded_source, 0, 0, self.image.metadata);
        self.retired_surfaces
            .into_iter()
            .try_fold(image.checked_combined(self.surface)?, |total, charge| {
                total.checked_combined(charge)
            })
    }
}

#[derive(Debug, Clone, Copy)]
struct ActiveTouch {
    pin_id: PinId,
    position: (f64, f64),
}

#[cfg(feature = "tablet-input")]
#[derive(Debug, Clone, Copy, Default)]
struct StylusToolState {
    pin_id: Option<PinId>,
    position: (f64, f64),
    pending_position: Option<(f64, f64)>,
    tip_down: bool,
}
