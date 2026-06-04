// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::sync::OnceLock;

pub const BUILD_VERSION: &str = match option_env!("PROVENANT_BUILD_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

const ATTRIBUTION_NOTICE: &str = "Not affiliated with, endorsed by, or sponsored by ScanCode Toolkit, AboutCode, or nexB Inc. Provenant builds on ScanCode Toolkit: it reuses ScanCode license data (CC-BY-4.0) and includes code derived from ScanCode (Apache-2.0). See NOTICE file or the show-attribution command.";

pub fn build_long_version() -> &'static str {
    static LONG_VERSION: OnceLock<String> = OnceLock::new();

    LONG_VERSION
        .get_or_init(|| format!("{BUILD_VERSION}\n{ATTRIBUTION_NOTICE}"))
        .as_str()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_long_version_mentions_affiliation_and_attribution() {
        let version = super::build_long_version();

        assert!(version.contains("Not affiliated with"));
        assert!(version.contains("code derived from ScanCode"));
        assert!(version.contains("ScanCode license data (CC-BY-4.0)"));
    }
}
