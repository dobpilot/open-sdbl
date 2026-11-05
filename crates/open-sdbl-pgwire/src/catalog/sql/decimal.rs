//! Exact bounded decimals: comparisons never pass through floating point.
use super::ErrorFields;
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Decimal {
    unscaled: i128,
    scale: u32,
}
impl Decimal {
    pub(super) fn parse(s: &str) -> Result<Self, ErrorFields> {
        let invalid = || ErrorFields::error("22P02", "invalid numeric");
        let (negative, s) = if let Some(s) = s.strip_prefix('-') {
            (true, s)
        } else {
            (false, s.strip_prefix('+').unwrap_or(s))
        };
        let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
        if whole.is_empty() && frac.is_empty()
            || !whole
                .chars()
                .chain(frac.chars())
                .all(|c| c.is_ascii_digit())
        {
            return Err(invalid());
        }
        let mut scale = u32::try_from(frac.len()).map_err(|_| invalid())?;
        if scale > 38 {
            return Err(ErrorFields::error(
                "0A000",
                "catalog numeric scale exceeds 38",
            ));
        }
        let digits = format!("{whole}{frac}");
        let mut unscaled = digits
            .parse::<i128>()
            .map_err(|_| ErrorFields::error("0A000", "catalog numeric exceeds 38 digits"))?;
        if negative {
            unscaled = -unscaled;
        }
        while scale > 0 && unscaled % 10 == 0 {
            unscaled /= 10;
            scale -= 1;
        }
        Ok(Self { unscaled, scale })
    }
    pub(super) fn text(&self) -> String {
        let digits = self.unscaled.unsigned_abs().to_string();
        let sign = if self.unscaled < 0 { "-" } else { "" };
        if self.scale == 0 {
            return format!("{sign}{digits}");
        }
        let padded = format!("{digits:0>width$}", width = self.scale as usize + 1);
        let at = padded.len() - self.scale as usize;
        format!("{sign}{}.{}", &padded[..at], &padded[at..])
    }
    pub(super) fn compare(&self, b: &Self) -> std::cmp::Ordering {
        let a = self.text();
        let b = b.text();
        let an = a.starts_with('-');
        let bn = b.starts_with('-');
        if an != bn {
            return if an {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        let (a, b) = (a.trim_start_matches('-'), b.trim_start_matches('-'));
        let (aw, af) = a.split_once('.').unwrap_or((a, ""));
        let (bw, bf) = b.split_once('.').unwrap_or((b, ""));
        let width = af.len().max(bf.len());
        let ord = aw
            .len()
            .cmp(&bw.len())
            .then_with(|| aw.cmp(bw))
            .then_with(|| format!("{af:0<width$}").cmp(&format!("{bf:0<width$}")));
        if an { ord.reverse() } else { ord }
    }
    pub(super) fn round_int(&self) -> Result<i64, ErrorFields> {
        let power = 10_i128
            .checked_pow(self.scale)
            .ok_or_else(|| ErrorFields::error("22003", "numeric out of range"))?;
        let mut n = self.unscaled / power;
        if (self.unscaled % power).unsigned_abs() >= power.unsigned_abs() / 2 && self.scale > 0 {
            n += self.unscaled.signum();
        }
        i64::try_from(n).map_err(|_| ErrorFields::error("22003", "integer out of range"))
    }
    pub(super) fn negative(&self) -> Self {
        Self {
            unscaled: -self.unscaled,
            scale: self.scale,
        }
    }
}
