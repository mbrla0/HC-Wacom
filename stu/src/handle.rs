use std::ops::{Deref, DerefMut};

/// A handle to memory managed by the Wacom STU runtime.
#[derive(Debug)]
pub struct Handle<T: ?Sized>(*mut T);
impl<T> Handle<T> {
	/// Wrap the given Wacom STU pointer in a new slice handle.
	///
	/// This function is very close to [`wrap()`], except that it is intended to
	/// create handles to slices, rather than to direct types. The same safety
	/// requirements apply to this function.
	pub unsafe fn wrap_slice(ptr: *mut T, length: usize) -> Handle<[T]> {
		Handle(std::ptr::slice_from_raw_parts_mut(ptr, length))
	}
}
impl<T: ?Sized> Handle<T> {
	/// Wrap the given Wacom STU pointer in a new handle.
	///
	/// This function expects pointers to memory allocations handed out by the
	/// Wacom STU API, exclusively. Calling this function with any other kind
	/// of pointer is unsafe and leads to undefined behavior.
	///
	/// The usual safety rules around pointer dereference are all required to be
	/// applicable to the given pointer.
	pub unsafe fn wrap(ptr: *mut T) -> Self {
		Self(ptr)
	}

	/// Transmute the type this handle points to into a new type.
	///
	/// The usual safety rules for transmutation apply, with the addition of the
	/// extra requirement that the internal pointer be aligned such that it
	/// allows values of the new type to be read from and written to.
	///
	/// The alignment requirement for this type may be checked against the
	/// pointer obtained from the [`as_ptr()`] function.
	///
	/// [`as_ptr()`]: Self::as_ptr
	pub unsafe fn transmute<U>(self) -> Handle<U> {
		Handle(self.0 as *mut _)
	}

	/// Get the underlying pointer backing this handle.
	pub const fn as_ptr(&self) -> *mut T {
		self.0
	}
}
impl<T: ?Sized> AsRef<T> for Handle<T> {
	fn as_ref(&self) -> &T {
		unsafe {
			/* Handle invariants guarantee that this operation is safe. */
			&*self.0
		}
	}
}
impl<T: ?Sized> AsMut<T> for Handle<T> {
	fn as_mut(&mut self) -> &mut T {
		unsafe {
			/* Handle invariants guarantee that this operation is safe. */
			&mut *self.0
		}
	}
}
impl<T: ?Sized> Deref for Handle<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		self.as_ref()
	}
}
impl<T: ?Sized> DerefMut for Handle<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.as_mut()
	}
}
impl<T: ?Sized> Drop for Handle<T> {
	fn drop(&mut self) {
		unsafe {
			/* Execute the drop implementation of the type before we return the
			 * memory to the STU manager. This is generally not needed, seeing
			 * as the handles given out by STU are all handles to types which
			 * have no drop implementation.
			 *
			 * Nonetheless, it's good that we account for types that *do*
			 * implement drop, because such a type may still be acquired via
			 * transmutation of this handle. */
			std::ptr::drop_in_place(self.0);

			/* Return the memory to the STU.
			 *
			 * At this point, this operation is safe. As we define the handle
			 * must have a valid pointer to an STU managed memory region when it
			 * is created, we assume that invariant has not been violated. */
			let _ = stu_sys::WacomGSS_free(self.0 as *mut _);
		}
	}
}

/// The Wacom STU pointers are Send-safe.
unsafe impl<T: Send> Send for Handle<T> {}
/// The Wacom STU pointers are Sync-safe.
unsafe impl<T: Sync> Sync for Handle<T> {}

