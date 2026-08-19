//! On-disk size values accepted by `--min-size` and `wipe.minsize`.
//!
//! Deliberately a hand-rolled parser rather than a dependency: git-wipe needs
//! exactly one size knob, with a small, predictable grammar and an error
//! message that names the accepted units. Mirrors [`crate::duration::MinAge`].

use std::fmt;
use std::str::FromStr;

use anyhow::{Result, anyhow};

const KB: u64 = 1024;
const MB: u64 = KB * 1024;
const GB: u64 = MB * 1024;

/// The minimum on-disk size a worktree must have before git-wipe will treat
/// it as a removal or listing candidate.
///
/// The default is zero, which disables the guard/filter entirely and
/// preserves the behaviour git-wipe had before the option existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Size(u64);

impl Size {
    /// The underlying byte count.
    pub fn as_bytes(self) -> u64 {
        self.0
    }

    /// Whether the guard/filter is disabled.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl FromStr for Size {
    type Err = anyhow::Error;

    /// Parse `<number><unit>`, with `unit` in `B`, `K`, `M` or `G` (binary,
    /// 1024-based).
    ///
    /// The unit may be omitted only for a bare `0`, so that "no guard" can be
    /// written the obvious way.
    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        let invalid = || anyhow!("invalid size {s:?}, expected e.g. 0, 512B, 100K, 100M or 2G");

        let (digits, unit) = trimmed.split_at(
            trimmed
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(trimmed.len()),
        );

        let value: u64 = digits.parse().map_err(|_| invalid())?;

        let multiplier = match unit {
            "B" => 1,
            "K" => KB,
            "M" => MB,
            "G" => GB,
            // A bare number is only meaningful when it is zero: every unit
            // agrees on what "0" means, so there is nothing to guess.
            "" if value == 0 => 1,
            _ => return Err(invalid()),
        };

        let bytes = value.checked_mul(multiplier).ok_or_else(invalid)?;
        Ok(Self(bytes))
    }
}

/// Rendered in the largest unit that divides the value exactly, so the output
/// always re-parses to the same size (though not necessarily to the same
/// spelling: `1024K` is displayed as `1M`). `B` always divides exactly, so
/// this never falls back to a bare number.
impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0;
        if bytes == 0 {
            return write!(f, "0B");
        }
        for (unit_bytes, suffix) in [(GB, 'G'), (MB, 'M'), (KB, 'K')] {
            if bytes.is_multiple_of(unit_bytes) {
                return write!(f, "{}{suffix}", bytes / unit_bytes);
            }
        }
        write!(f, "{bytes}B")
    }
}

/// Serialized as its canonical string form, matching `--min-size` and
/// `wipe.minsize`.
impl serde::Serialize for Size {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Render `bytes` as a human-friendly, one-decimal value in the largest
/// binary unit it reaches (`"160.7M"`, `"2.8G"`, `"512B"`).
///
/// Purely a display helper for the worktree multiselect, the `status` SIZE
/// column and the "freed" summary line: unlike [`Size`]'s own `Display`, this
/// never needs to re-parse, so it favours readability over exactness.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(GB, "G"), (MB, "M"), (KB, "K")];
    for (unit_bytes, suffix) in UNITS {
        if bytes >= unit_bytes {
            return format!("{:.1}{suffix}", bytes as f64 / unit_bytes as f64);
        }
    }
    format!("{bytes}B")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_unit() {
        let cases = [
            ("0", 0u64),
            ("0B", 0),
            ("512B", 512),
            ("1K", 1024),
            ("100K", 100 * 1024),
            ("100M", 100 * 1024 * 1024),
            ("2G", 2 * 1024 * 1024 * 1024),
        ];
        for (input, expected) in cases {
            let parsed: Size = input.parse().unwrap();
            assert_eq!(parsed.as_bytes(), expected, "parsing {input:?}");
        }
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!("  2G  ".parse::<Size>().unwrap().as_bytes(), 2 * GB);
    }

    #[test]
    fn rejects_invalid_input() {
        for input in ["", "abc", "5x", "-1M", "1.5M", "M", "2 M", "10", "1MM"] {
            assert!(
                input.parse::<Size>().is_err(),
                "expected {input:?} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_overflow() {
        assert!(format!("{}G", u64::MAX).parse::<Size>().is_err());
    }

    #[test]
    fn display_round_trips() {
        for input in ["0", "0B", "512B", "1K", "100K", "100M", "2G", "1536K"] {
            let parsed: Size = input.parse().unwrap();
            assert_eq!(
                parsed.to_string().parse::<Size>().unwrap(),
                parsed,
                "displaying {input:?} must re-parse to the same size"
            );
        }
    }

    #[test]
    fn display_picks_the_largest_exact_unit() {
        assert_eq!("1024B".parse::<Size>().unwrap().to_string(), "1K");
        assert_eq!("1536B".parse::<Size>().unwrap().to_string(), "1536B");
        assert_eq!("1024K".parse::<Size>().unwrap().to_string(), "1M");
    }

    #[test]
    fn default_is_zero() {
        assert!(Size::default().is_zero());
        assert_eq!(Size::default().to_string(), "0B");
    }

    #[test]
    fn serializes_as_a_string() {
        let json = serde_json::to_string(&"2G".parse::<Size>().unwrap()).unwrap();
        assert_eq!(json, "\"2G\"");
    }

    #[test]
    fn format_bytes_renders_bytes_below_a_kibibyte() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1023), "1023B");
    }

    #[test]
    fn format_bytes_renders_one_decimal_in_the_largest_unit() {
        assert_eq!(format_bytes(1024), "1.0K");
        assert_eq!(format_bytes(164_659), "160.8K");
        assert_eq!(format_bytes(160 * 1024 * 1024 + 700 * 1024), "160.7M");
        assert_eq!(format_bytes(2 * GB + GB * 4 / 10), "2.4G");
    }
}
