#![feature(float_interpolation)]

/* Display a console when in debug mode, have just the window be open when in
 * release mode. We don't want users thinking this is some kind of bad program,
 * do we? */
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg_attr(debug_assertions, windows_subsystem = "console")]

use crate::path::EventPath;
use crate::window::NoTabletConnector;
use crate::path::EventCanvas;

/// Utility structures for interpolating curved paths from ordered collections
/// of points.
mod path;

/// Structures handling windowing and display structures.
mod window;

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
			};

			std::process::exit(exit);
		}
	};

	std::thread::spawn(move || {
		let device = stu::list_devices()
			.find(|connector| connector.info() == information)
			.unwrap();

		println!("{:04x}:{:04x}", device.info().vendor(), device.info().product());
		let mut device = device.connect()
			.map_err(|what| {
				println!("{:?}", what);
				println!("{}", what);
			}).unwrap();

		let caps = device.capability().unwrap();
		device.clear().unwrap();

		println!("LCD Width: {}", caps.width());
		println!("LCD Height: {}", caps.height());
		println!("Table Width: {}", caps.input_grid_width());
		println!("Table Height: {}", caps.input_grid_height());
		println!("Table Depth: {}", caps.input_grid_pressure());

		device.inking(true);

		let mut queue = device.queue().unwrap();
		let mut path = EventPath::new();
		let mut canvas = EventCanvas::new(caps.width(), caps.height());

		loop {
			let event = queue.recv().unwrap();
			path.process(event);
			canvas.process(event);
		}
	}).join().unwrap();

}
