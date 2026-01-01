#[cfg(tablet)]
use log::{debug, info, warn};

#[cfg(tablet)]
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;

use crate::config::Config;

use super::super::setup::WaylandSetup;

#[cfg(tablet)]
const TABLET_MANAGER_MAX_VERSION: u32 = 2;

#[cfg(tablet)]
pub(super) fn bind_tablet_manager(
    setup: &WaylandSetup,
    config: &Config,
) -> Option<ZwpTabletManagerV2> {
    if config.tablet.enabled {
        match setup.globals.bind::<ZwpTabletManagerV2, _, _>(
            &setup.qh,
            1..=TABLET_MANAGER_MAX_VERSION,
            (),
        ) {
            Ok(manager) => {
                info!("Bound zwp_tablet_manager_v2");
                Some(manager)
            }
            Err(err) => {
                warn!("Tablet protocol not available: {}", err);
                None
            }
        }
    } else {
        debug!("Tablet input disabled in config");
        None
    }
}
