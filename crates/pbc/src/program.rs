//! A validated program: the constant table and the code (SPEC annex A).

use thiserror::Error;

/// The most constants a program may have (`const_count` is a `u16` in `0..=4095`, A.1).
pub const MAX_CONSTANTS: usize = 4095;

/// The longest code section a program may have (`code_len` is in `1..=65535`, A.1).
pub const MAX_CODE_LEN: usize = 65_535;

/// Why a `.pbc` file, or a set of program parts, was refused.
///
/// The spec reports every one of these as the single fault `BadHeader` (A.2, B.5): the
/// variants only exist to make our own errors readable. A load error is never a
/// runtime fault, because a program that does not load is never run. That is why this
/// type lives here and not in `vm::Fault` (journal D-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LoadError {
    /// The first four bytes are not `50 42 43 31` (`"PBC1"`).
    #[error("the file does not start with the magic bytes \"PBC1\"")]
    BadMagic,
    /// The version byte is not `0x01`.
    #[error("unsupported version byte (expected 0x01)")]
    BadVersion,
    /// The flags byte is not `0x00`.
    #[error("the flags byte must be 0x00 in v1")]
    BadFlags,
    /// More than [`MAX_CONSTANTS`] constants.
    #[error("more than {MAX_CONSTANTS} constants")]
    TooManyConstants,
    /// The code section is empty or longer than [`MAX_CODE_LEN`].
    #[error("the code must be between 1 and {MAX_CODE_LEN} bytes long")]
    BadCodeLength,
    /// The file ends before the size its header announces.
    #[error("the file is shorter than its header announces")]
    Truncated,
    /// The file has bytes after the size its header announces.
    #[error("the file is longer than its header announces")]
    TrailingBytes,
}

impl LoadError {
    /// The fault name the spec uses for every load error (B.5).
    pub const FAULT_NAME: &'static str = "BadHeader";
}

/// A program that respects the limits of SPEC A.1.
///
/// The fields are private, so a `Program` can only be built through
/// [`Program::from_parts`] (or the loader): every `Program` in the system is valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    constants: Vec<i64>,
    code: Vec<u8>,
}

impl Program {
    /// Builds a program from its constant table and its code, checking the A.1 limits.
    ///
    /// The loader calls this once it has parsed a file. Tests call it directly to build
    /// programs by hand.
    pub fn from_parts(constants: Vec<i64>, code: Vec<u8>) -> Result<Self, LoadError> {
        if constants.len() > MAX_CONSTANTS {
            return Err(LoadError::TooManyConstants);
        }
        if code.is_empty() || code.len() > MAX_CODE_LEN {
            return Err(LoadError::BadCodeLength);
        }
        Ok(Self { constants, code })
    }

    /// The constant table, read by `CONST` and `CONSTX`.
    pub fn constants(&self) -> &[i64] {
        &self.constants
    }

    /// The code bytes. Never empty.
    pub fn code(&self) -> &[u8] {
        &self.code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_limits() {
        assert!(Program::from_parts(vec![], vec![0x00]).is_ok());
        assert!(Program::from_parts(vec![7; MAX_CONSTANTS], vec![0x00; MAX_CODE_LEN]).is_ok());
    }

    #[test]
    fn refuses_too_many_constants() {
        assert_eq!(
            Program::from_parts(vec![0; MAX_CONSTANTS + 1], vec![0x00]),
            Err(LoadError::TooManyConstants)
        );
    }

    #[test]
    fn refuses_empty_or_oversized_code() {
        assert_eq!(
            Program::from_parts(vec![], vec![]),
            Err(LoadError::BadCodeLength)
        );
        assert_eq!(
            Program::from_parts(vec![], vec![0x00; MAX_CODE_LEN + 1]),
            Err(LoadError::BadCodeLength)
        );
    }

    #[test]
    fn keeps_the_parts() {
        let p = Program::from_parts(vec![1, -2], vec![0x01, 0x00, 0x01, 0x00]).unwrap();
        assert_eq!(p.constants(), &[1, -2]);
        assert_eq!(p.code(), &[0x01, 0x00, 0x01, 0x00]);
    }
}
