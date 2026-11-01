use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};

pub fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    let trimmed = strip_fraction(raw);
    DateTime::parse_from_rfc3339(&trimmed)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn strip_fraction(raw: &str) -> String {
    if let Some(dot) = raw.find('.') {
        let rest = &raw[dot + 1..];
        let z = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        format!("{}{}", &raw[..dot], &rest[z..])
    } else {
        raw.to_owned()
    }
}

pub fn deserialize_opt_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => Ok(parse_date(&s)),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_date;

    #[test]
    fn parses_laravel_dates() {
        assert!(parse_date("2026-10-24T10:11:12Z").is_some());
        assert_eq!(
            parse_date("2026-10-24T10:11:12.000000Z"),
            parse_date("2026-10-24T10:11:12Z")
        );
    }
}
