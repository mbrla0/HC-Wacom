use std::rc::Rc;
use std::cell::RefCell;
use nwg::NativeUi;
use std::borrow::BorrowMut;
use crate::path::{EventCanvas, EventPath};
use stu::{Tablet, Queue, Capability};
use crate::robot::{Playback, PhysicalArea};
use std::time::Duration;
use std::num::NonZeroU32;
use std::convert::TryFrom;

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

/// Display a window control that lets the user select a rectangular region on
/// the screen. This is intended for use with the signature painting
/// functionality.
pub fn pick_physical_area(
	parameters: AreaSelectionParameters)
	-> Result<PhysicalArea, PickPhysicalAreaError> {

	let (tx, rx) = std::sync::mpsc::channel();
	let window = AreaSelection::new(parameters, tx);
	let _window = nwg::NativeUi::build_ui(window)
		.map_err(PickPhysicalAreaError::WindowCreationError)?;

	nwg::dispatch_thread_events();
	match rx.recv() {
		Ok(result) => result,
		Err(_) => Err(PickPhysicalAreaError::Cancelled)
	}
}

/// Parameters controlling the prompt for picking a physical area on the screen.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct AreaSelectionParameters {
	/// The preferred width and height of the rectangle.
	pub preferred_dimensions: (u32, u32),
}

/// The structure controlling the physical area selection.
#[derive(nwd::NwgUi)]
pub struct AreaSelection {
	/// The top level window this controller is contained in.
	#[nwg_control(
		title: "Area Selection",
		flags: "WINDOW",
	)]
	#[nwg_events(
		OnInit: [Self::init],
		OnWindowClose: [Self::on_close],
		OnPaint: [Self::on_paint],
		OnMouseMove: [Self::on_mouse_move],
		OnMousePress: [Self::on_mouse_press(SELF, EVT)],
		OnKeyPress: [Self::on_key_press(SELF, EVT_DATA)]
	)]
	window: nwg::Window,

	/// The bitmap representing a screen capture.
	screen: RefCell<winapi::shared::windef::HBITMAP>,

	/// The parameters for the selection operation.
	params: AreaSelectionParameters,

	/// Whether the mouse is currently being pressed.
	mouse_pressed: RefCell<bool>,

	/// The position of the mouse when the button was pressed.
	mouse_anchor: RefCell<(i32, i32)>,

	/// The current area selection on the screen.
	selection: RefCell<PhysicalArea>,

	/// The channel through which we report our result.
	channel: std::sync::mpsc::Sender<Result<PhysicalArea, PickPhysicalAreaError>>,
}
impl AreaSelection {
	fn new(
		params: AreaSelectionParameters,
		channel:std::sync::mpsc::Sender<Result<PhysicalArea, PickPhysicalAreaError>>)
		-> Self {

		Self {
			window: Default::default(),
			screen: RefCell::new(std::ptr::null_mut()),
			params,
			mouse_pressed: RefCell::new(false),
			mouse_anchor: RefCell::new((0, 0)),
			selection: RefCell::new(PhysicalArea {
				x: 0,
				y: 0,
				width: 0,
				height: 0
			}),
			channel
		}
	}

	/// Called when a button on the mouse has been pressed or released.
	fn on_mouse_press(&self, event: nwg::Event) {
		if let nwg::Event::OnMousePress(event) = event {
			match event {
				nwg::MousePressEvent::MousePressLeftDown => {
					let anchor = nwg::GlobalCursor::position();

					*self.mouse_anchor.borrow_mut() = anchor;
					*self.selection.borrow_mut() = PhysicalArea {
						x: anchor.0.max(0) as u32,
						y: anchor.1.max(0) as u32,
						width: 0,
						height: 0
					};
					*self.mouse_pressed.borrow_mut() = true;
				},
				nwg::MousePressEvent::MousePressLeftUp => {
					*self.mouse_pressed.borrow_mut() = false;
				},
				_ => {},
			}
		}
	}

