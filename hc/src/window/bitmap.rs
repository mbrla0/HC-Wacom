use std::cell::RefCell;
use std::num::NonZeroU32;
use std::time::Duration;
use nwg::{FileDialogAction, NoticeSender, NwgError};
use crate::path::BitmapPath;
use crate::robot::Playback;
use crate::window::{AreaSelectionParameters, PickPhysicalAreaError};

/// Run the bitmap procedure.
pub fn run(notify: Option<NoticeSender>) -> Result<(), BitmapError> {
	let mut file_dialog = Default::default();
	nwg::FileDialog::builder()
		.title(crate::strings::bitmap::file_select_title())
		.filters(format!("{}(*.png;*.jpg;*.bmp)|{}(*.*)",
			crate::strings::bitmap::file_select_filter_image(),
			crate::strings::bitmap::file_select_filter_all()))
		.action(FileDialogAction::Open)
		.multiselect(false)
		.build(&mut file_dialog)
		.unwrap();

	if !file_dialog.run::<nwg::ControlHandle>(None) {
		return Err(BitmapError::Cancelled)
	}
	let file = file_dialog.get_selected_item().unwrap();
	let file = image::open(&file)
		.map_err(BitmapError::InvalidFile)?;
	let file = file.to_luma8();

	/* Open the manager and pass the bitmap to it. */
	let (tx, rx) = std::sync::mpsc::channel();

	let window = BitmapWindow::new(BitmapPath::new(file), tx);
	let _window = nwg::NativeUi::build_ui(window)
		.map_err(BitmapError::WindowCreationError)?;

	nwg::dispatch_thread_events();
	if let Some(notify) = notify {
		notify.notice();
	}

	match rx.try_recv() {
		Ok(what) => Err(what),
		Err(_) => Ok(())
	}
}

#[derive(nwd::NwgUi)]
pub struct BitmapWindow {
	/// The icon we're gonna be using for the window.
	#[nwg_resource(source_bin: Some(crate::window::ICON))]
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
		position: (10, 40)
	)]
	display: nwg::ImageFrame,

	/// Label for the device display.
	#[nwg_control(
		position: (10, 12),
		size: (100, 20)
	)]
	display_label: nwg::Label,

	/// Button for cancelling the operation.
	#[nwg_control(
		position: (10, 150)
	)]
	#[nwg_events(
		OnButtonClick: [Self::on_cancel_pressed]
	)]
	cancel_btn: nwg::Button,

	/// Button for painting the signature.
	#[nwg_control(
		position: (110, 150)
	)]
	#[nwg_events(
		OnButtonClick: [Self::on_paint_pressed]
	)]
	display_paint_btn: nwg::Button,

	/// Whether the management window is currently locked.
	locked: RefCell<bool>,

	/// The path containing the signature data.
	path: RefCell<BitmapPath>,

	/// The notification channel through which we know the painting is done.
	#[nwg_control()]
	#[nwg_events(
		OnNotice: [Self::on_paint_done]
	)]
	display_paint_done: nwg::Notice,

	/// The notification channel through which we know the area selection is done.
	#[nwg_control()]
	#[nwg_events(
		OnNotice: [Self::on_area_done]
	)]
	area_selection_done: nwg::Notice,

	/// The channel through which we communicate failures.
	fails: std::sync::mpsc::Sender<BitmapError>,
}
impl BitmapWindow {
	fn new(
		path: BitmapPath,
		fails: std::sync::mpsc::Sender<BitmapError>) -> Self {

		Self {
			icon: Default::default(),
			window: Default::default(),
			display: Default::default(),
			display_label: Default::default(),
			cancel_btn: Default::default(),
			display_paint_btn: Default::default(),
			locked: RefCell::new(false),
			path: RefCell::new(path),
			display_paint_done: Default::default(),
			area_selection_done: Default::default(),
			fails
		}
	}

	/// Locks all of the controls in this window.
	fn lock(&self) {
		self.cancel_btn.set_enabled(false);
		self.display_paint_btn.set_enabled(false);
		*self.locked.borrow_mut() = true;
	}

