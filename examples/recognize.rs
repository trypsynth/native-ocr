//! Prints the text in an image: `cargo run --example recognize -- page.png`

use std::{env, process::ExitCode};

use image::open;
use native_ocr::recognize_rgba;

fn main() -> ExitCode {
	let Some(path) = env::args().nth(1) else {
		eprintln!("usage: recognize <image>");
		return ExitCode::FAILURE;
	};
	let image = match open(&path) {
		Ok(image) => image.into_rgba8(),
		Err(err) => {
			eprintln!("couldn't open {path}: {err}");
			return ExitCode::FAILURE;
		}
	};
	match recognize_rgba(image.as_raw(), image.width(), image.height()) {
		Ok(text) => {
			println!("{text}");
			ExitCode::SUCCESS
		}
		Err(err) => {
			eprintln!("{err}");
			ExitCode::FAILURE
		}
	}
}
