//! MSSQL backend for the generic query compiler.

use std::fmt;

use super::core::SqlDialect;
use super::sealed;

/// Error returned when a physical 1C MSSQL year offset is unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidMsSqlYearOffset {
    year_offset: i32,
}

impl InvalidMsSqlYearOffset {
    /// Returns the rejected offset.
    #[must_use]
    pub const fn year_offset(self) -> i32 {
        self.year_offset
    }
}

impl fmt::Display for InvalidMsSqlYearOffset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "MSSQL year offset {} is outside supported range 0..=10000",
            self.year_offset
        )
    }
}

impl std::error::Error for InvalidMsSqlYearOffset {}

/// Immutable MSSQL backend configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MsSqlBackend {
    year_offset: i32,
}

impl MsSqlBackend {
    /// Largest accepted physical 1C year offset.
    pub const MAX_YEAR_OFFSET: i32 = 10_000;

    /// Creates an MSSQL backend with the physical 1C date offset.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidMsSqlYearOffset`] when the offset is outside
    /// `0..=10000`.
    pub const fn new(year_offset: i32) -> Result<Self, InvalidMsSqlYearOffset> {
        if year_offset < 0 || year_offset > Self::MAX_YEAR_OFFSET {
            Err(InvalidMsSqlYearOffset { year_offset })
        } else {
            Ok(Self { year_offset })
        }
    }

    /// Returns the physical 1C date offset.
    #[must_use]
    pub const fn year_offset(self) -> i32 {
        self.year_offset
    }
}

impl sealed::Sealed for MsSqlBackend {
    fn dialect(self) -> SqlDialect {
        SqlDialect::mssql(self.year_offset)
    }
}
