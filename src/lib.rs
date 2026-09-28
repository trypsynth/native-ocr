//! Text recognition with the OCR engine built into the operating system.
//!
//! - Windows: `Windows.Media.Ocr`, using the OCR language packs installed for the user's
//!   languages.
//! - macOS: Vision's accurate text recognition, whose languages ship with the OS.
//! - Elsewhere: no engine yet, so [`is_available`] is `false` and recognizing returns
//!   [`Error::Unavailable`].
//!
//! Recognition blocks, so call it from a worker thread in a GUI app.

use std::{error::Error as StdError, fmt, result::Result as StdResult};

#[cfg_attr(target_os = "macos", path = "engine/macos.rs")]
#[cfg_attr(windows, path = "engine/windows.rs")]
#[cfg_attr(not(any(windows, target_os = "macos")), path = "engine/unavailable.rs")]
mod engine;

/// Why recognition produced no text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
	/// The engine has no recognizer for any of the user's languages. On Windows this means no OCR
	/// language pack is installed, the most likely failure outside the English-speaking world, so
	/// apps can tell the user how to fix it.
	NoLanguage,
	/// There's no OCR engine on this platform.
	Unavailable,
	/// The image isn't a valid RGBA8 bitmap of the size given.
	InvalidImage(String),
	/// The engine refused the image, or failed while recognizing it.
	Failed(String),
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NoLanguage => write!(f, "no OCR language is installed"),
			Self::Unavailable => write!(f, "there is no OCR engine on this platform"),
			Self::InvalidImage(detail) => write!(f, "invalid image: {detail}"),
			Self::Failed(detail) => write!(f, "OCR failed: {detail}"),
		}
	}
}

impl StdError for Error {}

pub type Result<T> = StdResult<T, Error>;

/// Whether this platform has an OCR engine. A `true` here can still fail with
/// [`Error::NoLanguage`] on Windows.
#[must_use]
pub const fn is_available() -> bool {
	cfg!(any(windows, target_os = "macos"))
}

/// The largest image side the engine works with, or 0 without an engine. Engines downscale
/// anything bigger, so there's no point rendering pages larger than this.
#[must_use]
#[allow(clippy::missing_const_for_fn, reason = "Windows asks the OS at run time")]
pub fn max_image_dimension() -> u32 {
	engine::max_image_dimension()
}

/// Recognizes the text in an RGBA8 bitmap, returning the lines joined with newlines.
///
/// The bitmap is 4 bytes per pixel, rows top to bottom with no padding, so `width * height * 4`
/// bytes. Lines come in the order the engine reads them.
///
/// # Errors
///
/// [`Error::InvalidImage`] when the byte count doesn't match the size, [`Error::Unavailable`]
/// without an engine, [`Error::NoLanguage`] when Windows has no OCR language pack for the user's
/// languages, and [`Error::Failed`] when the engine fails.
pub fn recognize_rgba(rgba: &[u8], width: u32, height: u32) -> Result<String> {
	let expected = (width as usize) * (height as usize) * 4;
	if width == 0 || height == 0 || rgba.len() != expected {
		return Err(Error::InvalidImage(format!("{width}x{height} needs {expected} bytes, got {}", rgba.len())));
	}
	engine::recognize_rgba(rgba, width, height)
}
