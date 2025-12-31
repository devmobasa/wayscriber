/// CPU-side frozen image ready for Cairo rendering.
pub struct FrozenImage {
    pub width: u32,
    pub height: u32,
    pub stride: i32,
    pub data: Vec<u8>,
}
