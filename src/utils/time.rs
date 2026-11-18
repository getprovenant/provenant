// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use jiff::Timestamp;

const ISO_UTC_TIMESTAMP_FALLBACK: &str = "1970-01-01T00:00:00Z";

pub(crate) fn fallback_iso_utc_timestamp() -> &'static str {
    ISO_UTC_TIMESTAMP_FALLBACK
}

pub(crate) fn convert_header_timestamp_to_iso_utc(value: &str) -> Option<String> {
    let timestamp = value.parse::<Timestamp>().ok().or_else(|| {
        let datetime = jiff::civil::DateTime::strptime("%Y-%m-%dT%H%M%S%.f", value).ok()?;
        Some(datetime.to_zoned(jiff::tz::TimeZone::UTC).ok()?.timestamp())
    })?;
    Some(timestamp.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
}

#[cfg(test)]
mod tests {
    use super::convert_header_timestamp_to_iso_utc;

    #[test]
    fn convert_header_timestamp_to_iso_utc_accepts_scancode_and_rfc3339_inputs() {
        assert_eq!(
            convert_header_timestamp_to_iso_utc("2026-04-11T091828.024390"),
            Some("2026-04-11T09:18:28Z".to_string())
        );
        assert_eq!(
            convert_header_timestamp_to_iso_utc("2026-04-11T09:18:28.024390124+00:00"),
            Some("2026-04-11T09:18:28Z".to_string())
        );
        assert_eq!(
            convert_header_timestamp_to_iso_utc("2026-04-11T09:18:28.024390124Z"),
            Some("2026-04-11T09:18:28Z".to_string())
        );
    }
}