	/// Called when a key on the keyboard has been pressed.
	fn on_key_press(&self, data: &nwg::EventData) {
		let key = data.on_key();
		match key as _ {
			nwg::keys::_E => {
				let area = *self.selection.borrow();
				if area.width == 0 || area.height == 0 {
					return
				}

				self.channel.send(Ok(area));
				nwg::stop_thread_dispatch();
			},
			nwg::keys::_Q => {
				self.channel.send(Err(PickPhysicalAreaError::Cancelled));
				nwg::stop_thread_dispatch()
			},
			_ => {}
		}
	}

	/// Called when the mouse has moved on the screen.
	fn on_mouse_move(&self) {
		if !*self.mouse_pressed.borrow() { return }

		/* Resize the physical selection region. */
		let (x, y) = nwg::GlobalCursor::position();
		let x = x.max(0) as u32;
		let y = y.max(0) as u32;

		let (ax, ay) = *self.mouse_anchor.borrow();
		let ax = ax.max(0) as u32;
		let ay = ay.max(0) as u32;

		let (x, width) = if x < ax {(
			x,
			ax - x
		)} else {(
			ax,
			x - ax
		)};
		let (y, height) = if y < ay {(
			y,
			ay - y,
		)} else {(
			ay,
			y - ay
		)};
		*self.selection.borrow_mut() = PhysicalArea {
			x,
			y,
			width,
			height
		};

		/* Mark the window as being dirty. */
		unsafe {
			let hwnd = self.window.handle.hwnd().unwrap();

			use winapi::um::winuser as user;
			use winapi::um::errhandlingapi::GetLastError;

			let mut rect = std::mem::zeroed();
			let result = user::GetClientRect(hwnd, &mut rect);
			if result == 0 {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::on_mouse_move({:p})", self),
					message: format!("GetClientRect({:p}, {:p}) has failed: 0x{:08x}",
						hwnd, &rect, GetLastError())
				})
			}

