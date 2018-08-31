//! These types prevent accidentally interpreting a network byte order integer as host byte order.

#![allow(dead_code)]

use std::{u32, u16};

#[derive(Clone, Copy, Debug)]
pub struct NetworkU16 {
    data: u16
}

impl NetworkU16 {
    pub fn new(input: u16) -> NetworkU16 {
        NetworkU16 {
            data: u16::to_be(input),
        }
    }

    pub fn get(self) -> u16 {
        u16::from_be(self.data)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NetworkU32 {
    data: u32
}


impl NetworkU32 {
    pub fn new(input: u32) -> NetworkU32 {
        NetworkU32 {
            data: u32::to_be(input),
        }
    }

    pub fn get(self) -> u32 {
        u32::from_be(self.data)
    }
}
