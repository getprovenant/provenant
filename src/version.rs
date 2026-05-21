// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::sync::OnceLock;

pub const BUILD_VERSION: &str = match option_env!("PROVENANT_BUILD_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

const ATTRIBUTION_NOTICE: &str = "Independent project; not affiliated with, endorsed by, or sponsored by ScanCode Toolkit, AboutCode, or nexB Inc. License detection uses data from ScanCode Toolkit (CC-BY-4.0). See NOTICE file or the show-attribution command.";

pub fn build_long_version() -> &'static str {
    static LONG_VERSION: OnceLock<String> = OnceLock::new();

    LONG_VERSION
        .get_or_init(|| format!("{BUILD_VERSION}\n{ATTRIBUTION_NOTICE}"))
        .as_str()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_long_version_mentions_independence_and_attribution() {
        let version = super::build_long_version();

        assert!(version.contains("Independent project"));
        assert!(version.contains("ScanCode Toolkit (CC-BY-4.0)"));
    }
}
