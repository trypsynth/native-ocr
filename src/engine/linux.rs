//! Tesseract, loaded at run time from the system's `libtesseract` so nothing is linked or bundled.
//! Distros package the engine and each language's data separately, so either can be missing:
//! the library gives `NotInstalled`, the data `NoLanguage`.

use std::{
	env,
	ffi::{CStr, CString, c_char, c_int, c_uchar, c_void},
	fs,
	path::{Path, PathBuf},
	ptr::null_mut,
	sync::OnceLock,
};

use libloading::Library;

use crate::{Error, Result};

/// The sonames to try, newest first. The C API used here is the same in both.
const LIBRARIES: [&str; 2] = ["libtesseract.so.5", "libtesseract.so.4"];

/// Where distros put the language data, for when `TESSDATA_PREFIX` isn't set: Arch, Void and
/// openSUSE; Debian and Ubuntu; older Debian and Ubuntu; Fedora; source builds.
const TESSDATA_DIRS: [&str; 5] = [
	"/usr/share/tessdata",
	"/usr/share/tesseract-ocr/5/tessdata",
	"/usr/share/tesseract-ocr/4.00/tessdata",
	"/usr/share/tesseract/tessdata",
	"/usr/local/share/tessdata",
];

/// Data files that sit next to the languages but don't recognize text: orientation and script
/// detection, and equations.
const NOT_LANGUAGES: [&str; 2] = ["osd", "equ"];

type Handle = *mut c_void;

/// The functions used from Tesseract's C API, with the library they came from kept loaded.
struct Api {
	create: unsafe extern "C" fn() -> Handle,
	delete: unsafe extern "C" fn(Handle),
	set_variable: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
	init: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
	set_image: unsafe extern "C" fn(Handle, *const c_uchar, c_int, c_int, c_int, c_int),
	recognize: unsafe extern "C" fn(Handle, *mut c_void) -> c_int,
	get_utf8_text: unsafe extern "C" fn(Handle) -> *mut c_char,
	delete_text: unsafe extern "C" fn(*const c_char),
	_library: Library,
}

impl Api {
	fn load() -> Option<Self> {
		let library = LIBRARIES.into_iter().find_map(|name| unsafe { Library::new(name) }.ok())?;
		unsafe {
			Some(Self {
				create: *library.get(c"TessBaseAPICreate").ok()?,
				delete: *library.get(c"TessBaseAPIDelete").ok()?,
				set_variable: *library.get(c"TessBaseAPISetVariable").ok()?,
				init: *library.get(c"TessBaseAPIInit3").ok()?,
				set_image: *library.get(c"TessBaseAPISetImage").ok()?,
				recognize: *library.get(c"TessBaseAPIRecognize").ok()?,
				get_utf8_text: *library.get(c"TessBaseAPIGetUTF8Text").ok()?,
				delete_text: *library.get(c"TessDeleteText").ok()?,
				_library: library,
			})
		}
	}
}

fn api() -> Option<&'static Api> {
	static API: OnceLock<Option<Api>> = OnceLock::new();
	API.get_or_init(Api::load).as_ref()
}

pub fn is_available() -> bool {
	api().is_some()
}

/// About a 300 DPI letter page. Tesseract has no limit of its own, and larger pages only cost time.
pub const fn max_image_dimension() -> u32 {
	4000
}

