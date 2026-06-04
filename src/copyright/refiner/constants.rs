// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Constant lookup sets shared by copyright, holder, and author refinement.
//!
//! Prefix/suffix strip tables and known false-positive junk strings.

use std::collections::HashSet;
use std::sync::LazyLock;

// ─── Constant sets ───────────────────────────────────────────────────────────

/// Generic prefixes stripped from names (holders/authors).
pub(super) const PREFIXES: &[&str] = &[
    "?",
    "??",
    "????",
    "(insert",
    "then",
    "current",
    "year)",
    "maintained",
    "by",
    "developed",
    "created",
    "written",
    "recoded",
    "coded",
    "modified",
    // Note: Python has 'maintained''created' (missing comma = concatenation).
    // We include both separately.
    "maintainedcreated",
    "$year",
    "year",
    "uref",
    "owner",
    "from",
    "and",
    "of",
    "to",
    "for",
    "or",
    "<p>",
];

/// Suffixes stripped from copyright strings.
pub(super) static COPYRIGHTS_SUFFIXES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "copyright",
        ".",
        ",",
        "year",
        "parts",
        "any",
        "0",
        "1",
        "author",
        "all",
        "some",
        "and",
        "</p>",
        "is",
        "-",
        "distributed",
        "information",
        "credited",
        "by",
    ]
    .into_iter()
    .collect()
});

/// Authors prefixes = PREFIXES ∪ author-specific words.
pub(super) static AUTHORS_PREFIXES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut s: HashSet<&str> = PREFIXES.iter().copied().collect();
    for w in &[
        "contributor",
        "contributor(s)",
        "authors",
        "author",
        "authors'",
        "author:",
        "author(s)",
        "authored",
        "created",
        "author.",
        "author'",
        "authors,",
        "authorship",
        "maintainer",
        "co-maintainer",
        "or",
        "spdx-filecontributor",
        "</b>",
        "mailto:",
        "name'",
        "a",
        "moduleauthor",
        "\u{a9}", // ©
    ] {
        s.insert(w);
    }
    s
});

/// Authors junk — detected author strings that are false positives.
pub(super) static AUTHORS_JUNK: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "james hacker.",
        "james random hacker.",
        "contributor. c. a",
        "grant the u.s. government and others",
        "james random hacker",
        "james hacker",
        "company",
        "contributing project",
        "its author",
        "gnomovision",
        "would",
        "may",
        "attributions",
        "the",
        "app id",
        "homepage",
        "repository",
        "documentation",
        "package author",
        "package authors",
        "project",
        "previous lucene",
        "group",
        "the coordinator",
        "the owner",
        "a group",
        "sonatype nexus",
        "apache tomcat",
        "visual studio",
        "apache maven",
        "visual studio and visual studio",
        "work",
        "additional",
        "builder",
        "chef-client",
        "compatible",
        "guice",
        "incorporated",
        "guide",
        "grants",
        "recommend",
        "recheck",
        "reputations",
        "review",
        "reviewer",
        "document",
        "otherwise",
        "disclaims",
        "liability",
        "required",
        "desired",
        "intended",
        "someone",
        "performing",
        "volunteer",
        "volunteers",
        "automatically generated",
        "donald becker",
    ]
    .into_iter()
    .collect()
});

/// Prefix that triggers ignoring the author entirely.
pub(super) const AUTHORS_JUNK_PREFIX: &str = "httpProxy";

/// Holders prefixes = PREFIXES ∪ holder-specific words.
pub(super) static HOLDERS_PREFIXES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut s: HashSet<&str> = PREFIXES.iter().copied().collect();
    for w in &[
        "-",
        "a",
        "<a",
        "href",
        "ou",
        "portions",
        "portion",
        "notice",
        "holders",
        "holder",
        "property",
        "parts",
        "part",
        "at",
        "cppyright",
        "assemblycopyright",
        "c",
        "works",
        "present",
        "right",
        "rights",
        "reserved",
        "held",
        "is",
        "(x)",
        "later",
        "$",
        "current.year",
        "\u{a9}", // ©
        "author",
        "authors",
    ] {
        s.insert(w);
    }
    s
});

/// Holders prefixes including "all" (used when "reserved" is in the string).
pub(super) static HOLDERS_PREFIXES_WITH_ALL: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| {
        let mut s = HOLDERS_PREFIXES.clone();
        s.insert("all");
        s
    });

/// Suffixes stripped from holder strings.
pub(super) static HOLDERS_SUFFIXES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "http",
        "and",
        "email",
        "licensing@",
        "(minizip)",
        "website",
        "(c)",
        "<http",
        "/>",
        ".",
        ",",
        "year",
        "some",
        "all",
        "right",
        "rights",
        "reserved",
        "reserved.",
        "href",
        "c",
        "a",
        "</p>",
        "or",
        "taken",
        "from",
        "is",
        "-",
        "distributed",
        "information",
        "credited",
        "$",
    ]
    .into_iter()
    .collect()
});

/// Holders junk — detected holder strings that are false positives.
pub(super) static HOLDERS_JUNK: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "a href",
        "property",
        "copyright",
        "licensing@",
        "c",
        "works",
        "http",
        "the",
        "are",
        "?",
        "cppyright",
        "parts",
        "disclaimed",
        "or",
        "<holders>",
        "author",
        // License boilerplate false positives
        "holders",
        "holder",
        "holder,",
        "and/or",
        "if",
        "grant",
        "notice",
        "header",
        "comment",
        "do the following",
        "does",
        "has",
        "each",
        "also",
        "in",
        "simply",
        "other",
        "shall",
        "said",
        "who",
        "your",
        "their",
        "ensure",
        "allow",
        "terms",
        "conditions",
        "information",
        "contributors",
        "contributors as",
        "contributors and the university",
        "indemnification",
        "license",
        "claimed",
        "but",
        "agrees",
        "patent",
        "owner",
        "owners",
        "yyyy",
        "expressly",
        "stating",
        "enforce",
        "d",
        "ss",
        // Additional single-word junk
        "given",
        "may",
        "every",
        "no",
        "good",
        "row",
        "logo",
        "flag",
        "updated",
        "law",
        "england",
        "tm",
        "pgp",
        "distributed",
        "as",
        "null",
        "psy",
        "object",
        "indicate the origin and nature of",
        "statements",
        "protection",
        "(if any) with",
        "if any with",
        // Short gibberish from binary data
        "ga",
        "ka",
        "aa",
        "qa",
        "yx",
        "ac",
        "ae",
        "gn",
        "cb",
        "ib",
        "qb",
        "py",
        "pu",
        "ce",
        "nmd",
        "a1",
        "deg",
        "gnu",
        "with",
        "yy",
        "c/",
        "messages",
        "licenses",
        "not limited",
        "charge",
        "case 2",
        "dot",
        "public",
        // C function/macro names from ICS false positives
        "width",
        "len",
        "do",
        "date",
        "year",
        "note",
        "update",
        "info",
        "notices",
        "duplicated",
        "register",
        // C identifier/keyword false positives from ICS
        "isascii",
        "iscntrl",
        "isprint",
        "isdigit",
        "isalpha",
        "toupper",
        "yyunput",
        "ambiguous",
        "indir",
        "notive",
        "strict",
        "decoded",
        "unsigned",
        // Short numbers/tokens from code
        "0 1",
        "8",
        "9",
        "16",
        "24",
        "4",
        // More boilerplate/legal words
        "notices all the files",
        "may not be removed or altered",
        "duplicated in",
        "mjander",
        "3dfx",
        "related",
    ]
    .into_iter()
    .collect()
});
