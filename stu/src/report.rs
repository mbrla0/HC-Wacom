use crate::{Tablet, Error, Capability};
use crate::error::{InternalError, ClientError};
use crate::handle::Handle;
use std::collections::VecDeque;
use std::time::Instant;

/// An input event coming from a tablet device.
///
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Event {
	/// The point in time in which this event was generated.
	///
	/// For practical reasons, this field contains the timestamp for when the
	/// event was processed by the queue, rather than exactly when it was
	/// generated. This is due to the fact that reports have no timing data
	/// attached to them.
	timestamp: Instant,
	/// The position and pressure of the pen on the screen as an X,Y,Z
	/// coordinate tuple.
	///
	/// # Coordinate Representation
	/// The system in which these coordinates are represented is normalized so
	/// that a value of `0.0` is the minimum allowed value and `1.0` is the
	/// maximum allowed value for each coordinate.
	position: (f64, f64, f64),
	/// Whether the pen is touching the surface.
	touching: bool,
	/// Whether the pen is in proximity of the surface.
	close: bool,
}
impl Event {
	/// The point in time in which this event was generated.
	pub fn time(&self) -> Instant {
		self.timestamp
	}

	/// The position of the pen in the horizontal axis when this event was
	/// generated.
	///
	/// # Coordinate Representation
	/// The system in which these coordinates are represented is normalized so
	/// that a value of `0.0` is the minimum allowed value and `1.0` is the
	/// maximum allowed value for each coordinate.
	pub fn x(&self) -> f64 {
		self.position.0
	}

	/// The position of the pen in the vertical axis when this event was
	/// generated.
	///
	/// # Coordinate Representation
	/// The system in which these coordinates are represented is normalized so
	/// that a value of `0.0` is the minimum allowed value and `1.0` is the
	/// maximum allowed value for each coordinate.
	pub fn y(&self) -> f64 {
		self.position.1
	}

	/// The pressure being applied to the screen with the pen when this event
	/// was generated.
	///
	/// # Coordinate Representation
	/// The system in which these coordinates are represented is normalized so
	/// that a value of `0.0` is the minimum allowed value and `1.0` is the
	/// maximum allowed value for each coordinate.
	pub fn pressure(&self) -> f64 {
		self.position.2
	}

	/// Whether the pen was touching the screen when this event was generated.
	pub fn touching(&self) -> bool {
		self.touching
	}

	/// Whether the pen was close enough to be considered as having been
	/// hovering over the screen when this event was generated.
	pub fn hovering(&self) -> bool {
		self.close
	}
}

/// A report queue connected to a tablet device.
pub struct Queue<'a> {
	/// The device this queue is polling update data off of.
	_device: &'a Tablet,
	/// The queue backing this structure.
	queue: RawQueue,
	/// The report handler used by this instance of the queue.
	handler: ReportHandler,
}
impl<'a> Queue<'a> {
	/// Creates a new queue for this tablet device.
	pub(crate) fn new(device: &'a Tablet, caps: Capability) -> Result<Self, Error> {
		let queue = RawQueue(unsafe {
			let mut queue = std::mem::zeroed();

			InternalError::from_wacom_stu({
				stu_sys::WacomGSS_Interface_interfaceQueue(
					device.raw.interface,
					&mut queue)
			}).map_err(InternalError::unwrap_to_general)?;

			queue
		});
		let handler = ReportHandler {
			resolution:	(
				caps.input_grid_width(),
				caps.input_grid_height(),
				caps.input_grid_pressure()),
			queue: Default::default()
		};

		Ok(Self { _device: device, queue, handler })
	}

	/// Handles a report using the internal report handler in this queue.
	fn handle(&mut self, report: Handle<[u8]>) -> Result<usize, Error> {
		assert_eq!(
			self.handler.queue.len(),
			0,
			"Event queue must have been empty at the start of the handle \
			function, but instead, it is not. ReportHandler queues must get \
			emptied before every call to the Queue::handle() function");

		let mut pointer = std::ptr::null();
		let mut returned = 0;

		InternalError::from_wacom_stu(unsafe {
			stu_sys::WacomGSS_ReportHandler_handleReport(
				std::mem::size_of::<stu_sys::WacomGSS_ReportHandlerFunctionTable>() as _,
				&REPORT_HANDLER_FUNCTIONS,
				&mut self.handler as *mut ReportHandler as *mut _,
				report.as_ptr() as *const u8,
				report.len() as _,
				&mut pointer,
				&mut returned)
		}).map_err(InternalError::unwrap_to_general)?;

		let end = report.as_ptr_range().end;
		if returned == 0 || pointer != end {
			/* Having the handleReport() function indicate a failed return or
			 * a pointer that doesn't align with the expected end of the buffer
			 * means that the handling was incomplete and that the data we
			 * might have generated is invalid. */
			self.handler.queue.clear();

			Ok(0)
		} else {
			Ok(self.handler.queue.len())
		}
	}

