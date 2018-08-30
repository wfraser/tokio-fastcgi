//! These types prevent accidentally interpreting a network byte order integer as host byte order.

#![allow(dead_code)]

use byteorder::*;
use std::mem::size_of;
use std::slice;

#[derive(Clone, Copy, Debug)]
pub struct NetworkU16 {
    data: u16
}

impl NetworkU16 {
    pub fn new(input: u16) -> NetworkU16 {
        let mut out = 0;
        NetworkEndian::write_u16(
            unsafe {
                slice::from_raw_parts_mut(
                    &mut out as *mut u16 as *mut u8,
                    size_of::<u16>()
                )
            },
            input
        );
        NetworkU16 {
            data: out
        }
    }

    pub fn get(self) -> u16 {
        NetworkEndian::read_u16(
            unsafe {
                slice::from_raw_parts(
                    &self.data as *const u16 as *const u8,
                    size_of::<u16>()
                )
            }
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NetworkU32 {
    data: u32
}


impl NetworkU32 {
    pub fn new(input: u32) -> NetworkU32 {
        let mut out = 0;
        NetworkEndian::write_u32(
            unsafe {
                slice::from_raw_parts_mut(
                    &mut out as *mut u32 as *mut u8,
                    size_of::<u32>()
                )
            },
            input
        );
        NetworkU32 {
            data: out
        }
    }

    pub fn get(&self) -> u32 {
        NetworkEndian::read_u32(
            unsafe {
                slice::from_raw_parts(
                    &self.data as *const u32 as *const u8,
                    size_of::<u32>()
                )
            }
        )
    }
}
