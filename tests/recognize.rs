use image::open;
use native_ocr::{Error, is_available, recognize_rgba};

#[test]
fn reads_the_sample() {
	let image = open("tests/sample.png").expect("sample image").into_rgba8();
	match recognize_rgba(image.as_raw(), image.width(), image.height()) {
		Ok(text) => {
			let text = text.to_lowercase();
			assert!(text.contains("quick brown fox"), "{text}");
			assert!(text.contains("windows 10, 2026"), "{text}");
		}
		// CI machines may have no Tesseract, or an engine but no language to run it with.
		Err(Error::NoLanguage | Error::NotInstalled) => {}
		Err(Error::Unavailable) => assert!(!is_available()),
		Err(err) => panic!("{err}"),
	}
}

#[test]
fn rejects_a_mismatched_buffer() {
	assert!(matches!(recognize_rgba(&[0; 12], 2, 2), Err(Error::InvalidImage(_))));
	assert!(matches!(recognize_rgba(&[], 0, 0), Err(Error::InvalidImage(_))));
}
