# native-ocr

Text recognition with the OCR engine built into the operating system, so your app doesn't need to ship an OCR model or native library of its own.

| Platform | Engine | Languages |
|---|---|---|
| Windows 10 and 11 | `Windows.Media.Ocr` | The OCR language packs installed for the user's languages |
| macOS | Vision, at its accurate recognition level | Built into the OS |
| Everywhere else | None yet | |

## Usage

```rust
use native_ocr::{Error, recognize_rgba};

match recognize_rgba(&pixels, width, height) {
	Ok(text) => println!("{text}"),
	Err(Error::NoLanguage) => println!("Install an OCR language pack in Windows' language settings."),
	Err(err) => println!("{err}"),
}
```

`pixels` is an RGBA8 bitmap: 4 bytes per pixel, rows top to bottom with no padding. The result is the recognized lines joined with newlines, in the order the engine reads them.

Recognition blocks, so call it from a worker thread in a GUI app. Use `max_image_dimension()` to render pages no larger than the engine works with, and `is_available()` to hide OCR features on platforms without an engine.

To try it on an image:

```sh
cargo run --example recognize -- page.png
```

## License

MIT
