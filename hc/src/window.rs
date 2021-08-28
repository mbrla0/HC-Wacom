use std::rc::Rc;
use std::cell::RefCell;
use nwg::NativeUi;
use std::borrow::BorrowMut;
use crate::path::{EventCanvas, EventPath};
use stu::{Tablet, Queue, Capability};

/// Initialize globals required by the windowing interface.
pub fn init() {
	nwg::init().expect("Could not initialize Win32 UI framework.");
	nwg::Font::set_global_family("Segoe UI").unwrap();

	let mut font = Default::default();
	nwg::Font::builder()
		.family("Segoe UI")
		.size(16)
		.build(&mut font)
		.unwrap();
	nwg::Font::set_global_default(Some(font)).unwrap();
}

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
		let _selection = NativeUi::build_ui(selection)
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

		*(&(*self.channel)).borrow_mut() = Some(selection.0.unwrap());
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

/// Manage the given tablet device.
pub fn manage(device: Tablet) -> Result<(), ManagementError> {
	let queue = device.queue()
		.map_err(ManagementError::QueueCreationError)?;
	let caps = device.capability()
		.map_err(ManagementError::CapabilityQueryError)?;
	let (tx, rx) = std::sync::mpsc::channel();

	let window = ManagementWindow::new(
		device,
		queue,
		caps,
		tx);
	let _window = nwg::NativeUi::build_ui(window)
		.map_err(ManagementError::WindowCreationError)?;
	nwg::dispatch_thread_events();

	match rx.try_recv() {
		Ok(what) => Err(what),
		Err(_) => Ok(()),
	}
}

/// Tries running a device command and kills the manager if it fails.
macro_rules! mng_cmd_try {
	($this:expr, $e:expr) => {
		if let Err(what) = $e {
			$this.fail(ManagementError::DeviceCommandFailed(what));
			return
		}
	}
}

///
#[derive(nwd::NwgUi)]
pub struct ManagementWindow {
	/// The icon we're gonna be using for the window.
	#[nwg_resource(source_system: Some(nwg::OemIcon::Information))]
	icon: nwg::Icon,

	/// The top level window this controller is contained in.
	#[nwg_control(
		title: "Tablet",
		flags: "WINDOW|MINIMIZE_BOX",
		center: true,
		icon: Some(&data.icon),
		size: (800, 600)
	)]
	#[nwg_events(
		OnInit: [Self::init],
		OnWindowClose: [Self::on_exit]
	)]
	window: nwg::Window,

	/// The controller managing the display of the pen bitmap.
	#[nwg_control(
		background_color: Some([255, 255, 255]),
		position: (10, 30)
	)]
	display: nwg::ImageFrame,

	/// Label for the device display.
	#[nwg_control(
		text: "Screen Preview",
		position: (10, 10),
		size: (100, 20)
	)]
	display_label: nwg::Label,

	/// Button for clearing the signature.
	#[nwg_control(
		text: "Clear",
		position: (10, 130)
	)]
	#[nwg_events(
		OnButtonClick: [Self::on_clear_pressed]
	)]
	display_clear_btn: nwg::Button,

	/// The timer object whose job is to fire a callback for pulling in events
	/// from the tablet and updating user interface displays from tablet data.
	#[nwg_control(
		interval: std::time::Duration::new(0, 40_000_000),
		active: false,
		lifetime: None,
	)]
	#[nwg_events(
		OnTimerTick: [Self::on_update]
	)]
	update: nwg::AnimationTimer,

	/// The device we're connected to.
	device: Tablet,
	/// The queue though which we receive device updates.
	queue: RefCell<Queue>,

	/// The path accumulated from the events generated by the tablet.
	path: RefCell<EventPath>,
	/// The canvas accumulated from the events generated by the tablet.
	canvas: RefCell<EventCanvas>,

	/// The channel through which we communicate failures.
	fails: std::sync::mpsc::Sender<ManagementError>,
}
impl ManagementWindow {
	fn new(
		device: Tablet,
		queue: Queue,
		caps: Capability,
		fails: std::sync::mpsc::Sender<ManagementError>) -> Self {

		Self {
			icon: Default::default(),
			window: Default::default(),
			display: Default::default(),
			display_label: Default::default(),
			display_clear_btn: Default::default(),
			update: Default::default(),
			device,
			queue: RefCell::new(queue),
			path: Default::default(),
			canvas: RefCell::new(EventCanvas::new(caps.width(), caps.height())),
			fails
		}
	}