			let result = user::InvalidateRect(hwnd, &rect, 0);
			if result == 0 {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::on_mouse_move({:p})", self),
					message: format!("InvalidateRect({:p}, {:p}, {}) has failed: 0x{:08x}",
						hwnd, &rect, 1, GetLastError())
				})
			}
		}
	}

	/// Called when the window has been closed.
	fn on_close(&self) {
		let _ = self.channel.send(Err(PickPhysicalAreaError::Cancelled));
		nwg::stop_thread_dispatch();
	}

	/// Called when the window has requested a repaint.
	fn on_paint(&self) {
		unsafe { self.paint() }
	}

	/// Paints the window.
	unsafe fn paint(&self) {
		let hwnd = self.window.handle.hwnd().unwrap();

		use winapi::um::winuser as user;
		use winapi::um::wingdi as gdi;
		use winapi::um::errhandlingapi::GetLastError;

		let mut paint = std::mem::zeroed();
		if user::BeginPaint(hwnd, &mut paint).is_null() {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!("BeginPaint({:p}, {:p}) failed: 0x{:08x}",
					hwnd,
					&mut paint,
					GetLastError())
			});
			return
		};

		/* Gather information on the window. */
		let mut rect = std::mem::zeroed();
		if user::GetClientRect(hwnd, &mut rect) == 0 {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!("GetClientRect({:p}, {:p}) failed: 0x{:08x}",
					hwnd,
					&mut rect,
					GetLastError())
			});
			return
		}

		if rect.left > rect.right {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!(
					"The left side of the client rectangle ({}) is greater \
						than the right side ({})",
					rect.left, rect.right)
			});
			return
		}
		if rect.top > rect.bottom {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!(
					"The top side of the client rectangle ({}) is greater \
						than bottom right side ({})",
					rect.top, rect.bottom)
			});
			return
		}

		let width = rect.right - rect.left;
		let height = rect.bottom - rect.top;

		/* Create a back buffer we'll be copying to the window at the end. */
		let target_dc = gdi::CreateCompatibleDC(paint.hdc);
		if target_dc.is_null() {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!("CreateCompatibleDC({:p}) failed: 0x{:08x}",
					paint.hdc,
					GetLastError())
			});
			return
		}
		let target_bitmap = gdi::CreateCompatibleBitmap(paint.hdc, width, height);
		if target_bitmap.is_null() {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!(
					"CreateCompatibleBitmap({:p}, {}, {}) failed: 0x{:08x}",
					paint.hdc,
					width,
					height,
					GetLastError())
			})
		}

		let replaced = gdi::SelectObject(target_dc, target_bitmap as _);
		if replaced.is_null() {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!(
					"SelectObject({:p}, {:p}) failed: 0x{:08x}",
					target_dc,
					target_bitmap,
					GetLastError())
			});
			return
		}

		/* Paint the screenshot over the window. */
		let screen = self.screen.borrow();
		if !screen.is_null() {
			let dc = gdi::CreateCompatibleDC(target_dc);
			if dc.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::paint({:p})", self),
					message: format!("CreateCompatibleDC({:p}) failed: 0x{:08x}",
						target_dc,
						GetLastError())
				});
				return
			}

			let replaced = gdi::SelectObject(dc, *screen as _);
			if replaced.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::paint({:p})", self),
					message: format!(
						"SelectObject({:p}, {:p}) failed: 0x{:08x}",
						dc,
						*screen,
						GetLastError())
				});
				return
			}

			let result = gdi::BitBlt(
				target_dc,
				0,
				0,
				width as _,
				height as _,
				dc,
				0,
				0,
				gdi::SRCCOPY);
			if result == 0 {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::paint({:p})", self),
					message: format!(
						"BitBlit({:p}, {}, {}, {}, {}, {:p}, {}, {}, 0x{:08x}) failed: 0x{:08x}",
						target_dc,
						0,
						0,
						width,
						height,
						dc,
						0,
						0,
						gdi::SRCCOPY,
						GetLastError())
				});
				return
			}

			let _ = gdi::SelectObject(dc, replaced);
			let _ = gdi::DeleteDC(dc);
		};

		/* Paint the shading and selected areas and blend them on to the client
		 * screen. */
		let _ = {
			let dc = gdi::CreateCompatibleDC(target_dc);
			if dc.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::paint({:p})", self),
					message: format!("CreateCompatibleDC({:p}) failed: 0x{:08x}",
						target_dc,
						GetLastError())
				});
				return
			}

			let mut info = std::mem::zeroed::<gdi::BITMAPINFO>();
			info.bmiHeader.biSize = std::mem::size_of::<gdi::BITMAPINFO>() as _;
			info.bmiHeader.biPlanes = 1;
			info.bmiHeader.biBitCount = 32;
			info.bmiHeader.biCompression = gdi::BI_RGB;
			info.bmiHeader.biWidth = width;
			info.bmiHeader.biHeight = -height;

			let mut buffer = std::ptr::null_mut();
			let bitmap = gdi::CreateDIBSection(
				paint.hdc,
				&info,
				gdi::DIB_RGB_COLORS,
				&mut buffer,
				std::ptr::null_mut(),
				0);
			if bitmap.is_null() || buffer.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::paint({:p})", self),
					message: format!(
						"CreateDIBSection({:p}, {:p}, 0x{:08x}, {:p}, {:p}, \
							{}) failed: 0x{:08x}",
						paint.hdc,
						&info,
						gdi::DIB_RGB_COLORS,
						&mut buffer,
						std::ptr::null_mut::<()>(),
						0,
						GetLastError())
				});
				return
			}

			/* Before we render to the bitmap buffer, make sure there are no
			 * pending GDI operations that could cause a race condition. */
			let _ = gdi::GdiFlush();

			/* Render to the bitmap buffer. */
			let buffer = {
				let offset = buffer.align_offset(std::mem::align_of::<u8>());
				if offset != 0 {
					self.fail(PickPhysicalAreaError::WindowLogicError {
						scope: format!("AreaSelection::paint({:p})", self),
						message: format!(
							"Bitmap buffer at {:p} is not byte-aligned",
							buffer)
					});
					return
				}

				let width = width.abs();
				let height = height.abs();

				let pixels = width as u64 * height as u64;
				let length = pixels.checked_mul(4)
					.and_then(|bytes| usize::try_from(bytes).ok());
				let length = match length {
					Some(length) => length,
					None => {
						self.fail(PickPhysicalAreaError::WindowLogicError {
							scope: format!("AreaSelection::paint({:p})", self),
							message: format!(
								"Number of bytes in the bitmap {} * {} * {} \
								does not fit in a usize",
								width, height, 4)
						});
						return
					}
				};

				std::slice::from_raw_parts_mut(buffer as *mut u8, length)
			};
			for (i, slice) in buffer.chunks_exact_mut(4).enumerate() {
				let x = (i % width.abs() as usize) as u32;
				let y = (i / width.abs() as usize) as u32;

				let selection = self.selection.borrow();
				let horizontal = x >= selection.x && x < selection.x + selection.width;
				let vertical = y >= selection.y && y < selection.y + selection.height;

				slice[0] = 0;
				slice[1] = 0;
				slice[2] = 0;
				slice[3] = if horizontal && vertical {
					0
				} else {
					127
				};
			}

			/* Blend the bitmap we just rendered on to the window. */
			let replaced = gdi::SelectObject(dc, bitmap as _);
			if replaced.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::paint({:p})", self),
					message: format!(
						"SelectObject({:p}, {:p}) failed: 0x{:08x}",
						dc,
						bitmap,
						GetLastError())
				});
				return
			}

			let mut alpha = std::mem::zeroed::<gdi::BLENDFUNCTION>();
			alpha.BlendOp = gdi::AC_SRC_OVER;
			alpha.BlendFlags = 0;
			alpha.SourceConstantAlpha = 255;
			alpha.AlphaFormat = gdi::AC_SRC_ALPHA;

			let result = gdi::AlphaBlend(
				target_dc,
				0,
				0,
				width as _,
				height as _,
				dc,
				0,
				0,
				width as _,
				height as _,
				alpha);
			if result == 0 {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::paint({:p})", self),
					message: format!(
						"AlphaBlend({:p}, {}, {}, {}, {}, {:p}, {}, {}, {}, \
							{}, {:p}) failed: 0x{:08x}",
						target_dc,
						0,
						0,
						width,
						height,
						dc,
						0,
						0,
						width,
						height,
						&alpha,
						GetLastError())
				});
				return
			}

			/* Clean up. */
			let _ = gdi::SelectObject(dc, replaced);
			let _ = gdi::DeleteObject(bitmap as _);
			let _ = gdi::DeleteDC(dc);
		};

		/* Copy from the back buffer to the front buffer. */
		let result = gdi::BitBlt(
			paint.hdc,
			0,
			0,
			width as _,
			height as _,
			target_dc,
			0,
			0,
			gdi::SRCCOPY);
		if result == 0 {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!(
					"BitBlit({:p}, {}, {}, {}, {}, {:p}, {}, {}, 0x{:08x}) failed: 0x{:08x}",
					paint.hdc,
					0,
					0,
					width,
					height,
					target_dc,
					0,
					0,
					gdi::SRCCOPY,
					GetLastError())
			});
			return
		}

		let _ = gdi::SelectObject(target_dc, replaced);
		let _ = gdi::DeleteObject(target_bitmap as _);
		let _ = gdi::DeleteDC(target_dc);

		/* Finish up painting. */
		if user::EndPaint(hwnd, &paint) == 0 {
			self.fail(PickPhysicalAreaError::WindowLogicError {
				scope: format!("AreaSelection::paint({:p})", self),
				message: format!("BeginPaint({:p}, {:p}) failed: 0x{:08x}",
					hwnd,
					&mut paint,
					GetLastError())
			});
			return
		}
	}

	/// Fail with the given error.
	fn fail(&self, what: PickPhysicalAreaError) {
		let _ = self.channel.send(Err(what));
		nwg::stop_thread_dispatch();
	}

	/// Initialize the screen.
	fn init(&self) {
		/* Take a screenshot of the currently visible desktop. */
		let screenshot = unsafe {
			use winapi::um::wingdi as gdi;
			use winapi::um::winuser as user;
			use winapi::um::errhandlingapi::GetLastError;

			let screen_dc = user::GetDC(user::HWND_DESKTOP);
			if screen_dc.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::init({:p})", self),
					message: format!("GetDC({:p}) failed: {:08x}",
						user::HWND_DESKTOP,
						GetLastError())
				});
				return
			}

			let compat_dc = gdi::CreateCompatibleDC(screen_dc);
			if compat_dc.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::init({:p})", self),
					message: format!("CreateCompatibleDC({:p}) failed: 0x{:08x}",
						screen_dc, GetLastError())
				});
				return
			}

			let width = gdi::GetDeviceCaps(screen_dc, gdi::HORZRES);
			let height = gdi::GetDeviceCaps(screen_dc, gdi::VERTRES);

			let bitmap = gdi::CreateCompatibleBitmap(screen_dc, width, height);
			if bitmap.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::init({:p})", self),
					message: format!("CreateCompatibleBitmap({:p}, {}, {}) failed: 0x{:08x}",
						compat_dc, width, height, GetLastError())
				});
				return
			}

			let replaced = gdi::SelectObject(compat_dc, bitmap as _);
			if replaced.is_null() {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::init({:p})", self),
					message: format!("SelectObject({:p}, {:p}) failed: 0x{:08x}",
						compat_dc, bitmap, GetLastError())
				});
				return
			}

			let w = user::GetSystemMetrics(user::SM_CXSCREEN);
			let h = user::GetSystemMetrics(user::SM_CYSCREEN);

			let result = gdi::BitBlt(
				compat_dc,
				0,
				0,
				w,
				h,
				screen_dc,
				0,
				0,
				gdi::SRCCOPY | gdi::CAPTUREBLT);
			if result == 0 {
				self.fail(PickPhysicalAreaError::WindowLogicError {
					scope: format!("AreaSelection::init({:p})", self),
					message: format!("BitBlt({:p}, {}, {}, {}, {}, {:?}, {}, {}, 0x{:08x}) failed: 0x{:08x}",
						compat_dc, 0, 0, w, h, screen_dc, 0, 0, gdi::SRCCOPY | gdi::CAPTUREBLT, GetLastError())
				});
				return
			}

			let _ = gdi::SelectObject(compat_dc, replaced);
			let _ = gdi::DeleteDC(compat_dc);
			let _ = user::ReleaseDC(user::HWND_DESKTOP, screen_dc);

			bitmap
		};

		*self.screen.borrow_mut() = screenshot;

		/* Make the main window full screen and show it. */
		unsafe {
			let hwnd = self.window.handle.hwnd().unwrap();

			use winapi::um::winuser as user;
			let _ = user::SetWindowLongA(
				hwnd,
				user::GWL_STYLE,
				0);
			let _ = user::SetWindowLongA(
				hwnd,
				user::GWL_EXSTYLE,
				0);

			let w = user::GetSystemMetrics(user::SM_CXSCREEN);
			let h = user::GetSystemMetrics(user::SM_CYSCREEN);

			let _ = user::SetWindowPos(
				hwnd,
				std::ptr::null_mut(),
				0,
				0,
				w,
				h,
				user::SWP_FRAMECHANGED);
		}
		self.window.set_visible(true);
	}

}
impl Drop for AreaSelection {
	fn drop(&mut self) {
		unsafe {
			let screen = self.screen.borrow();
			if !screen.is_null() {
				winapi::um::wingdi::DeleteObject(*screen as _);
			}
		}
	}
}

