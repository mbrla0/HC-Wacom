
/// Tablet management window and logic.
mod manager;

/// Device selector window and logic.
mod selector;

/// Screen area selector window and logic.
mod area;

/// Initialize globals required by the windowing interface.
pub fn init() {
	nwg::init().expect("Could not initialize Win32 UI framework.");
	unsafe {
		/* Prevent the system from giving us the wrong system parameters, since
		 * we need to work with physical pixels, rather than with logical ones.
		 */
		winapi::um::winuser::SetProcessDPIAware();
	}

	nwg::Font::set_global_family("Segoe UI").unwrap();

	let mut font = Default::default();
	nwg::Font::builder()
		.family("Segoe UI")
		.size(16)
		.build(&mut font)
		.unwrap();
	nwg::Font::set_global_default(Some(font)).unwrap();
}

/* Re-export the user-facing functionality in our modules. */
pub use manager::{manage, ManagementError};
pub use selector::{pick_tablet, NoTabletConnector};
pub use area::{pick_physical_area, PickPhysicalAreaError, AreaSelectionParameters};
