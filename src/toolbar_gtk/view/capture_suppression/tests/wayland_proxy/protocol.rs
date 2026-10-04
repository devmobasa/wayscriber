//! Message layouts from the protocol tables compiled into `wayland-client` and
//! `wayland-protocols`: core Wayland and stable xdg-shell, as the overlay links them.

use std::collections::HashMap;

use anyhow::{Context, Result};
use wayland_client::Proxy;
use wayland_client::backend::protocol::{
    ANONYMOUS_INTERFACE, ArgumentType, Interface, MessageDesc,
};
use wayland_client::protocol::{
    wl_compositor::WlCompositor, wl_data_device_manager::WlDataDeviceManager,
    wl_display::WlDisplay, wl_fixes::WlFixes, wl_output::WlOutput, wl_seat::WlSeat,
    wl_shell::WlShell, wl_shm::WlShm, wl_subcompositor::WlSubcompositor,
};
use wayland_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase;

/// Interfaces by name.
pub(super) type Schema = HashMap<&'static str, &'static Interface>;

#[derive(Clone, Copy, Debug)]
pub(super) enum Direction {
    Request,
    Event,
}

impl Direction {
    pub(super) fn messages(self, interface: &'static Interface) -> &'static [MessageDesc] {
        match self {
            Self::Request => interface.requests,
            Self::Event => interface.events,
        }
    }
}

/// The display, every global a client can bind, and every interface their
/// messages create or name.
pub(super) fn schema() -> Schema {
    let mut schema = Schema::new();
    let mut pending = vec![
        WlDisplay::interface(),
        WlCompositor::interface(),
        WlShm::interface(),
        WlDataDeviceManager::interface(),
        WlShell::interface(),
        WlSeat::interface(),
        WlOutput::interface(),
        WlSubcompositor::interface(),
        WlFixes::interface(),
        XdgWmBase::interface(),
    ];
    while let Some(interface) = pending.pop() {
        if std::ptr::eq(interface, &ANONYMOUS_INTERFACE)
            || schema.insert(interface.name, interface).is_some()
        {
            continue;
        }
        for message in interface.requests.iter().chain(interface.events) {
            pending.extend(message.child_interface);
            pending.extend(message.arg_interfaces);
        }
    }

    schema
}

/// One-word arguments by signature position, plus the objects a message creates.
pub(super) struct Arguments {
    words: Vec<Option<u32>>,
    pub(super) new_objects: Vec<(u32, String)>,
}

impl Arguments {
    pub(super) fn word(&self, position: usize) -> Result<u32> {
        self.words
            .get(position)
            .copied()
            .flatten()
            .with_context(|| format!("argument {position} is not a one-word value"))
    }
}

pub(super) fn decode(message: &MessageDesc, payload: &[u8]) -> Result<Arguments> {
    let mut reader = Payload {
        bytes: payload,
        offset: 0,
    };
    let mut arguments = Arguments {
        words: Vec::with_capacity(message.signature.len()),
        new_objects: Vec::new(),
    };
    // An untyped new_id arrives as the interface name, its version, then the ID.
    let mut named_interface = None;

    for kind in message.signature {
        let word = match kind {
            ArgumentType::Fd => None,
            ArgumentType::Str(_) => {
                named_interface = Some(reader.text()?);
                None
            }
            ArgumentType::Array => {
                reader.skip_array()?;
                None
            }
            ArgumentType::NewId => {
                let id = reader.word()?;
                let interface = match message.child_interface {
                    Some(interface) => interface.name.to_owned(),
                    None => named_interface
                        .take()
                        .context("untyped new_id without an interface name")?,
                };
                arguments.new_objects.push((id, interface));
                Some(id)
            }
            ArgumentType::Int
            | ArgumentType::Uint
            | ArgumentType::Fixed
            | ArgumentType::Object(_) => Some(reader.word()?),
        };
        arguments.words.push(word);
    }

    Ok(arguments)
}

struct Payload<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Payload<'_> {
    fn word(&mut self) -> Result<u32> {
        let word = self
            .bytes
            .get(self.offset..)
            .and_then(<[u8]>::first_chunk::<4>)
            .context("message payload ends inside an argument")?;
        self.offset += 4;

        Ok(u32::from_ne_bytes(*word))
    }

    /// Reads a length-prefixed string. A length past the payload end yields only
    /// the bytes present; a later argument read then fails.
    fn text(&mut self) -> Result<String> {
        let length = self.word()? as usize;
        let start = self.offset.min(self.bytes.len());
        let end = (self.offset + length).min(self.bytes.len());
        let raw = &self.bytes[start..end];
        self.offset += padded(length);

        let text_len = raw
            .iter()
            .rposition(|&byte| byte != 0)
            .map_or(0, |last| last + 1);
        String::from_utf8(raw[..text_len].to_vec()).context("Wayland string is not UTF-8")
    }

    fn skip_array(&mut self) -> Result<()> {
        let length = self.word()? as usize;
        self.offset += padded(length);

        Ok(())
    }
}

fn padded(length: usize) -> usize {
    length.next_multiple_of(4)
}

#[test]
fn schema_holds_every_core_and_xdg_shell_interface() {
    let mut names: Vec<_> = schema().into_keys().collect();
    names.sort_unstable();

    assert_eq!(
        names,
        [
            "wl_buffer",
            "wl_callback",
            "wl_compositor",
            "wl_data_device",
            "wl_data_device_manager",
            "wl_data_offer",
            "wl_data_source",
            "wl_display",
            "wl_fixes",
            "wl_keyboard",
            "wl_output",
            "wl_pointer",
            "wl_region",
            "wl_registry",
            "wl_seat",
            "wl_shell",
            "wl_shell_surface",
            "wl_shm",
            "wl_shm_pool",
            "wl_subcompositor",
            "wl_subsurface",
            "wl_surface",
            "wl_touch",
            "xdg_popup",
            "xdg_positioner",
            "xdg_surface",
            "xdg_toplevel",
            "xdg_wm_base",
        ]
    );
}

#[test]
fn decode_reports_typed_and_bound_objects() {
    let schema = schema();
    let words = |values: &[u32]| -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect()
    };

    let get_xdg_surface = message(
        &schema,
        "xdg_wm_base",
        Direction::Request,
        "get_xdg_surface",
    );
    let arguments = decode(get_xdg_surface, &words(&[7, 5])).unwrap();
    assert_eq!(arguments.new_objects, [(7, "xdg_surface".to_owned())]);
    assert_eq!(
        (arguments.word(0).unwrap(), arguments.word(1).unwrap()),
        (7, 5)
    );

    // bind(name, interface: "wl_compositor\0" padded to 16, version, id)
    let mut bind = words(&[3, 14]);
    bind.extend(b"wl_compositor\0\0\0");
    bind.extend(words(&[6, 9]));
    let registry_bind = message(&schema, "wl_registry", Direction::Request, "bind");
    let arguments = decode(registry_bind, &bind).unwrap();
    assert_eq!(arguments.new_objects, [(9, "wl_compositor".to_owned())]);
    assert!(
        arguments.word(1).is_err(),
        "a string is not a one-word value"
    );
}

fn message(
    schema: &Schema,
    interface: &str,
    direction: Direction,
    name: &str,
) -> &'static MessageDesc {
    direction
        .messages(schema[interface])
        .iter()
        .find(|message| message.name == name)
        .expect("message in the schema")
}
