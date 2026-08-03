mod convert;
mod input;
mod named;
mod quad;
mod triplet;
mod validation;

#[cfg(test)]
mod tests;

pub use convert::{
    hex_from_rgb, hex_from_rgba, parse_hex, parse_quad_values, parse_triplet_values,
};
pub use input::ColorInput;
pub use named::{ColorMode, NamedColorOption};
pub use quad::ColorQuadInput;
pub use triplet::ColorTripletInput;
pub(crate) use validation::hex_field_error;
