use std::collections::btree_map::BTreeMap;
use std::time::Instant;
use image::Luma;
use stu::Event;

/// Trait for structures that can produce a [`Trace`].
///
/// [`Trace`]: Trace
pub trait IntoTrace {
	/// Type of trace that will be generated.
	type Trace<'a>: Trace
		where Self: 'a;

	/// Generate a tracing along the curve in this structure.
	fn trace<'a>(&'a self) -> Self::Trace<'a>;
}

/// Trait for parametric curves.
pub trait Trace {
	/// Get the point along this path at the given time.
	///
	/// The minimum value for the time is `0.0`, at the start of the path, and
	/// the maximum value is `1.0`, at the end of the path. Values greater than
	/// the maximum or smaller than the minimum will be clamped.
	fn get<E>(&self, t: f64, buffer: &mut E) -> usize
		where E: Extend<Point>;
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
}
impl IntoTrace for EventPath {
	type Trace<'a> = EventTrace<'a>;
	fn trace(&self) -> EventTrace {
		EventTrace {
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
pub struct EventTrace<'a> {
	/// A list of events, sorted by the time they happened. This is a list
	/// rather than other kinds of sorted containers because it allows for us to
	/// uniformly access its elements, which avoids the clustering of events.
	events: Box<[&'a Event]>
}
impl Trace for EventTrace<'_> {
	fn get<E>(&self, t: f64, buffer: &mut E) -> usize
		where E: Extend<Point> {
		if self.events.len() == 0 { return 0 }
		if self.events.len() == 1 {
			buffer.extend(Some(Point {
				x: self.events[0].x(),
				y: self.events[0].y(),
				touch: self.events[0].touching()
			}));
			return 1
		}

		let t = t.clamp(0.0, 1.0);
		let t = t * (self.events.len() - 1) as f64;

		let f = t.fract();

		let i = t.floor();
		let j = t.ceil();

		let a = self.events[i as usize];
		let b = self.events[j as usize];

		buffer.extend(Some(Point {
			x: lerp(f, a.x(), b.x()),
			y: lerp(f, a.y(), b.y()),
			touch: a.touching()
		}));
		1
	}
}

fn lerp(s: f64, a: f64, b: f64) -> f64 {
	(1.0 - s) * a + s * b
}

/// Structure that represents a path generated from a bitmap rather than from
/// a list of sign pad events.
#[derive(Debug, Clone)]
pub struct BitmapPath {
	image: image::GrayImage
}
impl BitmapPath {
	/// Creates a new bitmap path from the given image.
	pub fn new(mut image: image::GrayImage) -> Self {
		/* Force the image into a high-contrast format. */
		for i in 0..image.height() {
			for j in 0..image.width() {
				let pixel = image.get_pixel_mut(j, i);
				if pixel.0[0] < 20 {
					*pixel = Luma([0])
				} else {
					*pixel = Luma([255])
				}
			}
		}

		Self { image }
	}

	/// Width of the canvas.
	pub fn width(&self) -> u32 { self.image.width() }

	/// Height of the canvas.
	pub fn height(&self) -> u32 { self.image.height() }

	/// Copies the image data in this canvas into a memory blob encoded as a
	/// bitmap.
	///
	/// The format the bitmap will be in is full color 24-bpp RGB, in which
	/// pixels marked as active will be painted black and pixels that are not
	/// will be painted white.
	pub fn to_bitmap(&self) -> Box<[u8]> {
		let image = image::ImageBuffer::from_fn(
			self.image.width(),
			self.image.height(),
			|x, y| {
				let pixel = self.image.get_pixel(x, y).0[0];
				image::Rgb([pixel, pixel, pixel])
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
}
impl IntoTrace for BitmapPath {
	type Trace<'a> = BitmapTrace;
	fn trace<'a>(&'a self) -> Self::Trace<'a> {
		let mut points = Vec::new();
		for x in 0..self.image.width() {
			for y in 0..self.image.height() {
				if self.image.get_pixel(x, y).0[0] == 0 {
					points.push((
						f64::from(x) / f64::from(self.image.width()),
						f64::from(y) / f64::from(self.image.height()),
					))
				}
			}
		}

		BitmapTrace {
			points: points.into_boxed_slice(),
		}
	}
}

/// A parametric curve derived from a bitmap path.
pub struct BitmapTrace {
	points: Box<[(f64, f64)]>,
}
impl Trace for BitmapTrace {
	fn get<E>(&self, t: f64, buffer: &mut E) -> usize
		where E: Extend<Point> {

		let index = t * self.points.len() as f64;
		let index = index.floor() as usize;
		let (x, y) = if index < self.points.len() {
			self.points[index]
		} else {
			return 0
		};

		buffer.extend([
			Point {
				x,
				y,
				touch: true
			},
			Point {
				x,
				y,
				touch: false
			},
		]);
		2
	}
}
