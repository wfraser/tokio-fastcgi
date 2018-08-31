use std::slice;
use std::mem::size_of;

/// Return a slice of the bytes in the given struct.
/// This is safe because the input type must implement Copy.
pub fn as_bytes<T: Copy>(x: &T) -> &[u8] {
    unsafe {
        slice::from_raw_parts(
            x as *const T as *const u8,
            size_of::<T>())
    }
}

/// This is kind of like `std::mem::transmute_copy`, but safer because:
///  * the output type is restricted to those that implement Copy
///  * the input type may only be a byte slice
///  * the length of the input is asserted to be big enough to construct the output
/// These restrictions make it safe.
pub fn from_bytes<T: Copy>(bytes: &[u8]) -> T {
    assert!(bytes.len() >= size_of::<T>());
    unsafe { *(bytes.as_ptr() as *const T) }
}
