use std::collections::BTreeMap;
use std::time::{Instant, Duration};
use stu::Event;

/// A structure for generating pictures from events.
pub struct EventCanvas {
	/// A monochrome pixel data buffer.
	buffer: Box<[u8]>,
	/// The width of the canvas, in pixels.
	width: u32,
	/// The height of the canvas, in pixels.
	height: u32,
}
impl EventCanvas {
	/// Creates a new, blank canvas on with the given dimensions.
	pub fn new(width: u32, height: u32) -> Self {
		if width == 0 {
			panic!("Tried to create a canvas with no width.")
		}
		if height == 0 {
			panic!("Tried to create a canvas with no height.")
		}

		let bits = u64::from(width) * u64::from(height);

		let bytes = bits / 8 + if bits % 8 == 0 { 0 } else { 1 };
		let bytes = std::convert::TryFrom::try_from(bytes)
			.expect("Canvas size does not fit in a usize");

		let buffer = vec![0u8; bytes].into_boxed_slice();
		Self { buffer, width, height }
	}

	/// Clears this canvas back into an unset state.
	pub fn clear(&mut self) {
		for byte in &mut self.buffer[..] { *byte = 0; }
	}

	/// Process the given event altering the canvas if needed.
	pub fn process(&mut self, event: Event) {
		if event.touching() {
			let x = f64::from(self.width - 1) * event.x();
			let y = f64::from(self.height - 1) * event.y();

			let x = x.round() as u32;
			let y = y.round() as u32;

			self.set(x, y, true);
		}
	}

	/// Gets the index of the byte and offset of the bit corresponding to the
	/// pixel at the given coordinates.
	fn index_offset(&self, x: u32, y: u32) -> Option<(usize, u8)> {
		if x >= self.width || y >= self.height {
			return None
		}

		let pixel = u128::from(y) * u128::from(self.width) + u128::from(x);
		let index = pixel / 8;
		let offset = pixel % 8;

		let index = std::convert::TryFrom::try_from(index).unwrap();
		let offset = offset as u8;

		Some((index, offset))
	}

	/// Gets whether the pixel at the given position is set.
	pub fn get(&self, x: u32, y: u32) -> Option<bool> {
		let (index, offset) = self.index_offset(x, y)?;
		Some(self.buffer[index] & (1u8 << offset) != 0)
	}

	/// Defines whether the pixel at the given position is set.
	pub fn set(&mut self, x: u32, y: u32, val: bool) {
		let (index, offset) = self.index_offset(x, y).unwrap();

		if val {
			self.buffer[index] |= 1u8 << offset;
		} else {
			self.buffer[index] &= !(1u8 << offset);
		}
	}
}

/// A structure for generating paths from events.
pub struct EventPath {
	/// Ordered list of events in this path, sorted by the time in which they
	/// happened and were reported by the underlying API.
	events: BTreeMap<Instant, Event>,
}
impl EventPath {
	/// Creates a new, empty path.
	pub fn new() -> Self {
		Self {
			events: Default::default()
		}
	}

	/// Inserts a new event into this path.
	///
	/// If this path had already registered an event that happened at the same
	/// time as the given event, this event will replace it in the path and
	/// this function will return the event that was replaced.
	pub fn process(&mut self, event: Event) -> Option<Event> {
		self.events.insert(event.time(), event)
	}

	/// Clears all of the events in this path.
	pub fn clear(&mut self) {
		self.events.clear()
	}

	/// Get the point along this path at the given time.
	///
	/// The minimum value for the time is `0.0`, at the start of the path, and
	/// the maximum value is `1.0`, at the end of the path. Values greater than
	/// the maximum or smaller than the minimum will be clamped.
	pub fn get(&self, t: f64) -> Option<Point> {
		/* Transform the normalized point `t` along the curve into a point in
		 * time we can use to find the actual neighbourhood of events we're
		 * looking for. */
		let t = t.clamp(0.0, 1.0);

		let beg = self.events.iter().next();
		let end = self.events.iter().rev().next();

		let (beg, end) = match (beg, end) {
			(Some((_, beg)), Some((_, end))) => (beg, end),
			(Some((_, beg)), None) => return Some(Point {
				x: beg.x(),
				y: beg.y()
			}),
			(None, Some((_, end))) => return Some(Point {
				x: end.x(),
				y: end.y()
			}),
			(None, None) => return None
		};

		let delta = end.time().duration_since(beg.time());
		let delta = delta.as_secs_f64();
		let delta = Duration::from_secs_f64(delta * t);

		let target = beg.time() + delta;

		/* */
		let neighbourhood = {
			let mut a = self.events.range(..target);
			let mut b = self.events.range(target..);
			(
				a.next().filter(|(_, event)| event.touching()),
				b.next().filter(|(_, event)| event.touching()),
				b.next().filter(|(_, event)| event.touching())
			)
		};

		Some(match neighbourhood {
			(Some((_, a)), Some((_, b)), Some((_, c))) => {
				/* TODO: Use cubic hermite spline interpolation. */

				let (x, y) = if t < 0.5 {(
					(t * 2.0).lerp(a.x(), b.x()),
					(t * 2.0).lerp(a.y(), b.y()),
				)} else {(
					((t - 0.5) * 2.0).lerp(b.x(), c.x()),
					((t - 0.5) * 2.0).lerp(b.y(), c.y()),
				)};

				Point { x, y }
			},
			(Some((_, a)), Some((_, b)), None)
			| (None, Some((_, a)), Some((_, b))) => {
				/* Linearly interpolate between the two points. */

				let x = t.lerp(a.x(), b.x());
				let y = t.lerp(a.y(), b.y());

				Point { x, y }
			},
			(Some((_, a)), None, None)
			| (None, Some((_, a)), None)
			| (None, None, Some((_, a)))
			| (Some(_), None, Some((_, a))) => {
				/* Use the only point we have. */
				Point {
					x: a.x(),
					y: a.y()
				}

			},
			_ => unreachable!()
		})
	}
}
impl Default for EventPath {
	fn default() -> Self {
		Self::new()
	}
}

/// A point on the screen along a curve.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Point {
	/// The position of the cursor in the horizontal axis.
	pub x: f64,
	/// The position of the cursor in the vertical axis.
	pub y: f64,
}
