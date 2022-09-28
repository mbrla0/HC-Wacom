use std::collections::VecDeque;
use crate::path::{IntoTrace, Point, Trace};
use std::time::{Duration, Instant};
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;

/// A global state lock for controlling access to the mouse.
static MOUSE_LOCK: AtomicBool = AtomicBool::new(false);

/// A structure controlling the playback of an event path over a region of the
/// screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Playback<T> {
	/// The path this structure is going to be playing back.
	pub path: T,
	/// The rectangular region that maps the output to the physical screen.
	pub target: ScreenArea,
	/// The amount of time that the path should take to get written down.
	pub delta: Duration,
	/// The number of steps that will be used to play the path back.
	pub steps: NonZeroU32,
}
impl<T> Playback<T>
	where T: IntoTrace {

	/// Maps a point in normalized space into a point in screen space.
	fn map(&self, point: Point) -> (i32, i32) {
		let Point { x, y, .. } = point;

		/* Using device coordinates forces all points to map to the primary
		 * monitor, regardless of what monitor they're actually in. This is
		 * wrong, but works so long as only areas entirely inside the primary
		 * monitor are given to the Playback structure.
		 *
		 * TODO: Convert to virtual screen coordinates here.
		 */
		let w = f64::from(nwg::Monitor::width());
		let h = f64::from(nwg::Monitor::height());

		let a = self.target;

		let x = x * a.width.saturating_sub(1) as f64 + a.x as f64;
		let y = y * a.height.saturating_sub(1) as f64 + a.y as f64;

		let n = f64::from(256 * 256 - 1);
		let x = (x / w * n) as i32;
		let y = (y / h * n) as i32;

		(x, y)
	}

	/// Perform the mouse movements specified by this structure on to the screen.
	pub fn play_and_notify(self, sender: nwg::NoticeSender)
		where T: Send + 'static {

		if MOUSE_LOCK.fetch_or(true, std::sync::atomic::Ordering::SeqCst) {
			/* Calling this function twice is a bug in this program. */
			panic!("Called Playback::play_and_notify() more than once");
		}

		std::thread::spawn(move || {
			let mut x = 0.0;
			let mut pressed = false;
			let trace = self.path.trace();

			let dt = self.delta.div_f64(f64::from(self.steps.get()));
			let dx = 1.0 / f64::from(self.steps.get());

			let mut buffer = VecDeque::new();

			for _ in 0..self.steps.get() {
				/* Evaluate the curve at the current position. */
				let points = trace.get(x, &mut buffer);
				if points == 0 { break }

				for point in buffer.drain(..) {
					let timer1 = Instant::now();

					let (px, py) = self.map(point);

					/* Build the input structure and send it. */
					unsafe {
						let mut input: winapi::um::winuser::INPUT =
							std::mem::zeroed();

						input.type_ = winapi::um::winuser::INPUT_MOUSE;

						input.u.mi_mut().dx = px;
						input.u.mi_mut().dy = py;
						input.u.mi_mut().mouseData = 0;

						input.u.mi_mut().time = 0;

						input.u.mi_mut().dwExtraInfo = 0;
						input.u.mi_mut().dwFlags =
							winapi::um::winuser::MOUSEEVENTF_ABSOLUTE
								| winapi::um::winuser::MOUSEEVENTF_MOVE
								| if !pressed && point.touch {
								pressed = true;
								winapi::um::winuser::MOUSEEVENTF_LEFTDOWN
							} else if pressed && !point.touch {
								pressed = false;
								winapi::um::winuser::MOUSEEVENTF_LEFTUP
							} else { 0 };

						let _ = winapi::um::winuser::SendInput(
							1,
							&mut input,
							std::mem::size_of::<winapi::um::winuser::INPUT>() as _, );
					}

					x += dx;

					/* Spinning is way more accurate than using thread::sleep,
					 * and for small amounts time like we're dealing with here
					 * it would be too inaccurate. */
					while timer1.elapsed() < dt {}
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
///
/// The coordinates in this structure are in screen space, rather than virtual
/// space, so it is expected that positions may be negative when the rectangle
/// does not point to the primary screen.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ScreenArea {
	/// The position of the top left corner along the horizontal axis.
	pub x: i32,
	/// The position of the top left corner along the vertical axis.
	pub y: i32,
	/// The width of the rectangular region.
	pub width: u32,
	/// The height of the rectangular region.
	pub height: u32,
}