pub fn recognize_rgba(rgba: &[u8], width: u32, height: u32) -> Result<String> {
	let api = api().ok_or(Error::NotInstalled)?;
	let (datapath, languages) = find_languages().ok_or(Error::NoLanguage)?;
	let dimension = |value: u32| c_int::try_from(value).map_err(|_| Error::InvalidImage("too large".to_string()));
	let (width, height) = (dimension(width)?, dimension(height)?);
	let datapath = cstring(datapath.as_os_str().as_encoded_bytes())?;
	let languages = cstring(languages.join("+").as_bytes())?;
	let engine = Engine::new(api)?;
	unsafe {
		// Tesseract logs progress and warnings to stderr, which is noise in a GUI app.
		(api.set_variable)(engine.0, c"debug_file".as_ptr(), c"/dev/null".as_ptr());
		if (api.init)(engine.0, datapath.as_ptr(), languages.as_ptr()) != 0 {
			return Err(Error::Failed("loading the language data failed".to_string()));
		}
		// Tesseract reads 4-byte pixels as RGB and skips the fourth byte, so RGBA goes in as is.
		(api.set_image)(engine.0, rgba.as_ptr(), width, height, 4, width * 4);
		if (api.recognize)(engine.0, null_mut()) != 0 {
			return Err(Error::Failed("recognizing failed".to_string()));
		}
		let text = (api.get_utf8_text)(engine.0);
		if text.is_null() {
			return Err(Error::Failed("reading the text failed".to_string()));
		}
		let lines = CStr::from_ptr(text)
			.to_string_lossy()
			.lines()
			.filter(|line| !line.trim().is_empty())
			.collect::<Vec<_>>()
			.join("\n");
		(api.delete_text)(text);
		Ok(lines)
	}
}

/// One `TessBaseAPI`, deleted when dropped. Tesseract's instances aren't thread-safe, so each
/// call makes its own.
struct Engine<'a>(Handle, &'a Api);

impl<'a> Engine<'a> {
	fn new(api: &'a Api) -> Result<Self> {
		let handle = unsafe { (api.create)() };
		if handle.is_null() {
			Err(Error::Failed("creating the engine failed".to_string()))
		} else {
			Ok(Self(handle, api))
		}
	}
}

impl Drop for Engine<'_> {
	fn drop(&mut self) {
		unsafe { (self.1.delete)(self.0) };
	}
}

fn cstring(bytes: &[u8]) -> Result<CString> {
	CString::new(bytes).map_err(|_| Error::Failed("a path or language name contains a NUL byte".to_string()))
}

/// The language data directory and the languages to load from it: the user's locale languages that
/// are installed, then English if it is. With none of those, every installed language, since
/// someone who installed only French data likely wants French whatever their locale says.
fn find_languages() -> Option<(PathBuf, Vec<String>)> {
	let (dir, installed) = tessdata_dirs().into_iter().find_map(|dir| {
		let installed = installed_languages(&dir);
		(!installed.is_empty()).then_some((dir, installed))
	})?;
	let mut chosen: Vec<String> = Vec::new();
	for language in locale_languages().into_iter().chain(["eng".to_string()]) {
		if installed.contains(&language) && !chosen.contains(&language) {
			chosen.push(language);
		}
	}
	if chosen.is_empty() {
		chosen = installed;
	}
	Some((dir, chosen))
}

fn tessdata_dirs() -> Vec<PathBuf> {
	let mut dirs = Vec::new();
	if let Some(prefix) = env::var_os("TESSDATA_PREFIX").filter(|prefix| !prefix.is_empty()) {
		let prefix = PathBuf::from(prefix);
		// Tesseract 4 and later take the data directory itself; 3 took its parent.
		dirs.push(prefix.join("tessdata"));
		dirs.push(prefix);
	}
	dirs.extend(TESSDATA_DIRS.iter().map(PathBuf::from));
	dirs
}

/// The languages with a `.traineddata` file in `dir`, sorted so the fallback order is stable.
fn installed_languages(dir: &Path) -> Vec<String> {
	let Ok(entries) = fs::read_dir(dir) else {
		return Vec::new();
	};
	let mut languages: Vec<String> = entries
		.filter_map(|entry| {
			let path = entry.ok()?.path();
			if path.extension()? != "traineddata" {
				return None;
			}
			let language = path.file_stem()?.to_str()?.to_string();
			(!NOT_LANGUAGES.contains(&language.as_str())).then_some(language)
		})
		.collect();
	languages.sort();
	languages
}

