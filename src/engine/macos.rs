//! Vision's text recognition. Vision ships its recognition languages with the OS, so there's no
//! language pack to install and no `NoLanguage` case in practice.

use std::ptr::null;

use objc2::{AnyThread, rc::autoreleasepool};
use objc2_core_foundation::{CFData, CFRetained};
use objc2_core_graphics::{
	CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
};
use objc2_foundation::{NSArray, NSDictionary};
use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

use crate::{Error, Result};

/// About a 300 DPI letter page. Vision documents no limit, and larger pages only cost memory.
pub const fn max_image_dimension() -> u32 {
	4000
}

pub fn recognize_rgba(rgba: &[u8], width: u32, height: u32) -> Result<String> {
	let image = cg_image(rgba, width, height)?;
	autoreleasepool(|_| recognize(&image))
}

fn cg_image(rgba: &[u8], width: u32, height: u32) -> Result<CFRetained<CGImage>> {
	let data = CFData::from_bytes(rgba);
	let provider = CGDataProvider::with_cf_data(Some(&data)).ok_or_else(|| failed("creating the data provider"))?;
	let space = CGColorSpace::new_device_rgb().ok_or_else(|| failed("creating the color space"))?;
	let (width, height) = (width as usize, height as usize);
	unsafe {
		CGImage::new(
			width,
			height,
			8,
			32,
			width * 4,
			Some(&space),
			CGBitmapInfo(CGImageAlphaInfo::Last.0),
			Some(&provider),
			null(),
			false,
			CGColorRenderingIntent::RenderingIntentDefault,
		)
	}
	.ok_or_else(|| failed("creating the image"))
}

fn recognize(image: &CGImage) -> Result<String> {
	let request = VNRecognizeTextRequest::new();
	// The fast level is for short strings in camera frames, not pages of body text.
	request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
	request.setUsesLanguageCorrection(true);
	let handler = unsafe {
		VNImageRequestHandler::initWithCGImage_options(VNImageRequestHandler::alloc(), image, &NSDictionary::new())
	};
	let as_request: &VNRequest = &request;
	handler
		.performRequests_error(&NSArray::from_slice(&[as_request]))
		.map_err(|err| Error::Failed(err.localizedDescription().to_string()))?;
	let Some(observations) = request.results() else {
		return Ok(String::new());
	};
	let lines: Vec<String> = observations
		.iter()
		.filter_map(|observation| observation.topCandidates(1).firstObject())
		.map(|candidate| candidate.string().to_string())
		.collect();
	Ok(lines.join("\n"))
}

fn failed(step: &str) -> Error {
	Error::Failed(format!("{step} failed"))
}
