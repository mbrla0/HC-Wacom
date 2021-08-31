use std::collections::BTreeMap;
use std::time::Instant;
use stu::Event;

/// A structure for generating pictures from events.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EventCanvas {
	/// A monochrome pixel data buffer.
	buffer: Box<[u8]>,
	/// The width of the canvas, in pixels.
	width: u32,
	/// The height of the canvas, in pixels.
	height: u32,
	/// The last point the pen stroke.
	last: Option<(u32, u32)>,
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
		Self { buffer, width, height, last: None }
	}

	/// The width of this canvas, in pixels.
	pub fn width(&self) -> u32 {
		self.width
	}

	/// The height of this canvas, in pixels.
	pub fn height(&self) -> u32 {
		self.height
	}

	/// Copies the image data in this canvas into a memory blob encoded as a
	/// bitmap.
	///
	/// The format the bitmap will be in is full color 24-bpp RGB, in which
	/// pixels marked as active will be painted black and pixels that are not
	/// will be painted white.
	pub fn to_bitmap(&self) -> Box<[u8]> {
		let image = image::ImageBuffer::from_fn(
			self.width,
			self.height,
			|x, y| {
				let pixel = self.get(x, y).unwrap();
				if pixel {
					image::Rgb([0u8, 0u8, 0u8])
				} else {
					image::Rgb([255u8, 255u8, 255u8])
				}
			});

		let mut buffer = Vec::new();
		let mut encoder = image::codecs::bmp::BmpEncoder::new(&mut buffer);

		encoder.encode(
			image.as_raw(),
			image.width(),
			image.height(),
			image::ColorType::Rgb8)
			.unwrap();

		buffer.into_boxed_slice()
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
			if let Some((last_x, last_y)) = self.last {
				let mut ix = f64::from(last_x);
				let mut iy = f64::from(last_y);

				let dx = i64::from(x) - i64::from(last_x);
				let dy = i64::from(y) - i64::from(last_y);

				if dx != 0 || dy != 0 {
					/* Trace a line to this point from the last point. */
					if dx.abs() > dy.abs() {
						/* Trace along X. */
						let slope = dy as f64 / dx as f64;
						for ax in 0..dx.abs() {
							let x = i64::from(last_x) + ax * dx.signum();
							let y = iy.round();

							self.set(x as u32, y as u32, true);
							iy += slope * dx.signum() as f64;
						}
					} else {
						/* Trace along Y. */
						let slope = dx as f64 / dy as f64;
						for ay in 0..dy.abs() {
							let x = ix.round();
							let y = i64::from(last_y) + ay * dy.signum();

							self.set(x as u32, y as u32, true);
							ix += slope * dy.signum() as f64;
						}
					}
				}
			}

			self.last = Some((x, y));
		} else {
			self.last = None
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
#[derive(Debug, Clone, PartialEq)]
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

	/// Generate a tracing along the curve in this structure.
	pub fn trace(&self) -> Trace {
		Trace {
			events: self.events
				.values()
				.collect::<Vec<_>>()
				.into_boxed_slice()
		}
	}
}
impl Default for EventPath {
	fn default() -> Self {
		Self::new()
	}
}

/// A tracing along a path generated by [`EventPath`].
///
/// [`EventPath`]: EventPath
#[derive(Debug, Clone, PartialEq)]
pub struct Trace<'a> {
	/// A list of events, sorted by the time they happened. This is a list
	/// rather than other kinds of sorted containers because it allows for us to
	/// uniformly access its elements, which avoids the clustering of events.
	events: Box<[&'a Event]>
}
impl Trace<'_> {
	/// Get the point along this path at the given time.
	///
	/// The minimum value for the time is `0.0`, at the start of the path, and
	/// the maximum value is `1.0`, at the end of the path. Values greater than
	/// the maximum or smaller than the minimum will be clamped.
	pub fn get(&self, t: f64) -> Option<Point> {
		if self.events.len() == 0 { return None }
		if self.events.len() == 1 {
			return Some(Point {
				x: self.events[0].x(),
				y: self.events[0].y(),
				touch: self.events[0].touching()
			})
		}

		let t = t.clamp(0.0, 1.0);
		let t = t * (self.events.len() - 1) as f64;

		let f = t.fract();

		let i = t.floor();
		let j = t.ceil();

		let a = self.events[i as usize];
		let b = self.events[j as usize];

		Some(Point {
			x: f.lerp(a.x(), b.x()),
			y: f.lerp(a.y(), b.y()),
			touch: a.touching()
		})
	}
}

/// A point on the screen along a curve.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Point {
	/// The position of the cursor in the horizontal axis.
	pub x: f64,
	/// The position of the cursor in the vertical axis.
	pub y: f64,
	/// Whether the pen is touching the screen at this point.
	pub touch: bool,
}
