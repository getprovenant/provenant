// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use regex::Regex;
use std::sync::LazyLock;

use url::Url;

use crate::models::LineNumber;

use super::DetectionConfig;
use super::host::is_good_url_host_domain;
use super::junk_data::classify_url;

#[derive(Debug, Clone, PartialEq)]
pub struct UrlDetection {
    pub url: String,
    pub start_line: LineNumber,
    pub end_line: LineNumber,
}

static URLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?ix)
        (
            (?:https?|ftps?|sftp|rsync|ssh|svn|git|hg|https?\+git|https?\+svn|https?\+hg)://[^\s<>\[\]"]+
            |
            (?:www|ftp)\.[^\s<>\[\]"]+
            |
            git\@[^\s<>\[\]"]+:[^\s<>\[\]"]+\.git
        )
        "#,
    )
    .expect("valid url regex")
});

static INVALID_URLS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:https?|ftps?|sftp|rsync|ssh|svn|git|hg|https?\+git|https?\+svn|https?\+hg)://(?:[$%*/_])+$")
        .expect("valid invalid-url regex")
});

const EMPTY_URLS: &[&str] = &["https", "http", "ftp", "www"];

fn is_filterable(url: &str) -> bool {
    !url.starts_with("git@")
}

fn verbatim_crlf_url_cleaner(url: &str) -> String {
    url.to_string()
}

fn end_of_url_cleaner(url: &str) -> String {
    let mut cleaned = if url.ends_with('/') {
        url.to_string()
    } else {
        url.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    };

    for marker in ['\\', '<', '>', '(', ')', '[', ']', '"', '\'', '`'] {
        if let Some((before, _)) = cleaned.split_once(marker) {
            cleaned = before.to_string();
        }
    }

    if let Some((before, after)) = cleaned.split_once('}')
        && !has_unclosed_dollar_template(before)
        && ({
            let trimmed_after =
                after.trim_matches(|c: char| [',', '.', ':', ';', '!', '?'].contains(&c));
            trimmed_after.is_empty()
                || trimmed_after.chars().all(|ch| ch == '}')
                || trimmed_after.starts_with('{')
        })
    {
        cleaned = before.to_string();
    }

    cleaned = trim_trailing_template_openers(&cleaned);
    cleaned = cleaned.trim_end_matches('*').to_string();

    cleaned
        .trim_end_matches(|c: char| [',', '.', ':', ';', '!', '?'].contains(&c))
        .to_string()
}

fn has_unclosed_dollar_template(url: &str) -> bool {
    url.rfind("${")
        .is_some_and(|idx| !url[idx + 2..].contains('}'))
}

fn trim_trailing_template_openers(url: &str) -> String {
    let mut cleaned = url.to_string();

    for opener in ["${{", "${"] {
        if cleaned.ends_with(opener) {
            cleaned.truncate(cleaned.len() - opener.len());
            cleaned = cleaned.trim_end_matches('/').to_string();
            break;
        }
    }

    cleaned
}

fn add_fake_scheme(url: &str) -> String {
    if is_filterable(url) && !url.contains("://") && !url.contains('@') {
        format!("http://{url}")
    } else {
        url.to_string()
    }
}

fn is_bare_ftp_method_reference(candidate: &str) -> bool {
    let candidate = candidate
        .to_ascii_lowercase()
        .trim_end_matches(|c: char| [',', '.', ':', ';', '!', '?'].contains(&c))
        .to_string();
    if !candidate.starts_with("ftp.") {
        return false;
    }

    if !candidate.contains('/') && candidate.matches('.').count() == 1 {
        return true;
    }

    let Some(paren_index) = candidate.find('(') else {
        return false;
    };

    !candidate[..paren_index].contains('/')
}

fn remove_user_password(url: &str) -> Option<String> {
    if !is_filterable(url) {
        return Some(url.to_string());
    }

    if let Ok(mut parsed) = Url::parse(url) {
        parsed.set_username("").ok()?;
        parsed.set_password(None).ok()?;
        parsed.host_str()?;
        return Some(parsed.to_string());
    }

    strip_manual_userinfo(url).or_else(|| Some(url.to_string()))
}

