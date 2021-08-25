use std::path::PathBuf;

/** The environment variable from which we will try to derive the location of
 * the root folder of the user's Wacom STU SDK installation. */
const ENV_WACOM_STU_HOME: &'static str = "WACOM_STU_SDK_HOME";

fn main() {
	println!("cargo:rerun-if-env-changed={}", ENV_WACOM_STU_HOME);

	let home = match std::env::var_os(ENV_WACOM_STU_HOME) {
		Some(home) => PathBuf::from(home),
		None =>
			panic!("Missing the required {} environment variable, which is \
				used to determine the root folder of the Wacom STU SDK",
				ENV_WACOM_STU_HOME)
	};

	/* Generate the bindings with the header file. */
	let header = home.join("C/include/WacomGSS/wgssSTU.h");
	if !header.exists() {
		panic!(
			"Missing the required C header file at {:?}",
			match header.canonicalize() {
				Ok(canonical) => canonical,
				Err(_) => header
			});
	}
	let header = match header.to_str() {
		Some(header) => header,
		None => panic!(
			"Path to required C header file at {:?} is not UTF-8 compatible",
			header)
	};

	let bind = bindgen::builder()
		.header(header)
		.generate();
	let bind = match bind {
		Ok(bind) => bind,
		Err(_) => panic!(
			"Could not generate bindings for C header file {}",
			header)
	};

	let target = format!("{}/generated.rs", std::env::var("OUT_DIR").unwrap());
	if let Err(what) = bind.write_to_file(target) {
		panic!("Could not write generated bindings to target file: {}", what)
	}

	/* Tell rustc what libraries we will be linking against. */
	let lib = home
		.join("C/lib/")
		.join(target_name())
		.join("wgssSTU.lib");
	if !lib.exists() {
		panic!(
			"Missing the required C library file at {:?}",
			match lib.canonicalize() {
				Ok(canonical) => canonical,
				Err(_) => lib
			});
	}
	let lib = match lib.parent().unwrap().to_str() {
		Some(lib) => lib,
		None => panic!(
			"Path to required C library file at {:?} is not UTF-8 compatible",
			lib)
	};

	println!("cargo:rustc-link-search={}", lib);
	println!("cargo:rustc-link-lib=wgssSTU");
}

/** Name of the current target, in Wacom's naming scheme. */
const fn target_name() -> &'static str {
	#[cfg(all(target_arch = "x86_64", target_os = "windows"))]
	let name = "x64";
	#[cfg(all(target_arch = "x86", target_os = "windows"))]
	let name = "Win32";
	#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
	let name = "Linux-x86_64";
	#[cfg(all(target_arch = "x86", target_os = "linux"))]
	let name = "Linux-i386";
	#[cfg(not(any(
		all(target_arch = "x86_64", target_os = "windows"),
		all(target_arch = "x86", target_os = "windows"),
		all(target_arch = "x86_64", target_os = "linux"),
		all(target_arch = "x86", target_os = "linux")
	)))]
	std::compile_error!("Unsupported target for the Wacom STU SDK");

	name
}
