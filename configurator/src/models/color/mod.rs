mod convert;
mod input;
mod named;
mod quad;
mod triplet;

#[cfg(test)]
mod tests;

pub use convert::{
    hex_from_rgb, hex_from_rgba, hsv_to_rgb, parse_hex, parse_quad_values, parse_triplet_values,
    rgb_to_hsv,
};
pub use input::ColorInput;
pub use named::{ColorMode, NamedColorOption};
pub use quad::ColorQuadInput;
pub use triplet::ColorTripletInput;
