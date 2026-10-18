// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Unified captured frame: BGRA pixels, row-major, no padding.

/// One captured video frame, in the canonical form every backend produces.
#[derive(Debug)]
pub struct Frame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// BGRA pixel data, `width * height * 4` bytes, row-major, no padding.
    pub data: Vec<u8>,
}

impl Frame {
    /// Build a frame, validating the buffer size.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        if data.len() == width as usize * height as usize * 4 {
            Some(Self { width, height, data })
        } else {
            None
        }
    }
}
