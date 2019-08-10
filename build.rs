extern crate bindgen;

use std::env;
use std::path::PathBuf;

//#[cfg(not(windows))] // Just do branching with detect inside.
fn main() {
	// C:\Program Files\Azure Kinect SDK v1.1.0\sdk\include\k4a
	let target_os = env::var("CARGO_CFG_TARGET_OS");
	match target_os.as_ref().map(|x| &**x) {
		Ok("linux") | Ok("android") => {
			println!("cargo:rustc-link-lib=libk4a"); // libk4a1.1-dev
		},
		Ok("openbsd") | Ok("bitrig") | Ok("netbsd") | Ok("macos") | Ok("ios") => {
			println!("cargo:rustc-link-lib=libk4a");
		},
		Ok("windows") => {
			//rustup default stable-msvc!!!
			println!("cargo:rustc-link-search=native=.");
			println!("cargo:rustc-link-lib=k4a");
			println!("cargo:rustc-link-lib=k4abt");
			/*
			cargo:rustc-link-lib=static=foo
			cargo:rustc-link-search=native=/path/to/foo
			cargo:rustc-cfg=foo
			cargo:rustc-env=FOO=bar
			cargo:rustc-cdylib-link-arg=-Wl,-soname,libfoo.so.1.2.3
			*/
		},
		tos => panic!("unknown target os {:?}!", tos)
	}
	
	// The bindgen::Builder is the main entry point
	// to bindgen, and lets you build up options for
	// the resulting bindings.
	let bindings = bindgen::Builder::default()
		// The input header we would like to generate
		// bindings for.
		.header("wrapper.h")
		.clang_arg("-IC:\\Program Files\\Azure Kinect SDK v1.1.0\\sdk\\include")
		.clang_arg("-IC:\\Program Files\\Azure Kinect Body Tracking SDK\\sdk\\include")
		.constified_enum_module(r"k4a_.*_t")
		// Finish the builder and generate the bindings.
		.generate()
		// Unwrap the Result and panic on failure.
		.expect("Unable to generate bindings");
	
	// Write the bindings to the $OUT_DIR/bindings.rs file.
	let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
	bindings
		.write_to_file(out_path.join("bindings.rs"))
		.expect("Couldn't write bindings!");
}