	/// Sets all the necessary conditions to return with the given error.
	fn fail(&self, what: ManagementError) {
		let _ = self.fails.send(what);
		nwg::stop_thread_dispatch();
	}

	/// Populates the data in the window controls.
	fn init(&self) {
		mng_cmd_try!(self, self.device.clear());
		mng_cmd_try!(self, self.device.inking(true));

		self.update(true);

		self.update.start();
		self.window.set_visible(true);
		self.window.set_focus();
	}

	/// Called when an intent for clearing the device screen has been fired.
	fn on_clear_pressed(&self) {
		mng_cmd_try!(self, self.device.inking(false));

		self.canvas.borrow_mut().clear();
		self.path.borrow_mut().clear();

		mng_cmd_try!(self, self.device.clear());
		mng_cmd_try!(self, self.device.inking(true));

		self.update(true);
	}

	/// Pulls in events from the device and repaints the screen.
	fn update(&self, force_repaint: bool) {
		/* Process the input events. */
		let mut queue = self.queue.borrow_mut();
		let mut canvas = self.canvas.borrow_mut();
		let mut path = self.path.borrow_mut();

		let mut dirty = false;
		loop {
			match queue.try_recv() {
				Ok(event) => {
					canvas.process(event);
					path.process(event);

					dirty = true;
				},
				Err(stu::TryRecvError::Empty) =>
				/* Done processing events for now. */
					break,
				Err(stu::TryRecvError::Failed(what)) => {
					/* */
					self.fail(ManagementError::DevicePollingFailed(what));
					return
				}
			}
		}

		/* Update the display after the changes made by the events. */
		if dirty || force_repaint {
			let blob = canvas.to_bitmap();
			let bitmap = nwg::Bitmap::from_bin(&blob[..]).unwrap();

			self.display.set_size(canvas.width(), canvas.height());
			self.display.set_bitmap(Some(&bitmap));

			eprintln!("{:?}", time.elapsed());
		}
	}

	/// Called when an update to the pen display preview has been requested.
	fn on_update(&self) {
		self.update(false)
	}

	/// Called when the window has been told to close.
	fn on_exit(&self) {
		self.on_clear_pressed();
		nwg::stop_thread_dispatch();
	}
}

/// This structure enumerates the reasons for which creation of a management
/// window may fail.
#[derive(Debug, thiserror::Error)]
pub enum ManagementError {
	/// The management window could not be created.
	#[error("could not create management window: {0}")]
	WindowCreationError(nwg::NwgError),
	/// The management window could not create the queue required to access the
	/// events generated by the tablet device and, thus cannot perform its job.
	#[error("could not create queue: {0}")]
	QueueCreationError(stu::Error),
	/// The management window could not poll for the capabilities of the tablet
	/// device we would be managing and, thus cannot perform its job.
	#[error("could not query for device capabilities: {0}")]
	CapabilityQueryError(stu::Error),
	/// While trying to poll events off the tablet device, we encountered a
	/// fatal error and had to terminate the management structure.
	#[error("device polling failed: {0}")]
	DevicePollingFailed(stu::Error),
	/// While trying to send a command off to the tablet device, we encountered
	/// a fatal error and had to terminate the management structure.
	#[error("device command failed: {0}")]
	DeviceCommandFailed(stu::Error),
}
