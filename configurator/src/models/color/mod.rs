mod input;
mod named;
mod quad;
mod triplet;

#[cfg(test)]
mod tests;

pub use input::ColorInput;
pub use named::{ColorMode, NamedColorOption};
pub use quad::ColorQuadInput;
pub use triplet::ColorTripletInput;
