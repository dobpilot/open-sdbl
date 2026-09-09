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

/// Capability level of the target SQL Server that generated T-SQL may rely on.
///
/// Levels name feature sets rather than exact versions: `Sql2008` emits only
/// functions available on SQL Server 2008/2008 R2 (emulating newer ones with
/// equivalent arithmetic), while `Sql2012` may use functions introduced in
/// SQL Server 2012 such as `DATETIME2FROMPARTS`. Both levels yield the same
/// logical values. The enum is `#[non_exhaustive]` so that further levels
/// can be added without a breaking change.
#[non_exhaustive]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MsSqlDialectLevel {
    /// SQL Server 2008 and 2008 R2.
    Sql2008,
    /// SQL Server 2012 and newer.
    #[default]
    Sql2012,
}

impl MsSqlDialectLevel {
    /// Maps a `SERVERPROPERTY('ProductVersion')` string such as
    /// `10.50.6000.34` to a level; returns `None` when the major version
    /// cannot be parsed or is older than SQL Server 2008.
    #[must_use]
    pub fn from_product_version(version: &str) -> Option<Self> {
        let major = version.trim().split('.').next()?.parse::<u32>().ok()?;
        match major {
            0..=9 => None,
            10 => Some(Self::Sql2008),
            _ => Some(Self::Sql2012),
        }
    }

    /// Parses the level name used by command-line options: `2008` or `2012`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "2008" => Some(Self::Sql2008),
            "2012" => Some(Self::Sql2012),
            _ => None,
        }
    }

    /// Returns the stable level name: `2008` or `2012`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sql2008 => "2008",
            Self::Sql2012 => "2012",
        }
    }
}

impl fmt::Display for MsSqlDialectLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Immutable MSSQL backend configuration: the physical 1C year offset and
/// the dialect level of the target server.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MsSqlBackend {
    year_offset: i32,
    dialect_level: MsSqlDialectLevel,
}

impl MsSqlBackend {
    /// Largest accepted physical 1C year offset.
    pub const MAX_YEAR_OFFSET: i32 = 10_000;

    /// Creates an MSSQL backend with the physical 1C date offset and the
    /// default (newest) dialect level.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidMsSqlYearOffset`] when the offset is outside
    /// `0..=10000`.
    pub const fn new(year_offset: i32) -> Result<Self, InvalidMsSqlYearOffset> {
        if year_offset < 0 || year_offset > Self::MAX_YEAR_OFFSET {
            Err(InvalidMsSqlYearOffset { year_offset })
        } else {
            Ok(Self {
                year_offset,
                dialect_level: MsSqlDialectLevel::Sql2012,
            })
        }
    }

    /// Returns a backend targeting the given dialect level.
    #[must_use]
    pub const fn with_dialect_level(self, dialect_level: MsSqlDialectLevel) -> Self {
        Self {
            year_offset: self.year_offset,
            dialect_level,
        }
    }

    /// Returns the physical 1C date offset.
    #[must_use]
    pub const fn year_offset(self) -> i32 {
        self.year_offset
    }

    /// Returns the dialect level generated SQL targets.
    #[must_use]
    pub const fn dialect_level(self) -> MsSqlDialectLevel {
        self.dialect_level
    }
}

impl sealed::Sealed for MsSqlBackend {
    fn dialect(self) -> SqlDialect {
        SqlDialect::mssql(self.year_offset, self.dialect_level)
    }
}

#[cfg(test)]
mod tests {
    use super::{MsSqlBackend, MsSqlDialectLevel};

    #[test]
    fn maps_product_versions_to_levels() {
        assert_eq!(
            MsSqlDialectLevel::from_product_version("10.50.6000.34"),
            Some(MsSqlDialectLevel::Sql2008)
        );
        assert_eq!(
            MsSqlDialectLevel::from_product_version("10.0.5500.0"),
            Some(MsSqlDialectLevel::Sql2008)
        );
        assert_eq!(
            MsSqlDialectLevel::from_product_version("11.0.7001.0"),
            Some(MsSqlDialectLevel::Sql2012)
        );
        assert_eq!(
            MsSqlDialectLevel::from_product_version(" 16.0.1000.6 "),
            Some(MsSqlDialectLevel::Sql2012)
        );
        assert_eq!(MsSqlDialectLevel::from_product_version("9.0.5000"), None);
        assert_eq!(MsSqlDialectLevel::from_product_version("garbage"), None);
        assert_eq!(
            MsSqlDialectLevel::parse("2008"),
            Some(MsSqlDialectLevel::Sql2008)
        );
        assert_eq!(MsSqlDialectLevel::parse("2019"), None);
        assert_eq!(MsSqlDialectLevel::Sql2008.to_string(), "2008");
    }

    #[test]
    fn backend_defaults_to_the_newest_level() {
        let backend = MsSqlBackend::new(2000).unwrap();
        assert_eq!(backend.dialect_level(), MsSqlDialectLevel::Sql2012);
        assert_eq!(
            MsSqlBackend::default().dialect_level(),
            MsSqlDialectLevel::Sql2012
        );
        let legacy = backend.with_dialect_level(MsSqlDialectLevel::Sql2008);
        assert_eq!(legacy.year_offset(), 2000);
        assert_eq!(legacy.dialect_level(), MsSqlDialectLevel::Sql2008);
    }
}
