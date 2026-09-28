use std::{error::Error as StdError, fmt, result::Result as StdResult};

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
