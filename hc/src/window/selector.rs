use std::cell::RefCell;
use std::rc::Rc;

/// Prompt the user to pick a tablet device to connect to.
pub fn pick_tablet() -> Result<stu::Information, NoTabletConnector> {
	let mut devices = stu::list_devices()
		.map(|connector| connector.info())
		.collect::<Vec<_>>();
	if devices.len() == 0 {
		return Err(NoTabletConnector::NoDevicesAvailable)
	}

	let mut channel = Rc::new(RefCell::new(None));
	let _ = {
		let selection = DeviceSelection::new(devices, channel.clone());
		let _selection = nwg::NativeUi::build_ui(selection)
			.map_err(NoTabletConnector::WindowCreationError)?;
		nwg::dispatch_thread_events();
	};

	let connector = channel.borrow_mut().take();
	match connector {
		Some(connector) => Ok(connector),
		None => Err(NoTabletConnector::Cancelled)
	}
}

/// Error type enumerating all of the reasons for which no tablet connector may
/// be available after a call to [`pick_tablet_connector()`].
///
/// [`pick_tablet_connector()`]: pick_tablet_connector
#[derive(Debug, thiserror::Error)]
pub enum NoTabletConnector {
	/// This variant indicates that are no available devices.
	#[error("there are no available tablet devices")]
	NoDevicesAvailable,
	/// The user has cancelled the operation.
	#[error("the operation was cancelled")]
	Cancelled,
	/// The prompt window could not be created.
	#[error("the device prompt window could not be created: {0}")]
	WindowCreationError(nwg::NwgError),
}

/// A modal message window containing a device selection drop down menu.
#[derive(nwd::NwgUi)]
pub struct DeviceSelection {
	/// The icon we're gonna be using for the window.
	#[nwg_resource(source_system: Some(nwg::OemIcon::Information))]
	icon: nwg::Icon,

	/// The top level window this controller is contained in.
	#[nwg_control(
	title: "Tablet",
	flags: "WINDOW",
	center: true,
	icon: Some(&data.icon),
	size: (400, 100)
	)]
	#[nwg_events(
	OnInit: [Self::init],
	OnWindowClose: [Self::on_cancel]
	)]
	window: nwg::Window,

	/// The description of what should be done.
	#[nwg_control(
	text: "Select the tablet device you would like to connect to.",
	size: (380, 20),
	position: (10, 10)
	)]
	description: nwg::Label,

	/// The device connector selection box.
	#[nwg_control(
	size: (380, 40),
	position: (10, 30)
	)]
	selection: nwg::ComboBox<ConnectorDisplay>,

	/// The cancel button.
	///
	/// Having this button be clicked indicates that the user does not wish to
	/// connect to any devices and that the operation should be aborted.
	#[nwg_control(
	text: "Cancel",
	position: (290, 65)
	)]
	#[nwg_events(
	OnButtonClick: [Self::on_cancel]
	)]
	cancel: nwg::Button,

	/// The accept button.
	///
	/// Having this button be clicked indicates that the user wishes to connect
	/// to the device that is currently selected in the selection box.
	#[nwg_control(
	text: "Connect",
	position: (180, 65)
	)]
	#[nwg_events(
	OnButtonClick: [Self::on_accept]
	)]
	accept: nwg::Button,

	/// The list of table devices currently available to us.
	devices: RefCell<Vec<stu::Information>>,

	/// The channel through which we will provide our answer.
	channel: Rc<RefCell<Option<stu::Information>>>
}
impl DeviceSelection {
	/// Create a new device selection structure for the given connectors.
	fn new(
		devices: Vec<stu::Information>,
		channel: Rc<RefCell<Option<stu::Information>>>) -> Self {
		assert_ne!(
			devices.len(),
			0,
			"window::DeviceSelection controls must be initialized with device \
			lists with at least one element.");

		Self {
			icon: Default::default(),
			window: Default::default(),
			description: Default::default(),
			cancel: Default::default(),
			accept: Default::default(),
			selection: Default::default(),
			devices: RefCell::new(devices),
			channel
		}
	}

	/// Populates the data in the window controls.
	fn init(&self) {
		for device in self.devices.borrow_mut().drain(..) {
			self.selection
				.collection_mut()
				.push(ConnectorDisplay(Some(device)));
		}
		self.selection.sync();
		self.selection.set_selection(Some(0));
		self.selection.set_visible(true);

		self.window.set_visible(true);
		self.window.set_focus();
	}

	/// A source of cancellation intent has been fired.
	fn on_cancel(&self) {
		nwg::stop_thread_dispatch();
	}

	/// A source of acceptance intent has been fired.
	fn on_accept(&self) {
		let selection = self.selection.selection().unwrap();
		let selection = self.selection.collection_mut().swap_remove(selection);

		*RefCell::borrow_mut(&self.channel) = Some(selection.0.unwrap());
		nwg::stop_thread_dispatch();
	}
}

/// A structure that wraps a connector and provides a display implementation.
#[derive(Default)]
struct ConnectorDisplay(Option<stu::Information>);
impl std::fmt::Display for ConnectorDisplay {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		let info = self.0.as_ref().unwrap();

		write!(f, "{:04x}:{:04x}", info.vendor(), info.product())
	}
}
