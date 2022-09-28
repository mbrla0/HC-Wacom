/* Display a console when in debug mode, have just the window be open when in
 * release mode. We don't want users thinking this is some kind of bad program,
 * do we? */
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(debug_assertions, windows_subsystem = "console")]
use crate::window::bitmap::BitmapError;
use crate::window::NoTabletConnector;

/// Utility structures for interpolating curved paths from ordered collections
/// of points.
mod path;

/// Structures handling windowing and display structures.
mod window;

/// Structures handling the automated playback of user input.
mod robot;

/// Strings used in the UI.
mod strings;

fn main() {
	window::init();

	match window::bitmap::run(None) {
		Ok(_) => {},
		Err(BitmapError::Cancelled) => {
			println!("cancelled")
		},
		Err(what) => {
			nwg::error_message(
				&crate::strings::errors::title(),
				&*match what {
					BitmapError::Cancelled => unreachable!(),
					BitmapError::InvalidFile(what) => format!(
						"{}: {}",
						crate::strings::errors::invalid_file(),
						what),
					BitmapError::FileNotFound =>
						crate::strings::errors::file_not_found().to_string(),
					BitmapError::WindowCreationError(_) => panic!("")
				});
		}
	}
	return;

	let information = match window::pick_tablet() {
		Ok(information) => information,
		Err(what) => {
			let exit = match what {
				NoTabletConnector::Cancelled => 0,
				NoTabletConnector::NoDevicesAvailable => {
					nwg::error_message(
						&crate::strings::errors::title(),
						&crate::strings::errors::no_tablets_available());
					0
				}
				NoTabletConnector::WindowCreationError(what) => {
					nwg::error_message(
						&crate::strings::errors::title(),
						&crate::strings::errors::device_prompt_creation_failed(what));
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
			nwg::error_message(
				&crate::strings::errors::title(),
				&crate::strings::errors::tablet_not_found(information));

			std::process::exit(1);
		}
	};
	let device = match device.connect() {
		Ok(device) => device,
		Err(what) => {
			nwg::error_message(
				&crate::strings::errors::title(),
				&crate::strings::errors::tablet_connection_failed(information, what));

			std::process::exit(1);
		}
	};

	if let Err(what) = window::manage(device) {
		nwg::error_message(
			&crate::strings::errors::title(),
			&crate::strings::errors::management_failed(what));

		std::process::exit(1);
	}
}
