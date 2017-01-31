use byteorder::*;
use std::mem::transmute;
use std::slice;

pub fn htons(input: u16) -> u16 {
    let mut out = 0u16;
    NetworkEndian::write_u16(
        unsafe {
            slice::from_raw_parts_mut(
                transmute(&mut out),
                2
            )
        },
        input
    );
    out
}

pub fn ntohs(input: u16) -> u16 {
    NetworkEndian::read_u16(
        unsafe {
            slice::from_raw_parts(
                transmute(&input),
                2
            )
        }
    )
}
