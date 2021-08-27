fn main() {
	for device in stu::list_devices() {
		println!("{:04x}:{:04x}", device.info().vendor(), device.info().product());
		let mut device = device.connect().unwrap();
		let caps = device.capability().unwrap();
		device.clear().unwrap();

		println!("LCD Width: {}", caps.width());
		println!("LCD Height: {}", caps.height());
		println!("Table Width: {}", caps.input_grid_width());
		println!("Table Height: {}", caps.input_grid_height());
		println!("Table Depth: {}", caps.input_grid_pressure());

		device.inking(true);

		let mut queue = device.queue().unwrap();
		loop {
			let event = queue.recv().unwrap();

			println!("{:?}", event);
		}
	}
}
