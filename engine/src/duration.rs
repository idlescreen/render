use crate::error::RenderError;
use std::time::Duration;

/// Parse human durations: `10`, `10s`, `5m`, `2h`, `1d` (case-insensitive).
pub fn parse_duration_secs(raw: &str) -> Result<Duration, RenderError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(RenderError::Duration("empty".into()));
    }
    let (num_part, mult) = match s.chars().last() {
        Some(c) if c.is_ascii_alphabetic() => {
            let (n, u) = s.split_at(s.len() - 1);
            let m = match u.to_ascii_lowercase().as_str() {
                "s" => 1u64,
                "m" => 60,
                "h" => 3600,
                "d" => 86400,
                other => {
                    return Err(RenderError::Duration(format!("unknown unit '{other}'")));
                }
            };
            (n, m)
        }
        _ => (s, 1u64),
    };
    let n: u64 = num_part
        .trim()
        .parse()
        .map_err(|_| RenderError::Duration(format!("not a number: {num_part}")))?;
    if n == 0 {
        return Err(RenderError::Duration("duration must be > 0".into()));
    }
    let secs = n
        .checked_mul(mult)
        .ok_or_else(|| RenderError::Duration("duration overflow".into()))?;
    Ok(Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_seconds_suffix() {
        assert_eq!(parse_duration_secs("10s").unwrap(), Duration::from_secs(10));
    }

    #[test]
    fn parses_minutes() {
        assert_eq!(parse_duration_secs("2m").unwrap(), Duration::from_secs(120));
    }

    #[test]
    fn parses_hours() {
        assert_eq!(
            parse_duration_secs("1h").unwrap(),
            Duration::from_secs(3600)
        );
    }

    #[test]
    fn bare_number_is_seconds() {
        assert_eq!(parse_duration_secs("10").unwrap(), Duration::from_secs(10));
    }

    #[test]
    fn rejects_zero() {
        assert!(parse_duration_secs("0s").is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(parse_duration_secs("  ").is_err());
    }
}