	/// Tries to receive an event from the device.
	///
	/// This function returns immediately, regardless of whether a message is
	/// available or not. If you wish to have blocking behavior, use [`recv()`]
	/// instead.
	///
	/// [`recv()`]: Self::recv
	pub fn try_recv(&mut self) -> Result<Event, TryRecvError> {
		if let Some(event) = self.handler.pop_event() {
			/* Don't bother calling the device for more info if we already have
			 * data to feed our client with right away. */
			return Ok(event)
		}

		let report = unsafe {
			let mut report = std::ptr::null_mut();
			let mut length = 0;
			let mut available = 0;

			InternalError::from_wacom_stu({
				stu_sys::WacomGSS_InterfaceQueue_try_getReport(
					self.queue.0,
					&mut report,
					&mut length,
					&mut available)
			}).map_err(InternalError::unwrap_to_general)
				.map_err(TryRecvError::Failed)?;

			if available != 0 {
				Some(Handle::wrap_slice(report, length as _))
			} else {
				None
			}
		};

		report
			.ok_or(TryRecvError::Empty)
			.and_then(|report| {
				self.handle(report)
					.map_err(TryRecvError::Failed)?;

				self.handler.queue.pop_front()
					.ok_or(TryRecvError::Empty)
			})
	}

	/// Tries to receive an event from the device.
	///
	/// This function returns immediately if a message is already available and
	/// blocks, waiting for a message to arrive, otherwise. If you wish to have
	/// non-blocking behavior, use [`try_recv()`] instead.
	///
	/// [`try_recv()`]: Self::try_recv
	pub fn recv(&mut self) -> Result<Event, Error> {
		if let Some(event) = self.handler.pop_event() {
			/* Don't bother calling the device for more info if we already have
			 * data to feed our client with right away. */
			return Ok(event)
		}

		let report = unsafe {
			let mut report = std::ptr::null_mut();
			let mut length = 0;

			InternalError::from_wacom_stu({
				stu_sys::WacomGSS_InterfaceQueue_wait_getReport(
					self.queue.0,
					&mut report,
					&mut length)
			}).map_err(InternalError::unwrap_to_general)?;

			Handle::wrap_slice(report, length as _)
		};

		self.handle(report)?;
		self.handler.queue.pop_front()
			.ok_or(Error::ClientError(ClientError::InvalidReport))
	}
}

/// The raw type holding a pointer to a Wacom STU API queue.
struct RawQueue(stu_sys::WacomGSS_InterfaceQueue);
impl Drop for RawQueue {
	fn drop(&mut self) {
		unsafe {
			let _ = stu_sys::WacomGSS_InterfaceQueue_free(self.0);
		}
	}
}

/// The table of report handler functions.
const REPORT_HANDLER_FUNCTIONS: stu_sys::WacomGSS_ReportHandlerFunctionTable = stu_sys::WacomGSS_ReportHandlerFunctionTable {
	onPenData: Some(on_pen_data),
	onPenDataOption: None,
	onPenDataEncrypted: None,
	onPenDataEncryptedOption: None,
	onDevicePublicKey: None,
	decrypt: None,
	onPenDataTimeCountSequence: None,
	onPenDataTimeCountSequenceEncrypted: None,
	onEncryptionStatus: None,
	onEventData: None,
	onEventDataPinPad: None,
	onEventDataKeyPad: None,
	onEventDataSignature: None,
	onEventDataEncrypted: None,
	onEventDataPinPadEncrypted: None,
	onEventDataKeyPadEncrypted: None,
	onEventDataSignatureEncrypted: None
};

/// This structure holds the data shared between the handler functions and the
/// receiving function in the [queue]. Since the callbacks are C callbacks, this
/// structure serves as the local state those callbacks are allowed to change.
///
/// [queue]: Queue
#[derive(Debug)]
struct ReportHandler {
	/// The resolution of this screen in each of the three axes.
	resolution: (u32, u32, u32),
	/// The internal queue of converted events.
	queue: VecDeque<Event>,
}
impl ReportHandler {
	/// Enqueue a new event on this handler.
	pub fn push_event(&mut self, event: Event) {
		self.queue.push_back(event)
	}

	/// Pop the oldest event, if it is available.
	pub fn pop_event(&mut self) -> Option<Event> {
		self.queue.pop_front()
	}
}

/// Generic handler for pen data callbacks.
unsafe extern "C" fn on_pen_data(
	handler: *mut std::os::raw::c_void,
	_size_of_pen_data: stu_sys::size_t,
	pen_data: *const stu_sys::WacomGSS_PenData) -> std::os::raw::c_int {

	let this = &mut *(handler as *mut ReportHandler);
	assert_ne!(this.resolution.0, 0);
	assert_ne!(this.resolution.1, 0);
	assert_ne!(this.resolution.2, 0);

	let pen_data = *pen_data;
	this.push_event(Event {
		timestamp: Instant::now(),
		position: (
			(f64::from(pen_data.x) / f64::from(this.resolution.0)).clamp(0.0, 1.0),
			(f64::from(pen_data.y) / f64::from(this.resolution.1)).clamp(0.0, 1.0),
			(f64::from(pen_data.pressure) / f64::from(this.resolution.2)).clamp(0.0, 1.0),
		),
		touching: pen_data.sw != 0,
		close: pen_data.rdy != 0
	});

	0
}

/// This structure enumerates the reasons why an event may not be available.
#[derive(Debug)]
pub enum TryRecvError {
	/// The interface is valid, but there are still no more events to be read.
	Empty,
	/// The interface has returned an error and should be considered invalid.
	Failed(Error)
}