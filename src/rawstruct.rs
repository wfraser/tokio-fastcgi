use std::slice;
use std::mem::{size_of, transmute};

pub unsafe fn as_bytes<T: Copy>(x: &T) -> &[u8] {
    slice::from_raw_parts(
        x as *const T as *const u8,
        size_of::<T>()
    )
}

pub unsafe fn from_bytes<T: Copy>(bytes: &[u8]) -> T {
    *transmute::<*const u8, *const T>(bytes.as_ptr())
}