/// The user's languages as Tesseract names them, most preferred first, from the same variables
/// gettext reads.
fn locale_languages() -> Vec<String> {
	let mut locales: Vec<String> =
		env::var("LANGUAGE").map(|list| list.split(':').map(str::to_string).collect()).unwrap_or_default();
	locales.extend(["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|name| env::var(name).ok()));
	locales.iter().filter_map(|locale| tesseract_language(locale)).collect()
}

/// Maps a POSIX locale such as `pt_BR.UTF-8` or `sr_RS@latin` to Tesseract's name for its
/// language's data.
fn tesseract_language(locale: &str) -> Option<String> {
	let (locale, modifier) =
		locale.split_once('@').map_or((locale, None), |(locale, modifier)| (locale, Some(modifier)));
	let locale = locale.split('.').next()?;
	let (language, region) =
		locale.split_once('_').map_or((locale, None), |(language, region)| (language, Some(region)));
	let name = match (language, region, modifier) {
		("zh", Some("TW" | "HK" | "MO"), _) => "chi_tra",
		("zh", ..) => "chi_sim",
		("sr", _, Some("latin")) => "srp_latn",
		("uz", _, Some("cyrillic")) => "uzb_cyrl",
		_ => iso_639_2(language)?,
	};
	Some(name.to_string())
}

fn iso_639_2(code: &str) -> Option<&'static str> {
	Some(match code {
		"af" => "afr",
		"am" => "amh",
		"ar" => "ara",
		"az" => "aze",
		"be" => "bel",
		"bg" => "bul",
		"bn" => "ben",
		"bs" => "bos",
		"ca" => "cat",
		"cs" => "ces",
		"cy" => "cym",
		"da" => "dan",
		"de" => "deu",
		"el" => "ell",
		"en" => "eng",
		"eo" => "epo",
		"es" => "spa",
		"et" => "est",
		"eu" => "eus",
		"fa" => "fas",
		"fi" => "fin",
		"fr" => "fra",
		"ga" => "gle",
		"gl" => "glg",
		"gu" => "guj",
		"he" => "heb",
		"hi" => "hin",
		"hr" => "hrv",
		"hu" => "hun",
		"hy" => "hye",
		"id" => "ind",
		"is" => "isl",
		"it" => "ita",
		"ja" => "jpn",
		"ka" => "kat",
		"kk" => "kaz",
		"km" => "khm",
		"kn" => "kan",
		"ko" => "kor",
		"lo" => "lao",
		"lt" => "lit",
		"lv" => "lav",
		"mk" => "mkd",
		"ml" => "mal",
		"mn" => "mon",
		"mr" => "mar",
		"ms" => "msa",
		"mt" => "mlt",
		"my" => "mya",
		"nb" | "nn" | "no" => "nor",
		"ne" => "nep",
		"nl" => "nld",
		"pa" => "pan",
		"pl" => "pol",
		"pt" => "por",
		"ro" => "ron",
		"ru" => "rus",
		"si" => "sin",
		"sk" => "slk",
		"sl" => "slv",
		"sq" => "sqi",
		"sr" => "srp",
		"sv" => "swe",
		"sw" => "swa",
		"ta" => "tam",
		"te" => "tel",
		"th" => "tha",
		"tl" => "tgl",
		"tr" => "tur",
		"uk" => "ukr",
		"ur" => "urd",
		"uz" => "uzb",
		"vi" => "vie",
		"yi" => "yid",
		_ => return None,
	})
}

#[cfg(test)]
mod tests {
	use super::tesseract_language;

	#[test]
	fn maps_locales() {
		assert_eq!(tesseract_language("en_US.UTF-8").as_deref(), Some("eng"));
		assert_eq!(tesseract_language("pt_BR").as_deref(), Some("por"));
		assert_eq!(tesseract_language("zh_TW.UTF-8").as_deref(), Some("chi_tra"));
		assert_eq!(tesseract_language("zh_CN").as_deref(), Some("chi_sim"));
		assert_eq!(tesseract_language("sr_RS.UTF-8@latin").as_deref(), Some("srp_latn"));
		assert_eq!(tesseract_language("de").as_deref(), Some("deu"));
		assert_eq!(tesseract_language("C"), None);
		assert_eq!(tesseract_language("POSIX"), None);
	}
}
