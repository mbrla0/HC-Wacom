use crate::handle::Handle;
use crate::error::InternalError;

/// Handling of errors from the Wacom STU interface.
mod error;
pub use error::{Exception, Error};
use std::collections::HashSet;

/// Handles to memory managed by the Wacom STU allocator.
mod handle;

/// The interface to a Wacom STU tablet.
pub struct Tablet {
	/// The raw handle to the tablet interface.
	raw: RawTablet,
	/// The list of reports types supported by this tablet.
	supported_reports: HashSet<stu_sys::tagWacomGSS_ReportId>,
}
impl Tablet {
	/// Create a new Tablet instance from the given RawTablet interface.
	pub(crate) fn wrap(raw: RawTablet) -> Result<Self, Error> {
		assert!(raw.connected());

		let supported_reports = {
			let report_list = unsafe {
				let mut list = std::ptr::null_mut();
				let mut length = 0;

				println!("A");
				InternalError::from_wacom_stu({
					stu_sys::WacomGSS_Protocol_getReportSizeCollection(
						raw.interface,
						&mut length,
						&mut list)
				}).map_err(InternalError::unwrap_to_general)?;

				println!("B");
				Handle::wrap_slice(list, length as _)
			};

			let mut supported = HashSet::with_capacity(report_list.len());
			for i in 0..report_list.len() {
				if report_list[i] != 0 {
					/* Mark this report type as being supported. */
					supported.insert(i as _);
				}
			}

			supported
		};

		for i in &supported_reports {
			eprintln!("   - Supports ReportId: {}", i)
		}

		Ok(Self {
			raw,
			supported_reports
		})
	}
}

struct RawTablet {
	interface: stu_sys::WacomGSS_Interface,
}
impl RawTablet {
	fn connected(&self) -> bool {
		let mut connected = 0;
		let _ = InternalError::from_wacom_stu(unsafe {
			stu_sys::WacomGSS_Interface_isConnected(
				self.interface,
				&mut connected)
		}).map_err(InternalError::unwrap_to_general);

		connected != 0
	}
}
impl Drop for RawTablet {
	fn drop(&mut self) {
		unsafe {
			let _ = stu_sys::WacomGSS_Interface_disconnect(self.interface);
			let _ = stu_sys::WacomGSS_Interface_free(self.interface);
		}
	}
}

/// The structure containing information about a device.
pub struct Information<'a> {
	device: &'a stu_sys::WacomGSS_UsbDevice
}
impl Information<'_> {
	/// Vendor identification number of this device.
	pub fn vendor(&self) -> u16 {
		self.device.usbDevice.idVendor
	}

	/// Product identification number of this device.
	pub fn product(&self) -> u16 {
		self.device.usbDevice.idProduct
	}
}

/// A connector to a tablet device.
///
/// This structure has no functionality for direct communication with a tablet,
/// and, instead, only serves to initiate a connection to a device attached to
/// the system. This structure also provides a means to identify the device
/// before a connection is established.
pub struct Connector {
	device: stu_sys::WacomGSS_UsbDevice,
}
impl Connector {
	/// Get the information about the device this connector is targeting.
	pub fn info(&self) -> Information {
		Information {
			device: &self.device
		}
	}

	/// Try to connect to the device this connector is targeting.
	pub fn connect(self) -> Result<Tablet, Error> {
		let interface = unsafe {
			let mut interface = std::mem::zeroed();
			InternalError::from_wacom_stu({
				stu_sys::WacomGSS_UsbInterface_create_1(
					std::mem::size_of::<stu_sys::WacomGSS_UsbDevice>() as _,
					&self.device,
					false as _,
					&mut interface)
			}).map_err(InternalError::unwrap_to_general)?;

			interface
		};

		Tablet::wrap(RawTablet { interface })
	}
}

/// An iterator over the [connectors] currently available to the application.
///
/// This structure is obtained from the [`list_devices()`] function in this
/// crate.
///
/// [connectors]: Connector
/// [`list_devices()`]: list_devices
pub struct Connectors {
	values: Handle<[stu_sys::WacomGSS_UsbDevice]>,
	index: usize,
}
impl Iterator for Connectors {
	type Item = Connector;
	fn next(&mut self) -> Option<Self::Item> {
		let val = self.values.get(self.index)
			.map(|device| Connector {
				device: *device
			});
		self.index = self.index.saturating_add(1);

		val
	}
}

/// List all of the currently available devices.
///
/// # Panic
/// This function panics if USB devices are not supported by the system.
pub fn list_devices() -> Connectors {
	let devices = unsafe {
		let mut count = 0;
		let mut devices = std::ptr::null_mut();
		InternalError::from_wacom_stu({
			stu_sys::WacomGSS_getUsbDevices(
				std::mem::size_of::<stu_sys::WacomGSS_UsbDevice>() as _,
				&mut count,
				&mut devices)
		}).unwrap();

		Handle::wrap_slice(devices, count as _)
	};

	Connectors {
		values: devices,
		index: 0
	}
}