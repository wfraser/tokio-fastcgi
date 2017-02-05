//! These types prevent accidentally interpreting a network byte order integer as host byte order.

#![allow(dead_code)]

use byteorder::*;
use std::mem::transmute;
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
                    transmute(&mut out),
                    2
                )
            },
            input
        );
        NetworkU16 {
            data: out
        }
    }

    pub fn get(&self) -> u16 {
        NetworkEndian::read_u16(
            unsafe {
                slice::from_raw_parts(
                    transmute(&self.data),
                    2
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
                    transmute(&mut out),
                    4
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
                    transmute(&self.data),
                    4
                )
            }
        )
    }
}
