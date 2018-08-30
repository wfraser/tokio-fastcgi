use std::slice;
use std::mem::size_of;

pub unsafe fn as_bytes<T: Copy>(x: &T) -> &[u8] {
    slice::from_raw_parts(
        x as *const T as *const u8,
        size_of::<T>()
    )
}

pub unsafe fn from_bytes<T: Copy>(bytes: &[u8]) -> T {
    *(bytes.as_ptr() as *const T)
}
