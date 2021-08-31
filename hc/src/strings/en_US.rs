
/// Strings used in the tablet management window.
pub mod manager {
	pub fn title() -> &'static str { "Tablet" }
	pub fn help_btn() -> &'static str { "Help" }
	pub fn help() -> &'static str { "Nothing here but us chickens!" }
	pub fn display_clear_btn() -> &'static str { "Clear" }
	pub fn display_paint_btn() -> &'static str { "Paint" }
	pub fn display_label() -> &'static str { "Display Controls" }
}

/// Strings used in the device selection window.
pub mod selector {
	pub fn title() -> &'static str { "Tablet" }
	pub fn description() -> &'static str { "Select the tablet device you would like to connect to." }
	pub fn cancel() -> &'static str { "Cancel" }
	pub fn accept() -> &'static str { "Connect" }
}

/// Strings used in the area selection window.
pub mod area {
	pub fn tip() -> &'static str {
		"Select a region by clicking and dragging. Press and hold the Alt key \
		to fix its aspect ratio. When done, press 'e' to paint on to the \
		selected region or 'q' to cancel."
	}
}

/// Strings used in error messages.
pub mod errors {
	pub fn title() -> &'static str { "Error" }
	pub fn signature_paint_pick_area_failed(
		what: crate::window::PickPhysicalAreaError) -> String {
		format!("Could not display paint controls: {}", what)
	}
	pub fn no_tablets_available() -> &'static str {
		"There are no tablet devices available on the system"
	}
	pub fn device_prompt_creation_failed(
		what: nwg::NwgError) -> String {
		format!("Could not create device prompt window: {}", what)
	}
	pub fn tablet_not_found(
		information: stu::Information) -> String {
		format!(
			"Could not find \"{} - {:04x}:{:04x}\". Has the tablet been disconnected?",
			information.device(), information.vendor(), information.product())
	}
	pub fn tablet_connection_failed(
		information: stu::Information,
		what: stu::Error) -> String {
		format!(
			"\
				Could not connect to \"{} - {:04x}:{:04x}\": {}.\n\n\
				\
				Error: {:?}\
			",
			information.device(),
			information.vendor(),
			information.product(),
			what, what)
	}
	pub fn management_failed(
		what: crate::window::ManagementError) -> String {
		format!(
			"An error has occurred while managing the device: {}",
			what)
	}
}