/// Writes a Windows bitmap to a buffer in memory.
unsafe fn bitmap_to_image(
	hdc: winapi::shared::windef::HDC,
	bitmap: winapi::shared::windef::HBITMAP)
	-> Result<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>, String> {

	use winapi::um::wingdi as gdi;
	use winapi::um::errhandlingapi::GetLastError;

	/* Pull the bytes in from Windows into an internal buffer. */
	let mut info = std::mem::zeroed::<gdi::BITMAPINFO>();
	info.bmiHeader.biSize = std::mem::size_of::<gdi::BITMAPINFO>() as _;

	let result = gdi::GetDIBits(
		hdc,
		bitmap,
		0,
		0,
		std::ptr::null_mut(),
		&mut info,
		gdi::DIB_RGB_COLORS);
	if result == 0 {
		return Err(format!(
			"GetDIBits({:p}, {:p}, {}, {}, {:p}, {:p}, {}) failed: 0x{:08x}",
			hdc,
			bitmap,
			0,
			0,
			std::ptr::null_mut::<()>(),
			&mut info,
			gdi::DIB_RGB_COLORS,
			GetLastError()))
	}

	info.bmiHeader.biHeight = -info.bmiHeader.biHeight;
	info.bmiHeader.biCompression = gdi::BI_RGB;
	info.bmiHeader.biBitCount = 24;

	let mut buffer = {
		let width = usize::try_from(info.bmiHeader.biWidth)
			.map_err(|what| {
				format!("The biWidth value ({}) does not fit in a usize: {}",
					info.bmiHeader.biWidth, what)
			})?;
		let height = usize::try_from(-info.bmiHeader.biHeight)
			.map_err(|what| {
				format!("The biHeight value ({}) does not fit in a usize: {}",
					-info.bmiHeader.biHeight, what)
			})?;
		let bytes_per_pixel = info.bmiHeader.biBitCount as usize / 8;
		let bytes = width.checked_mul(height)
			.and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
			.ok_or_else(|| {
				format!("The number of bytes for the bitmap ({} * {} * {}) does \
					not fit in a usize",
					width, height, bytes_per_pixel)
			})?;

		vec![0u8; bytes]
	};

	let (width, height) = {
		let width = u32::try_from(info.bmiHeader.biWidth)
			.map_err(|what| {
				format!("The biWidth value ({}) does not fit in a u32: {}",
					info.bmiHeader.biWidth, what)
			})?;
		let height = u32::try_from(-info.bmiHeader.biHeight)
			.map_err(|what| {
				format!("The biHeight value ({}) does not fit in a u32: {}",
					-info.bmiHeader.biHeight, what)
			})?;

		(width, height)
	};

	let result = gdi::GetDIBits(
		hdc,
		bitmap,
		0,
		info.bmiHeader.biHeight as _,
		buffer.as_mut_ptr() as _,
		&mut info,
		gdi::DIB_RGB_COLORS);
	if result == 0 {
		return Err(format!(
			"GetDIBits({:p}, {:p}, {}, {}, {:p}, {:p}, {}) failed: 0x{:08x}",
			hdc,
			bitmap,
			0,
			info.bmiHeader.biHeight as u32,
			buffer.as_mut_ptr(),
			&mut info,
			gdi::DIB_RGB_COLORS,
			GetLastError()))
	}

	/* Convert to a bitmap in memory. */
	let image = match info.bmiHeader.biBitCount {
		24 => {
			image::ImageBuffer::from_fn(
				width,
				height,
				|x, y| {
					let base = (y * width + x) * 3;
					let base = base as usize;

					let r = buffer[base + 2];
					let g = buffer[base + 1];
					let b = buffer[base + 0];

					image::Rgb([r, g, b])
				})
		},
		32 => {
			image::ImageBuffer::from_fn(
				width,
				height,
				|x, y| {
					let base = (y * width + x) * 4;
					let base = base as usize;

					let r = buffer[base + 3];
					let g = buffer[base + 2];
					let b = buffer[base + 1];

					image::Rgb([r, g, b])
				})
		},
		_ => unreachable!()
	};

	Ok(image)
}

