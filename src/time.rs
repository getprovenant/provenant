// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use jiff::Timestamp;

pub(crate) fn format_scancode_timestamp(timestamp: &Timestamp) -> String {
    timestamp.strftime("%Y-%m-%dT%H%M%S.%6f").to_string()
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use super::format_scancode_timestamp;

    #[test]
    fn format_scancode_timestamp_uses_compact_microsecond_precision() {
        let timestamp = "2026-04-11T09:18:28.024390124Z"
            .parse::<Timestamp>()
            .expect("valid timestamp");

        assert_eq!(
            format_scancode_timestamp(&timestamp),
            "2026-04-11T091828.024390"
        );
    }
}
