#![feature(float_interpolation)]

/* Display a console when in debug mode, have just the window be open when in
 * release mode. We don't want users thinking this is some kind of bad program,
 * do we? */
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(debug_assertions, windows_subsystem = "console")]

use crate::window::NoTabletConnector;

/// Utility structures for interpolating curved paths from ordered collections
/// of points.
mod path;

/// Structures handling windowing and display structures.
mod window;

/// Structures handling the automated playback of user input.
mod robot;

fn main() {
	window::init();
	let information = match window::pick_tablet() {
		Ok(information) => information,
		Err(what) => {
			let exit = match what {
				NoTabletConnector::Cancelled => 0,
				NoTabletConnector::NoDevicesAvailable => {
					nwg::error_message(
						"Error",
						"There are no tablet devices available on the system");
					1
				}
				NoTabletConnector::WindowCreationError(what) => {
					let message = format!(
						"Could not create device prompt window: {}",
						what);
					nwg::error_message(
						"Error",
						&message);
					1
				}
			};

			std::process::exit(exit);
		}
	};

	let device = stu::list_devices()
		.find(|connector| connector.info() == information);
	let device = match device {
		Some(device) => device,
		None => {
			let message = format!(
				"Could not find {:04x}:{:04x}. Has the tablet been disconnected?",
				information.vendor(), information.product());
			nwg::error_message(
				"Tablet",
				&message);

			std::process::exit(1);
		}
	};
	let mut device = match device.connect() {
		Ok(device) => device,
		Err(what) => {
			let message = format!(
				"\
					Could not connect to {:04x}:{:04x}: {}.\n\n\
					\
					Error: {:?}\
				",
				information.vendor(),
				information.product(),
				what, what);

			nwg::error_message(
				"Tablet",
				&message);

			std::process::exit(1);
		}
	};

	if let Err(what) = window::manage(device) {
		let message = format!(
			"A fatal error has occurred: {}",
			what);
		nwg::error_message(
			"Tablet",
			&message);

		std::process::exit(1);
	}
}
