// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use regex::Regex;
use std::sync::LazyLock;

use crate::models::LineNumber;

use super::DetectionConfig;
use super::host::is_good_email_domain;
use super::junk_data::classify_email;

#[derive(Debug, Clone, PartialEq)]
pub struct EmailDetection {
    pub email: String,
    pub start_line: LineNumber,
    pub end_line: LineNumber,
}

static EMAILS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[A-Z0-9._%-]+@[A-Z0-9.-]+\.[A-Z]{2,63}\b").expect("valid email regex")
});

pub fn find_emails(text: &str, config: &DetectionConfig) -> Vec<EmailDetection> {
    let mut detections = Vec::new();

    for (line_index, line) in text.lines().enumerate() {
        let line_number = LineNumber::from_0_indexed(line_index);
        let normalized_line = line.replace("\\r\\n", "\\n").replace("\\r", "\\n");
        for segment in normalized_line.split("\\n") {
            for matched in EMAILS_REGEX.find_iter(segment) {
                // Skip SSH remotes such as `git@github.com:org/repo.git` and
                // `user@host:port` URL authorities. Only a `:` followed by a
                // path segment (contains `/`) or a bare numeric port qualifies —
                // so structured text like `owner=admin@corp.com:active` keeps its
                // email.
                if let Some(after_colon) = segment[matched.end()..].strip_prefix(':') {
                    let suffix = after_colon.split_whitespace().next().unwrap_or("");
                    let is_ssh_path = suffix.contains('/');
                    let is_port = !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit());
                    if is_ssh_path || is_port {
                        continue;
                    }
                }
                let email = matched.as_str().to_lowercase();
                if !is_good_email_domain(&email) {
                    continue;
                }
                if !classify_email(&email) {
                    continue;
                }

                detections.push(EmailDetection {
                    email,
                    start_line: line_number,
                    end_line: line_number,
                });
            }
        }
    }

    let mut detections = if config.unique {
        let mut seen = std::collections::HashSet::<String>::new();
        detections
            .into_iter()
            .filter(|d| seen.insert(d.email.clone()))
            .collect::<Vec<_>>()
    } else {
        detections
    };

    if config.max_emails > 0 && detections.len() > config.max_emails {
        let mut seen = std::collections::HashSet::<String>::new();
        detections.retain(|d| seen.insert(d.email.clone()));
        detections.truncate(config.max_emails);
    }

    detections
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emails(text: &str) -> Vec<String> {
        find_emails(text, &DetectionConfig::default())
            .into_iter()
            .map(|d| d.email)
            .collect()
    }

    #[test]
    fn test_find_emails_skips_ssh_remote_and_authority() {
        // `git@github.com:org/repo.git` SSH remote (colon + path) is not an email.
        assert!(emails("clone: git clone git@github.com:tonsky/FiraCode.git").is_empty());
        // `user@host:port` URL authority (colon + numeric port) is not an email.
        assert!(emails("connect admin@dbhost.io:5432 now").is_empty());
        // A real email followed by a colon + non-path text is still captured.
        assert_eq!(
            emails("owner=admin@corp.com:active"),
            vec!["admin@corp.com"]
        );
        // A real email followed by a colon and a space is still captured.
        assert_eq!(
            emails("Contact real@corp.com: for help"),
            vec!["real@corp.com"]
        );
        // Plain email untouched.
        assert_eq!(
            emails("mail jane@realcorp.io today"),
            vec!["jane@realcorp.io"]
        );
    }
}
