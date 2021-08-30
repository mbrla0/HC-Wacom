use crate::path::{EventPath, Point};
use std::time::{Duration, Instant};
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;

/// A global state lock for controlling access to the mouse.
static MOUSE_LOCK: AtomicBool = AtomicBool::new(false);

/// A structure controlling the playback of an event path over a region of the
/// screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Playback {
	/// The path this structure is going to be playing back.
	pub path: EventPath,
	/// The rectangular region that maps the output to the physical screen.
	pub target: PhysicalArea,
	/// The amount of time that the path should take to get written down.
	pub delta: Duration,
	/// The number of steps that will be used to play the path back.
	pub steps: NonZeroU32,
}
impl Playback {
	/// Maps a point in normalized space into a point in screen space.
	fn map(&self, point: Point) -> (i32, i32) {
		let Point { x, y } = point;

		let w = f64::from(nwg::Monitor::virtual_width());
		let h = f64::from(nwg::Monitor::virtual_height());

		let a = self.target;

		let x = x * a.width.saturating_sub(1) as f64 + a.x as f64;
		let y = y * a.height.saturating_sub(1) as f64 + a.y as f64;

		let n = f64::from(256 * 256 - 1);
		let x = (x / w * n) as i32;
		let y = (y / h * n) as i32;

		(x, y)
	}

	/// Perform the mouse movements specified by this structure on to the screen.
	pub fn play_and_notify(self, sender: nwg::NoticeSender) {
		if MOUSE_LOCK.fetch_or(true, std::sync::atomic::Ordering::SeqCst) {
			/* Calling this function twice is a bug in this program. */
			panic!("Called Playback::play_and_notify() more than once");
		}

		let mut pressed = false;
		std::thread::spawn(move || {
			let mut x = 0.0;

			let dt = self.delta.div_f64(f64::from(self.steps.get()));
			let dx = 1.0 / f64::from(self.steps.get());

			for _ in 0..self.steps.get() {
				let timer = Instant::now();

				/* Evaluate the curve at the current position. */
				let point = match self.path.get(x) {
					Some(point) => point,
					None => break
				};
				let point = self.map(point);

				/* Build the input structure and send it. */
				unsafe {
					let mut input: winapi::um::winuser::INPUT =
						std::mem::zeroed();

					input.type_ = winapi::um::winuser::INPUT_MOUSE;

					input.u.mi_mut().dx = point.0;
					input.u.mi_mut().dy = point.1;
					input.u.mi_mut().mouseData = 0;

					input.u.mi_mut().time = 0;

					input.u.mi_mut().dwExtraInfo = 0;
					input.u.mi_mut().dwFlags =
						  winapi::um::winuser::MOUSEEVENTF_ABSOLUTE
						| winapi::um::winuser::MOUSEEVENTF_VIRTUALDESK
						| winapi::um::winuser::MOUSEEVENTF_MOVE
						| if !pressed {
							  pressed = true;
							  winapi::um::winuser::MOUSEEVENTF_LEFTDOWN
						  } else { 0 };

					let _ = winapi::um::winuser::SendInput(
						1,
						&mut input,
						std::mem::size_of::<winapi::um::winuser::INPUT>() as _,);
				}

				x += dx;

				let elapsed = timer.elapsed();
				if elapsed < dt {
					let diff = dt - elapsed;
					std::thread::sleep(diff);
				}
			}

			/* Tell the mouse to release the left down key. */
			unsafe {
				let mut input: winapi::um::winuser::INPUT =
					std::mem::zeroed();

				input.type_ = winapi::um::winuser::INPUT_MOUSE;

				input.u.mi_mut().dx = 0;
				input.u.mi_mut().dy = 0;
				input.u.mi_mut().mouseData = 0;

				input.u.mi_mut().time = 0;

				input.u.mi_mut().dwExtraInfo = 0;
				input.u.mi_mut().dwFlags = winapi::um::winuser::MOUSEEVENTF_LEFTUP;

				let _ = winapi::um::winuser::SendInput(
					1,
					&mut input,
					std::mem::size_of::<winapi::um::winuser::INPUT>() as _,);
			}

			/* Release our lock on the mouse. */
			MOUSE_LOCK.store(false, std::sync::atomic::Ordering::SeqCst);
			sender.notice();
		});
	}
}

/// An area in physical screen coordinate space encoded as a rectangle.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct PhysicalArea {
	/// The position of the top left corner along the horizontal axis.
	pub x: u32,
	/// The position of the top left corner along the vertical axis.
	pub y: u32,
	/// The width of the rectangular region.
	pub width: u32,
	/// The height of the rectangular region.
	pub height: u32,
}
