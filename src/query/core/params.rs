//! Named query parameters and compilation options.

use std::fmt;

use crate::Token;
use crate::metadata::ObjectId;
use crate::query::core::ast::days_in_month;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::PresentationPlan;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// A calendar date and time supplied as a query parameter.
///
/// Values are validated on construction, so a stored date always renders
/// as a legal SQL literal on both providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParameterDate {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

/// The reason a [`ParameterDate`] could not be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InvalidParameterDate {
    component: &'static str,
}

impl fmt::Display for InvalidParameterDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parameter date {} is out of range",
            self.component
        )
    }
}

impl std::error::Error for InvalidParameterDate {}

impl ParameterDate {
    /// Builds a date from its components, validating every field.
    ///
    /// # Errors
    ///
    /// Returns an error when the year is outside `1..=9999`, the month or
    /// day does not exist, or a time component is out of range.
    pub const fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<Self, InvalidParameterDate> {
        if year == 0 || year > 9999 {
            return Err(InvalidParameterDate { component: "year" });
        }
        if month == 0 || month > 12 {
            return Err(InvalidParameterDate { component: "month" });
        }
        if day == 0 || day > days_in_month(year, month) {
            return Err(InvalidParameterDate { component: "day" });
        }
        if hour > 23 {
            return Err(InvalidParameterDate { component: "hour" });
        }
        if minute > 59 {
            return Err(InvalidParameterDate {
                component: "minute",
            });
        }
        if second > 59 {
            return Err(InvalidParameterDate {
                component: "second",
            });
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// The year in `1..=9999`.
    #[must_use]
    pub const fn year(self) -> u16 {
        self.year
    }

    /// The month in `1..=12`.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    /// The day of the month.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }

    /// The hour in `0..=23`.
    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// The minute in `0..=59`.
    #[must_use]
    pub const fn minute(self) -> u8 {
        self.minute
    }

    /// The second in `0..=59`.
    #[must_use]
    pub const fn second(self) -> u8 {
        self.second
    }
}

/// The value bound to a `&Имя` query parameter.
///
/// Values are inlined into generated SQL as typed literals of the target
/// dialect; there are no placeholders to bind afterwards. A [`Self::List`]
/// is valid only as the operand of `В`/`IN`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParameterValue {
    /// SQL `NULL`; compatible with every column kind.
    Null,
    /// A boolean rendered per dialect and usable as a predicate.
    Boolean(bool),
    /// A decimal number: `unscaled × 10^-scale`, so `{ unscaled: 1550,
    /// scale: 2 }` renders as `15.50`.
    Number {
        /// The digits without a decimal point.
        unscaled: i128,
        /// How many trailing digits sit after the decimal point.
        scale: u8,
    },
    /// A string rendered with the dialect's quoting.
    String(String),
    /// A date and time; MSSQL applies the backend year offset.
    Date(ParameterDate),
    /// A 16-byte 1C reference to a row of `object`.
    Reference {
        /// The metadata object the reference points to.
        object: ObjectId,
        /// The physical `RRRef` bytes.
        id: [u8; 16],
    },
    /// Raw bytes rendered as a binary literal; compared like a `0x…` literal.
    Binary(Vec<u8>),
    /// A list of scalar values for `В (&Список)`.
    List(Vec<ParameterValue>),
}

/// A named parameter supplied for one compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParameter {
    name: String,
    value: ParameterValue,
}

impl QueryParameter {
    /// Binds `value` to the parameter written as `&name` in the source.
    /// Names are matched case-insensitively, like every 1C identifier.
    #[must_use]
    pub fn new(name: impl Into<String>, value: ParameterValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    /// The parameter name without the leading `&`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The bound value.
    #[must_use]
    pub const fn value(&self) -> &ParameterValue {
        &self.value
    }
}

/// Inputs beyond the source text that a compilation may need.
///
/// ```
/// use open_sdbl::query::{CompileOptions, ParameterValue, QueryParameter};
///
/// let parameters = [QueryParameter::new("Лимит", ParameterValue::Number { unscaled: 100, scale: 0 })];
/// let options = CompileOptions::new().parameters(&parameters);
/// assert_eq!(options.parameter_values().len(), 1);
/// ```
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct CompileOptions<'a> {
    presentations: &'a [PresentationPlan],
    parameters: &'a [QueryParameter],
}

impl<'a> CompileOptions<'a> {
    /// Options without presentation plans or parameters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            presentations: &[],
            parameters: &[],
        }
    }

    /// Supplies application presentation plans.
    #[must_use]
    pub const fn presentations(mut self, plans: &'a [PresentationPlan]) -> Self {
        self.presentations = plans;
        self
    }

    /// Supplies named parameter values.
    #[must_use]
    pub const fn parameters(mut self, parameters: &'a [QueryParameter]) -> Self {
        self.parameters = parameters;
        self
    }

    /// The presentation plans in effect.
    #[must_use]
    pub const fn presentation_plans(&self) -> &'a [PresentationPlan] {
        self.presentations
    }

    /// The parameter values in effect.
    #[must_use]
    pub const fn parameter_values(&self) -> &'a [QueryParameter] {
        self.parameters
    }
}

/// The parameter values of one compilation. Preparation runs unbound: every
/// parameter then has a wildcard kind and renders as `NULL`, which is enough
/// to collect presentation targets.
#[derive(Debug, Clone, Copy)]
pub(super) struct Parameters<'a> {
    values: &'a [QueryParameter],
    bound: bool,
}

impl<'a> Parameters<'a> {
    pub(super) const fn bound(values: &'a [QueryParameter]) -> Self {
        Self {
            values,
            bound: true,
        }
    }

    pub(super) const fn unbound() -> Self {
        Self {
            values: &[],
            bound: false,
        }
    }

    /// Finds the value of a parameter token; `None` while unbound.
    pub(super) fn lookup(
        &self,
        token: &Token<'_>,
    ) -> Result<Option<&'a ParameterValue>, QueryDiagnostic> {
        if !self.bound {
            return Ok(None);
        }
        let name = parameter_name(token);
        self.values
            .iter()
            .find(|parameter| names_equal(parameter.name(), name))
            .map(|parameter| Some(parameter.value()))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Parameter,
                    Some(token),
                    format!("parameter {:?} has no value", token.lexeme),
                )
            })
    }
}

/// The name of a parameter token without its leading `&`.
pub(super) fn parameter_name<'source>(token: &Token<'source>) -> &'source str {
    token.lexeme.strip_prefix('&').unwrap_or(token.lexeme)
}
