use thiserror::Error;

use super::{FrameFormatKind, FrameSizeKind};

#[derive(Error, Debug)]
pub enum FrameError {
    #[error("Frame size error: {kind} - {details}")]
    Size {
        kind: FrameSizeKind,
        details: String,
        frame_data: Option<Vec<u8>>,
    },

    #[error("Frame format error: {kind} - {details}")]
    Format {
        kind: FrameFormatKind,
        details: String,
        frame_data: Option<Vec<u8>>,
    },

    #[error("CRC error: calculated={calculated:04X}, received={received:04X}, frame={frame_hex}")]
    Crc {
        calculated: u16,
        received: u16,
        frame_hex: String,
    },
}