fn strip_manual_userinfo(url: &str) -> Option<String> {
    let scheme_end = url.find("://")?;
    let authority_and_rest = &url[scheme_end + 3..];
    let authority_end = authority_and_rest
        .find(['/', '?', '#'])
        .unwrap_or(authority_and_rest.len());
    let authority = &authority_and_rest[..authority_end];
    let at_index = authority.rfind('@')?;

    let mut rebuilt = String::with_capacity(url.len());
    rebuilt.push_str(&url[..scheme_end + 3]);
    rebuilt.push_str(&authority[at_index + 1..]);
    rebuilt.push_str(&authority_and_rest[authority_end..]);
    Some(rebuilt)
}

fn canonical_url(url: &str) -> Option<String> {
    if !is_filterable(url) {
        return Some(url.to_string());
    }
    Url::parse(url)
        .ok()
        .map(|parsed| parsed.to_string())
        .or_else(|| canonical_template_host_url(url))
}

fn canonical_template_host_url(url: &str) -> Option<String> {
    let scheme_end = url.find("://")?;
    let scheme = &url[..scheme_end];
    if !matches!(scheme, "http" | "https") {
        return None;
    }

    let after_scheme = &url[scheme_end + 3..];
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if !host.contains("%s") || !looks_like_template_host(host) {
        return None;
    }

    let mut canonical = url.to_string();
    if authority_end == after_scheme.len() {
        canonical.push('/');
    }
    Some(canonical)
}

fn looks_like_template_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 3 || labels[labels.len() - 2..].contains(&"%s") {
        return false;
    }
    if !labels
        .iter()
        .any(|label| *label != "%s" && !label.is_empty())
    {
        return false;
    }

    let concrete = host.replace("%s", "template");
    is_good_url_host_domain(&format!("http://{concrete}"))
}

pub fn find_urls(text: &str, config: &DetectionConfig) -> Vec<UrlDetection> {
    let mut detections = Vec::new();

    for (line_index, line) in text.lines().enumerate() {
        let line_number = LineNumber::from_0_indexed(line_index);
        let normalized_line = line.replace("\\r\\n", "\\n").replace("\\r", "\\n");

        for segment in normalized_line.split("\\n") {
            for matched in URLS_REGEX.find_iter(segment) {
                if matched.start() > 0 {
                    let prev_byte = segment.as_bytes()[matched.start() - 1];
                    if prev_byte.is_ascii_alphanumeric()
                        && !matched.as_str().starts_with("http://")
                        && !matched.as_str().starts_with("https://")
                    {
                        continue;
                    }
                }

                let mut candidate = matched.as_str().to_string();

                if is_bare_ftp_method_reference(&candidate) {
                    continue;
                }

                candidate = verbatim_crlf_url_cleaner(&candidate);
                candidate = end_of_url_cleaner(&candidate);

                let candidate_lower = candidate.to_ascii_lowercase();
                if candidate.is_empty() || EMPTY_URLS.contains(&candidate_lower.as_str()) {
                    continue;
                }

                candidate = add_fake_scheme(&candidate);

                let Some(candidate) = remove_user_password(&candidate) else {
                    continue;
                };
                if INVALID_URLS_PATTERN.is_match(&candidate) {
                    continue;
                }

                let Some(candidate) = canonical_url(&candidate) else {
                    continue;
                };

                if is_filterable(&candidate)
                    && !is_good_url_host_domain(&candidate)
                    && !canonical_template_host_url(&candidate)
                        .is_some_and(|url| looks_like_template_url_host_domain(&url))
                {
                    continue;
                }
                if !classify_url(&candidate.to_ascii_lowercase()) {
                    continue;
                }

                detections.push(UrlDetection {
                    url: candidate,
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
            .filter(|d| seen.insert(d.url.clone()))
            .collect::<Vec<_>>()
    } else {
        detections
    };

    if config.max_urls > 0 && detections.len() > config.max_urls {
        detections.sort_by_key(|detection| is_low_priority_url(&detection.url));
        detections.truncate(config.max_urls);
    }

    detections
}

fn is_low_priority_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let path = parsed.path().to_ascii_lowercase();
    [
        ".apng", ".avif", ".bmp", ".gif", ".ico", ".jpeg", ".jpg", ".png", ".svg", ".webp",
    ]
    .iter()
    .any(|suffix| path.ends_with(suffix))
}

fn looks_like_template_url_host_domain(url: &str) -> bool {
    let Some(scheme_end) = url.find("://") else {
        return false;
    };
    let after_scheme = &url[scheme_end + 3..];
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    looks_like_template_host(host)
}
