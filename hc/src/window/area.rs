use crate::robot::ScreenArea;
use std::cell::RefCell;
use std::convert::TryFrom;

/// Display a window control that lets the user select a rectangular region on
/// the screen. This is intended for use with the signature painting
/// functionality.
pub fn pick_physical_area(
	parameters: AreaSelectionParameters)
	-> Result<ScreenArea, PickPhysicalAreaError> {

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
	selection: RefCell<ScreenArea>,

	/// The channel through which we report our result.
	channel: std::sync::mpsc::Sender<Result<ScreenArea, PickPhysicalAreaError>>,
}
impl AreaSelection {
	fn new(
		params: AreaSelectionParameters,
		channel:std::sync::mpsc::Sender<Result<ScreenArea, PickPhysicalAreaError>>)
		-> Self {

		Self {
			window: Default::default(),
			screen: RefCell::new(std::ptr::null_mut()),
			params,
			mouse_pressed: RefCell::new(false),
			mouse_anchor: RefCell::new((0, 0)),
			selection: RefCell::new(ScreenArea {
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
					*self.selection.borrow_mut() = ScreenArea {
						x: anchor.0.max(0),
						y: anchor.1.max(0),
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
		let x = x.max(0);
		let y = y.max(0);

		let (ax, ay) = *self.mouse_anchor.borrow();
		let ax = ax.max(0);
		let ay = ay.max(0);

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
		*self.selection.borrow_mut() = ScreenArea {
			x,
			y,
			width: width as u32,
			height: height as u32
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
				let x = (i % width.abs() as usize) as i32;
				let y = (i / width.abs() as usize) as i32;

				let selection = self.selection.borrow();
				let horizontal = x >= selection.x && x < selection.x + selection.width as i32;
				let vertical = y >= selection.y && y < selection.y + selection.height as i32;

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
