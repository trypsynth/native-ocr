use crate::{Error, Result};

pub const fn max_image_dimension() -> u32 {
	0
}

pub const fn recognize_rgba(_rgba: &[u8], _width: u32, _height: u32) -> Result<String> {
	Err(Error::Unavailable)
}

pub const fn is_available() -> bool {
	false
}
