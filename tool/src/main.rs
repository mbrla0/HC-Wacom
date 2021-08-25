fn main() {
	for device in stu::list_devices() {
		println!("{:04x}:{:04x}", device.info().vendor(), device.info().product());
		if let Err(what) = device.connect() {
			eprintln!("ERROR: {:?}", what);
			eprintln!("{}", what);
		}
	}
}