/// Enumeration of reasons why prompting the user to pick a physical region on
/// the screen might have failed.
#[derive(Debug, thiserror::Error)]
pub enum PickPhysicalAreaError {
	/// The window could not be created.
	#[error("could not create the prompt window: {0}")]
	WindowCreationError(nwg::NwgError),
	/// The window logic has failed.
	#[error("window logic error: {scope}: {message}")]
	WindowLogicError {
		/// The scope in which this error originated.
		scope: String,
		/// The message being carried by this value.
		message: String,
	},
	/// The user has requested for the prompt to close with no selection.
	#[error("the operation was cancelled")]
	Cancelled,
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
		text: "Screen Controls",
		position: (10, 10),
		size: (100, 20)
	)]
	display_label: nwg::Label,

	/// Button for clearing the signature.
	#[nwg_control(
		text: "Clear",
		position: (10, 140)
	)]
	#[nwg_events(
		OnButtonClick: [Self::on_clear_pressed]
	)]
	display_clear_btn: nwg::Button,

	/// Button for painting the signature.
	#[nwg_control(
		text: "Paint",
		position: (110, 140)
	)]
	#[nwg_events(
		OnButtonClick: [Self::on_paint_pressed]
	)]
	display_playback_btn: nwg::Button,

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

	/// Whether the management window is currently locked.
	locked: RefCell<bool>,

	/// The device we're connected to.
	device: Tablet,
	/// The queue though which we receive device updates.
	queue: RefCell<Queue>,

	/// The path accumulated from the events generated by the tablet.
	path: RefCell<EventPath>,
	/// The canvas accumulated from the events generated by the tablet.
	canvas: RefCell<EventCanvas>,

	/// The notification channel through which we know the painting is done.
	#[nwg_control()]
	#[nwg_events(
		OnNotice: [Self::on_paint_done]
	)]
	display_paint_done: nwg::Notice,

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
			display_playback_btn: Default::default(),
			update: Default::default(),
			locked: RefCell::new(false),
			device,
			queue: RefCell::new(queue),
			path: Default::default(),
			canvas: RefCell::new(EventCanvas::new(caps.width(), caps.height())),
			display_paint_done: Default::default(),
			fails
		}
	}

	/// Locks all of the controls in this window.
	fn lock(&self) {
		self.device.inking(false);
		self.display_clear_btn.set_enabled(false);
		self.display_playback_btn.set_enabled(false);
		*self.locked.borrow_mut() = true;
	}

	/// Unlocks all of the controls in this window.
	fn unlock(&self) {
		self.device.inking(true);
		self.display_clear_btn.set_enabled(true);
		self.display_playback_btn.set_enabled(true);
		*self.locked.borrow_mut() = false;
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

	/// Called when an intent for painting the device data has been fired.
	fn on_paint_pressed(&self) {
		self.lock();
		Playback {
			path: self.path.borrow().clone(),
			target: PhysicalArea {
				x: 10,
				y: 50,
				width: 1316,
				height: 668
			},
			delta: Duration::from_secs(4),
			steps: unsafe { NonZeroU32::new_unchecked(10000) }
		}.play_and_notify(self.display_paint_done.sender());
	}

	/// Called when the painting of the signature has been completed.
	fn on_paint_done(&self) {
		self.unlock();
	}

	/// Pulls in events from the device and repaints the screen.
	fn update(&self, force_repaint: bool) {
		/* Process the input events. */
		let mut queue = self.queue.borrow_mut();
		let mut canvas = self.canvas.borrow_mut();
		let mut path = self.path.borrow_mut();

		let mut dirty = false;
		let locked = self.locked.borrow();
		loop {
			match queue.try_recv() {
				Ok(event) => {
					if !*locked {
						canvas.process(event);
						path.process(event);

						dirty = true;
					}
				},
				Err(stu::TryRecvError::Empty) =>
					/* Done processing events for now. */
					break,
				Err(stu::TryRecvError::Failed(what)) => {
					/* The polling process has failed. */
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
