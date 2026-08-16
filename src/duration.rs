//! Duration values accepted by `--min-age` and `wipe.minage`.
//!
//! Deliberately a hand-rolled parser rather than a dependency: git-wipe needs
//! exactly one duration knob, with a small, predictable grammar and an error
//! message that names the accepted units.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Result, anyhow};

const SECOND: u64 = 1;
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

/// The minimum age a worktree must have before git-wipe will remove it.
///
/// The default is zero, which disables the guard entirely and preserves the
/// behaviour git-wipe had before the option existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct MinAge(Duration);

impl MinAge {
    /// The underlying [`Duration`].
    pub fn as_duration(self) -> Duration {
        self.0
    }

    /// Whether the guard is disabled.
    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }
}

impl FromStr for MinAge {
    type Err = anyhow::Error;

    /// Parse `<number><unit>`, with `unit` in `s`, `m`, `h`, `d` or `w`.
    ///
    /// The unit may be omitted only for a bare `0`, so that "no guard" can be
    /// written the obvious way.
    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        let invalid = || anyhow!("invalid duration {s:?}, expected e.g. 0, 30s, 15m, 2h, 7d or 1w");

        let (digits, unit) = trimmed.split_at(
            trimmed
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(trimmed.len()),
        );

        let value: u64 = digits.parse().map_err(|_| invalid())?;

        let multiplier = match unit {
            "s" => SECOND,
            "m" => MINUTE,
            "h" => HOUR,
            "d" => DAY,
            "w" => WEEK,
            // A bare number is only meaningful when it is zero: every unit
            // agrees on what "0" means, so there is nothing to guess.
            "" if value == 0 => SECOND,
            _ => return Err(invalid()),
        };

        let seconds = value.checked_mul(multiplier).ok_or_else(invalid)?;
        Ok(Self(Duration::from_secs(seconds)))
    }
}

/// Rendered in the largest unit that divides the value exactly, so the output
/// always re-parses to the same duration (though not necessarily to the same
/// spelling: `7d` is displayed as `1w`).
impl fmt::Display for MinAge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();
        if secs == 0 {
            return write!(f, "0s");
        }
        for (unit_secs, suffix) in [(WEEK, 'w'), (DAY, 'd'), (HOUR, 'h'), (MINUTE, 'm')] {
            if secs.is_multiple_of(unit_secs) {
                return write!(f, "{}{suffix}", secs / unit_secs);
            }
        }
        write!(f, "{secs}s")
    }
}

/// Serialized as its canonical string form, matching `--min-age` and
/// `wipe.minage`.
impl serde::Serialize for MinAge {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_unit() {
        let cases = [
            ("0", 0u64),
            ("0s", 0),
            ("30s", 30),
            ("15m", 15 * 60),
            ("2h", 2 * 3600),
            ("7d", 7 * 86400),
            ("1w", 604800),
        ];
        for (input, expected) in cases {
            let parsed: MinAge = input.parse().unwrap();
            assert_eq!(
                parsed.as_duration().as_secs(),
                expected,
                "parsing {input:?}"
            );
        }
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            "  2h  ".parse::<MinAge>().unwrap().as_duration().as_secs(),
            7200
        );
    }

    #[test]
    fn rejects_invalid_input() {
        for input in ["", "abc", "5x", "-1h", "1.5h", "h", "2 h", "10", "1hh"] {
            assert!(
                input.parse::<MinAge>().is_err(),
                "expected {input:?} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_overflow() {
        assert!(format!("{}w", u64::MAX).parse::<MinAge>().is_err());
    }

    #[test]
    fn display_round_trips() {
        for input in ["0", "0s", "30s", "90s", "15m", "2h", "7d", "1w", "36h"] {
            let parsed: MinAge = input.parse().unwrap();
            assert_eq!(
                parsed.to_string().parse::<MinAge>().unwrap(),
                parsed,
                "displaying {input:?} must re-parse to the same duration"
            );
        }
    }

    #[test]
    fn display_picks_the_largest_exact_unit() {
        assert_eq!("60s".parse::<MinAge>().unwrap().to_string(), "1m");
        assert_eq!("90s".parse::<MinAge>().unwrap().to_string(), "90s");
        assert_eq!("24h".parse::<MinAge>().unwrap().to_string(), "1d");
    }

    #[test]
    fn default_is_zero() {
        assert!(MinAge::default().is_zero());
        assert_eq!(MinAge::default().to_string(), "0s");
    }

    #[test]
    fn serializes_as_a_string() {
        let json = serde_json::to_string(&"2h".parse::<MinAge>().unwrap()).unwrap();
        assert_eq!(json, "\"2h\"");
    }
}