	/// Unlocks all of the controls in this window.
	fn unlock(&self) {
		self.cancel_btn.set_enabled(true);
		self.display_paint_btn.set_enabled(true);
		*self.locked.borrow_mut() = false;
	}

	/// Sets all the necessary conditions to return with the given error.
	fn fail(&self, what: BitmapError) {
		let _ = self.fails.send(what);
		nwg::stop_thread_dispatch();
	}

	/// Populates the data in the window controls.
	fn init(&self) {
		self.window.set_text(&crate::strings::bitmap::title());
		self.display_paint_btn.set_text(&crate::strings::bitmap::display_paint_btn());
		self.cancel_btn.set_text(&crate::strings::bitmap::cancel_btn());
		self.display_label.set_text(&crate::strings::bitmap::display_label());

		self.update();

		self.window.set_visible(true);
		self.window.set_focus();
	}

	fn update(&self) {
		let path = self.path.borrow();
		let blob = path.to_bitmap();
		let bitmap = nwg::Bitmap::from_bin(&blob[..]).unwrap();

		self.display.set_size(path.width(), path.height());
		self.display.set_bitmap(Some(&bitmap));

		/* Move the UI around. */
		self.window.set_size(path.width() + 20, path.height() + 85);
		let (_, btn_height) = self.cancel_btn.size();
		let (_, lbl_height) = self.display_label.size();

		self.display_label.set_size(
			path.width().saturating_sub(80),
			lbl_height);
		self.cancel_btn.set_size(
			(path.width() / 2).saturating_sub(5),
			btn_height);
		self.display_paint_btn.set_size(
			(path.width() / 2).saturating_sub(5),
			btn_height);
		self.cancel_btn.set_position(
			10,
			lbl_height as i32 + 30 + path.height() as i32);
		self.display_paint_btn.set_position(
			(20 + (path.width() / 2).saturating_sub(5)) as i32,
			lbl_height as i32 + 30 + path.height() as i32);
	}

	/// Called when an intent for painting the device data has been fired.
	fn on_paint_pressed(&self) {
		self.lock();

		let path = self.path.borrow().clone();
		let done_sender = self.display_paint_done.sender();
		let area_sender = self.area_selection_done.sender();

		let width = path.width();
		let height = path.height();

		std::thread::spawn(move || {
			let area = super::pick_physical_area(AreaSelectionParameters {
				preferred_dimensions: (width, height)
			});
			let area = match area {
				Ok(area) => area,
				Err(PickPhysicalAreaError::Cancelled) => {
					area_sender.notice();
					return
				},
				Err(what) => {
					nwg::error_message(
						&crate::strings::errors::title(),
						&crate::strings::errors::signature_paint_pick_area_failed(what));
					area_sender.notice();
					return
				}
			};

			Playback {
				path,
				target: area,
				delta: Duration::from_secs(8),
				steps: unsafe { NonZeroU32::new_unchecked(5000) }
			}.play_and_notify(done_sender);
		});
	}

	/// Called when the painting of the signature has been completed.
	fn on_paint_done(&self) {
		nwg::stop_thread_dispatch();
	}

	/// Called when the painting of the signature has been completed.
	fn on_area_done(&self) {
		self.unlock();
	}

	/// Called when the window has been told to close.
	fn on_exit(&self) {
		nwg::stop_thread_dispatch();
	}

	/// Called when the cancel button is pressed.
	fn on_cancel_pressed(&self) {
		nwg::stop_thread_dispatch();
	}
}

#[derive(Debug, thiserror::Error)]
pub enum BitmapError {
	#[error("the bitmap insertion procedure was cancelled")]
	Cancelled,
	#[error("the bitmap file is invalid: {0}")]
	InvalidFile(image::ImageError),
	#[error("the bitmap file was not found")]
	FileNotFound,
	#[error("the window could not be created")]
	WindowCreationError(NwgError)
}