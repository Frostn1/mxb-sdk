//! Public, headless tools for finding PiBoSo game folders and managing mods.
//!
//! The modules cover folder discovery, mod inventory and identification, safe install and
//! uninstall plans, verification, paints, and presets.
//!
//! # What this crate will never do
//!
//! It has no network client, does not interpret protected packages, and never permanently
//! deletes a user's files. Removal always goes through the operating system trash.

pub mod folders;
pub mod identify;
pub mod install;
pub mod mods;
pub mod paints;
pub mod placement;
pub mod presets;
pub mod uninstall;
pub mod verify;

pub(crate) mod game {
    pub use crate::placement::{is_rider_model_area, ALL_MODS_DIRS};
}

pub(crate) mod bikeswap {
    pub(crate) use crate::placement::read_identity;
}

use std::fmt;

/// An error returned by the public MX Bikes operations.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    Invalid(String),
    UnsafePath(String),
    Unavailable(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Zip(e) => write!(f, "{e}"),
            Self::Json(e) => write!(f, "{e}"),
            Self::Invalid(e) | Self::UnsafePath(e) | Self::Unavailable(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
