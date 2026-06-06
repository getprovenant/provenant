// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::{
    extract_comment_author_supplements, extract_copyright_information,
    extract_patch_header_author_supplements, is_binary_string_copyright_candidate,
};
use crate::copyright;
use crate::models::{FileInfoBuilder, FileType};
use std::path::Path;
use std::time::Duration;

#[test]
fn test_binary_string_copyright_candidate_rejects_gibberish_holder_text() {
    let gibberish = "(c) S8@9 K @9 D @9 I,@9N(@ F@@9L,@ HD@9) M0@9s J'@y DH@9Ih@y";
    assert!(!is_binary_string_copyright_candidate(gibberish));
}

#[test]
fn test_binary_string_copyright_candidate_rejects_control_char_gibberish() {
    let gibberish = "(c) K0\u{000e}q6 b$L";
    assert!(!is_binary_string_copyright_candidate(gibberish));
}

#[test]
fn test_binary_string_copyright_candidate_rejects_digit_bearing_gibberish_without_year() {
    let gibberish = "(c) K0 b$L";
    assert!(!is_binary_string_copyright_candidate(gibberish));
}

#[test]
fn test_binary_string_copyright_candidate_keeps_digit_bearing_company_name_without_year() {
    let notice = "Copyright (c) 3Com Corporation";
    assert!(is_binary_string_copyright_candidate(notice));
}

#[test]
fn test_extract_copyright_information_drops_binary_string_gibberish_notice() {
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("fixture.blb"),
        "(c) K0\n b$L",
        120.0,
        true,
    );

    let file = builder
        .name("fixture.blb".to_string())
        .base_name("fixture".to_string())
        .extension(".blb".to_string())
        .path("fixture.blb".to_string())
        .file_type(FileType::File)
        .size(9)
        .build()
        .expect("builder should produce file info");
    assert!(
        file.copyrights.is_empty(),
        "copyrights: {:?}",
        file.copyrights
    );
}

#[test]
fn test_extract_copyright_information_preserves_raw_text_and_normalized_shadow() {
    let text = "/* Copyright 2024 Example Corp. All rights reserved. */\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("fixture.c"), text, 120.0, false);

    let file = builder
        .name("fixture.c".to_string())
        .base_name("fixture".to_string())
        .extension(".c".to_string())
        .path("fixture.c".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(file.copyrights.len(), 1);
    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright 2024 Example Corp. All rights reserved."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("Copyright 2024 Example Corp.")
    );
}

#[test]
fn test_extract_copyright_information_keeps_raw_notice_and_holder_for_no_year_c_symbol() {
    let text = "// Copyright (c) ATO Gear. All rights reserved.\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("RNBackgroundTimer.h"),
        text,
        120.0,
        false,
    );

    let file = builder
        .name("RNBackgroundTimer.h".to_string())
        .base_name("RNBackgroundTimer".to_string())
        .extension(".h".to_string())
        .path("RNBackgroundTimer.h".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright (c) ATO Gear. All rights reserved."
    );
    assert_eq!(file.holders.len(), 1, "holders: {:?}", file.holders);
    assert_eq!(file.holders[0].holder, "ATO Gear");
}

#[test]
fn test_extract_copyright_information_uses_embedded_sourcemap_sources_for_parties() {
    let text = r#"{"version":3,"comment":"Copyright 1999 Wrong Corp.","sourcesContent":["/* Copyright 2024 Example Corp. */\n"]}"#;
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("bundle.js.map"), text, 120.0, false);

    let file = builder
        .name("bundle.js.map".to_string())
        .base_name("bundle.js".to_string())
        .extension(".map".to_string())
        .path("bundle.js.map".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(file.copyrights[0].copyright, "Copyright 2024 Example Corp.");
    assert_eq!(file.holders.len(), 1, "holders: {:?}", file.holders);
    assert_eq!(file.holders[0].holder, "Example Corp.");
    assert!(
        file.copyrights
            .iter()
            .all(|copyright| !copyright.copyright.contains("Wrong Corp"))
    );
    assert!(
        file.holders
            .iter()
            .all(|holder| !holder.holder.contains("Wrong Corp"))
    );
}

#[test]
fn test_extract_copyright_information_multiline_native_projection_avoids_comment_wrappers() {
    let text = "/*\n * Copyright 2024 Example Corp.\n * All rights reserved.\n */\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("fixture.c"), text, 120.0, false);

    let file = builder
        .name("fixture.c".to_string())
        .base_name("fixture".to_string())
        .extension(".c".to_string())
        .path("fixture.c".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(file.copyrights.len(), 1);
    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright 2024 Example Corp. All rights reserved."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("Copyright 2024 Example Corp.")
    );
}

#[test]
fn test_extract_copyright_information_xml_comment_projection_avoids_comment_wrappers() {
    let text = "<!-- (c) Example Corp. and affiliates. Confidential and proprietary. -->\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("fixture.xml"), text, 120.0, false);

    let file = builder
        .name("fixture.xml".to_string())
        .base_name("fixture".to_string())
        .extension(".xml".to_string())
        .path("fixture.xml".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(
        file.copyrights[0].copyright,
        "(c) Example Corp. and affiliates. Confidential and proprietary."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("(c) Example Corp. and affiliates. Confidential and proprietary")
    );
    assert_eq!(file.holders.len(), 1, "holders: {:?}", file.holders);
    assert_eq!(file.holders[0].holder, "Example Corp. and affiliates");
}

#[test]
fn test_extract_copyright_information_js_block_comment_lowercase_c_header() {
    let text = "/**\n * (c) foo platforms, inc. and affiliates. confidential and proprietary.\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("fixture.js"), text, 120.0, false);

    let file = builder
        .name("fixture.js".to_string())
        .base_name("fixture".to_string())
        .extension(".js".to_string())
        .path("fixture.js".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(file.holders.len(), 1, "holders: {:?}", file.holders);
    assert_eq!(
        file.copyrights[0].copyright,
        "(c) foo platforms, inc. and affiliates. confidential and proprietary."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("(c) foo platforms, inc. and affiliates")
    );
    assert_eq!(file.holders[0].holder, "foo platforms, inc. and affiliates");
}

#[test]
fn test_extract_copyright_information_xml_comment_projection_preserves_native_symbol() {
    let text = "<!-- Copyright © 2024 Example Corp. All rights reserved. -->\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("fixture.xml"), text, 120.0, false);

    let file = builder
        .name("fixture.xml".to_string())
        .base_name("fixture".to_string())
        .extension(".xml".to_string())
        .path("fixture.xml".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright © 2024 Example Corp. All rights reserved."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("Copyright (c) 2024 Example Corp.")
    );
}

#[test]
fn test_extract_copyright_information_bloomfilter_exact_file_shape_keeps_onelab() {
    let text = "/**
 *
 * Copyright (c) 2005, European Commission project OneLab under contract 034819 (http://www.one-lab.org)
 * All rights reserved.
 * Redistribution and use in source and binary forms, with or 
 * without modification, are permitted provided that the following 
 * conditions are met:
 *  - Redistributions of source code must retain the above copyright 
 *    notice, this list of conditions and the following disclaimer.
 *  - Redistributions in binary form must reproduce the above copyright 
 *    notice, this list of conditions and the following disclaimer in 
 *    the documentation and/or other materials provided with the distribution.
 */

/**
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * \"License\"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 */

package org.apache.hadoop.util.bloom;

/**
 * Originally created by
 * <a href=\"http://www.one-lab.org\">European Commission One-Lab Project 034819</a>.
 */
public class BloomFilter {}
";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("BloomFilter.java"),
        text,
        120.0,
        false,
    );

    let file = builder
        .name("BloomFilter.java".to_string())
        .base_name("BloomFilter".to_string())
        .extension(".java".to_string())
        .path("BloomFilter.java".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(
        file.copyrights.iter().any(|c| {
            c.normalized_copyright.as_deref()
                == Some("Copyright (c) 2005, European Commission project OneLab")
        }),
        "copyrights: {:?}",
        file.copyrights
    );
    assert!(
        file.holders
            .iter()
            .any(|h| h.holder == "European Commission project OneLab"),
        "holders: {:?}",
        file.holders
    );
}

#[test]
fn test_extract_copyright_information_strips_flutter_wrapper_assignments() {
    let text = "PRODUCT_COPYRIGHT = Copyright © 2014 The Flutter Authors. All rights reserved.\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("AppInfo.xcconfig"),
        text,
        120.0,
        false,
    );

    let file = builder
        .name("AppInfo.xcconfig".to_string())
        .base_name("AppInfo".to_string())
        .extension(".xcconfig".to_string())
        .path("AppInfo.xcconfig".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright (c) 2014 The Flutter Authors. All rights reserved."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("Copyright (c) 2014 The Flutter Authors")
    );
}

#[test]
fn test_extract_copyright_information_strips_flutter_application_legalese_wrapper() {
    let text = "applicationLegalese: '© 2014 The Flutter Authors',\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("about.dart"), text, 120.0, false);

    let file = builder
        .name("about.dart".to_string())
        .base_name("about".to_string())
        .extension(".dart".to_string())
        .path("about.dart".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(file.copyrights[0].copyright, "(c) 2014 The Flutter Authors");
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("(c) 2014 The Flutter Authors")
    );
}

#[test]
fn test_extract_copyright_information_strips_flutter_storyboard_text_wrapper() {
    let text = r#"<label text="© 2018 The Flutter Authors. All rights reserved." />\n"#;
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("LaunchScreen.storyboard"),
        text,
        120.0,
        false,
    );

    let file = builder
        .name("LaunchScreen.storyboard".to_string())
        .base_name("LaunchScreen".to_string())
        .extension(".storyboard".to_string())
        .path("LaunchScreen.storyboard".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(
        file.copyrights[0].copyright,
        "(c) 2018 The Flutter Authors. All rights reserved."
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("(c) 2018 The Flutter Authors")
    );
}

#[test]
fn test_extract_copyright_information_drops_flutter_generated_doc_false_positive() {
    let text = r#"<i class="material-icons-sharp md-36">copyright</i> &#x2014; material icon named "copyright" (sharp).\n"#;
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("icons.dart"), text, 120.0, false);

    let file = builder
        .name("icons.dart".to_string())
        .base_name("icons".to_string())
        .extension(".dart".to_string())
        .path("icons.dart".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(
        file.copyrights.is_empty(),
        "copyrights: {:?}",
        file.copyrights
    );
    assert!(file.holders.is_empty(), "holders: {:?}", file.holders);
}

#[test]
fn test_extract_copyright_information_strips_trailing_or_notice_bleed() {
    let text = "Copyright © 1993,2004 Sun Microsystems or\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("NOTICE"), text, 120.0, false);

    let file = builder
        .name("NOTICE".to_string())
        .base_name("NOTICE".to_string())
        .extension("".to_string())
        .path("NOTICE".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright (c) 1993,2004 Sun Microsystems"
    );
}

#[test]
fn test_extract_copyright_information_strips_locale_timestamp_from_raw_projection() {
    let text = "// Copyright (C) EDF R&D, lun sep 30 14:23:19 CEST 2002\n";
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("action_aat_product.hh"),
        text,
        120.0,
        false,
    );

    let file = builder
        .name("action_aat_product.hh".to_string())
        .base_name("action_aat_product".to_string())
        .extension(".hh".to_string())
        .path("action_aat_product.hh".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(file.copyrights[0].copyright, "Copyright (c) EDF R&D 2002");
    assert_eq!(file.holders.len(), 1, "holders: {:?}", file.holders);
    assert_eq!(file.holders[0].holder, "EDF R&D");
}

#[test]
fn test_extract_copyright_information_projects_clean_python_assignment_metadata() {
    let text = concat!(
        "author = \"Pyodide contributors\"\n",
        "copyright = \"2019-2026, Pyodide contributors and Mozilla\"\n",
    );
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(&mut builder, Path::new("docs/conf.py"), text, 120.0, false);

    let file = builder
        .name("conf.py".to_string())
        .base_name("conf".to_string())
        .extension(".py".to_string())
        .path("docs/conf.py".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert_eq!(
        file.copyrights.len(),
        1,
        "copyrights: {:?}",
        file.copyrights
    );
    assert_eq!(
        file.copyrights[0].copyright,
        "Copyright 2019-2026, Pyodide contributors and Mozilla"
    );
    assert_eq!(
        file.copyrights[0].normalized_copyright.as_deref(),
        Some("Copyright 2019-2026, Pyodide contributors and Mozilla")
    );
    assert_eq!(file.holders.len(), 1, "holders: {:?}", file.holders);
    assert_eq!(file.holders[0].holder, "Pyodide contributors and Mozilla");
}

#[test]
fn test_binary_string_copyright_candidate_keeps_real_notice() {
    let notice = "Copyright nexB and others (c) 2012";
    assert!(is_binary_string_copyright_candidate(notice));
}

#[test]
fn test_binary_string_copyright_candidate_rejects_changelog_phrase() {
    assert!(!is_binary_string_copyright_candidate(
        "Copyright - split out libs"
    ));
}

#[test]
fn test_extract_patch_header_author_supplements_collects_common_patch_headers() {
    let text = "From: Robert Scheck <robert@fedoraproject.org>\n\
Signed-off-by: Khem Raj <raj.khem@gmail.com>\n\
Patch by Example Person <example@example.com>\n";

    let authors = extract_patch_header_author_supplements(text);
    let values: Vec<_> = authors.into_iter().map(|author| author.author).collect();

    assert_eq!(
        values,
        vec![
            "Robert Scheck <robert@fedoraproject.org>",
            "Khem Raj <raj.khem@gmail.com>",
            "Example Person <example@example.com>",
        ]
    );
}

#[test]
fn test_extract_comment_author_supplements_collects_written_by_and_email_name_forms() {
    let text = "# udhcpc script edited by Tim Riker <Tim@Rikers.org>\n\
#   clst@ambu.com (Claus Stovgaard)\n\
#                by Ian Murdock <imurdock@gnu.ai.mit.edu>.\n";

    let authors = extract_comment_author_supplements(text);
    let values: Vec<_> = authors.into_iter().map(|author| author.author).collect();

    assert_eq!(
        values,
        vec![
            "Tim Riker <Tim@Rikers.org>",
            "Claus Stovgaard <clst@ambu.com>",
            "Ian Murdock <imurdock@gnu.ai.mit.edu>",
        ]
    );
}

#[test]
fn test_extract_comment_author_supplements_collects_obfuscated_angle_contact_author() {
    let text = "* Author: Deepak M <m.deepak at intel.com>\n";

    let authors = extract_comment_author_supplements(text);
    let values: Vec<_> = authors.into_iter().map(|author| author.author).collect();

    assert_eq!(values, vec!["Deepak M m.deepak at intel.com"]);
}

#[test]
fn test_extract_comment_author_supplements_collects_comment_by_and_docker_maintainer_lines() {
    let text = "# a2enmod by Stefan Fritsch <sf@debian.org>\n\
LABEL maintainer=\"Progress Chef <docker@chef.io>\"\n";

    let authors = extract_comment_author_supplements(text);
    let values: Vec<_> = authors.into_iter().map(|author| author.author).collect();

    assert_eq!(
        values,
        vec![
            "Stefan Fritsch <sf@debian.org>",
            "Progress Chef <docker@chef.io>",
        ]
    );
}

#[test]
fn test_extract_comment_author_supplements_handles_c_style_translator_headers() {
    let text = "/* Translated by Jorge Barreiro <yortx.barry@gmail.com>. */\n\
/* Written by Mathias Bynens <https://mathiasbynens.be/> */\n\
/* Written by Cloudream (cloudream@gmail.com). */\n\
/* Written by S A Sureshkumar (saskumar@live.com). */\n";

    let authors = extract_comment_author_supplements(text);
    let values: Vec<_> = authors.into_iter().map(|author| author.author).collect();

    assert_eq!(
        values,
        vec![
            "Jorge Barreiro <yortx.barry@gmail.com>",
            "Mathias Bynens (https://mathiasbynens.be)",
            "Cloudream (cloudream@gmail.com)",
            "S A Sureshkumar (saskumar@live.com)",
        ]
    );
}

#[test]
fn test_extract_comment_author_supplements_handles_html_comment_by_line() {
    let text = "<!-- Checkstyle XML Style Sheet by Stephane Bailliez <sbailliez@apache.org> -->\n";

    let authors = extract_comment_author_supplements(text);
    let values: Vec<_> = authors.into_iter().map(|author| author.author).collect();

    assert_eq!(values, vec!["Stephane Bailliez <sbailliez@apache.org>"]);
}

#[test]
fn test_extract_comment_author_supplements_ignores_html_tags() {
    let text = "the order defined by the DTD (see Section 13.3).</p>";

    let authors = extract_comment_author_supplements(text);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_extract_comment_author_supplements_ignores_plain_markdown_prose() {
    let text =
        "Support this project by [becoming a sponsor](https://opencollective.com/pnpm#sponsor).";

    let authors = extract_comment_author_supplements(text);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_extract_copyright_information_ignores_pnpm_markdown_link_prose() {
    let text = concat!(
        "</table>\n\n",
        "<!-- sponsors end -->\n\n",
        "Support this project by [becoming a sponsor](https://opencollective.com/pnpm#sponsor).\n\n",
        "## Background\n",
    );

    let mut builder = FileInfoBuilder::default();
    extract_copyright_information(&mut builder, Path::new("README.md"), text, 120.0, false);

    let file = builder
        .name("README.md".to_string())
        .base_name("README".to_string())
        .extension(".md".to_string())
        .path("README.md".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(file.authors.is_empty(), "authors: {:?}", file.authors);
}

#[test]
fn test_extract_copyright_information_ignores_flutter_issue_hygiene_markdown_link_prose() {
    let text = concat!(
        "See also:\n\n",
        " * [All open issues sorted by thumbs-up](https://github.com/flutter/flutter/issues?q=is%3Aissue+is%3Aopen+sort%3Areactions-%2B1-desc)\n",
        " * [Feature requests by thumbs-up](https://github.com/flutter/flutter/issues?q=is%3Aissue+is%3Aopen+sort%3Areactions-%2B1-desc+label%3A%22c%3A+new+feature%22)\n",
    );

    let mut builder = FileInfoBuilder::default();
    extract_copyright_information(
        &mut builder,
        Path::new("docs/contributing/issue_hygiene/README.md"),
        text,
        120.0,
        false,
    );

    let file = builder
        .name("README.md".to_string())
        .base_name("README".to_string())
        .extension(".md".to_string())
        .path("docs/contributing/issue_hygiene/README.md".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(file.authors.is_empty(), "authors: {:?}", file.authors);
}

#[test]
fn test_extract_copyright_information_ignores_flutter_api_sentence_fragment() {
    let text = concat!(
        "* If fixing it requires an API that is not yet available on stable, add the `p: waiting for stable update` label.\n",
        "  * If it's easy to determine, include the version that the replacement API will be available in the issue description.\n",
    );

    let mut builder = FileInfoBuilder::default();
    extract_copyright_information(
        &mut builder,
        Path::new("docs/infra/Packages-Gardener-Rotation.md"),
        text,
        120.0,
        false,
    );

    let file = builder
        .name("Packages-Gardener-Rotation.md".to_string())
        .base_name("Packages-Gardener-Rotation".to_string())
        .extension(".md".to_string())
        .path("docs/infra/Packages-Gardener-Rotation.md".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(file.authors.is_empty(), "authors: {:?}", file.authors);
}

#[test]
fn test_detector_timeout_and_non_timeout_paths_match_for_pnpm_markdown_link_prose() {
    let text = concat!(
        "</table>\n\n",
        "<!-- sponsors end -->\n\n",
        "Support this project by [becoming a sponsor](https://opencollective.com/pnpm#sponsor).\n\n",
        "## Background\n",
    );

    let (_c1, _h1, authors_no_deadline) = copyright::detect_copyrights(text, None);
    let (_c2, _h2, authors_with_deadline) =
        copyright::detect_copyrights(text, Some(Duration::from_secs(120)));

    assert_eq!(authors_no_deadline, authors_with_deadline);
    assert!(
        authors_with_deadline.is_empty(),
        "authors_with_deadline: {authors_with_deadline:?}"
    );
}

#[test]
fn test_extract_copyright_information_ignores_pnpm_changelog_markdown_link_on_large_input() {
    let repeated = "- Do not hang indefinitely, when there is a glob that starts with `!/` in `pnpm-workspace.yaml`. This fixes a regression introduced by [#9169](https://github.com/pnpm/pnpm/pull/9169).\n";
    let text = repeated.repeat(4000);

    let mut builder = FileInfoBuilder::default();
    extract_copyright_information(
        &mut builder,
        Path::new("pnpm/CHANGELOG.md"),
        &text,
        0.000001,
        false,
    );

    let file = builder
        .name("CHANGELOG.md".to_string())
        .base_name("CHANGELOG".to_string())
        .extension(".md".to_string())
        .path("pnpm/CHANGELOG.md".to_string())
        .file_type(FileType::File)
        .size(text.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(file.authors.is_empty(), "authors: {:?}", file.authors);
}

// A Jupyter notebook's code cell must not produce a copyright/holder false
// positive from the JSON string-array punctuation around source lines.
#[test]
fn test_extract_copyright_information_ipynb_code_cell_no_false_positive() {
    let notebook = r##"{
      "cells": [
        {"cell_type":"code","source":["@show typeof(C)\n","C[1:10,:]\n","# C.year #[!,:year]"],
         "outputs":[]}
      ],
      "nbformat": 4
    }"##;
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("01. Data.ipynb"),
        notebook,
        120.0,
        false,
    );

    let file = builder
        .name("01. Data.ipynb".to_string())
        .base_name("01. Data".to_string())
        .extension(".ipynb".to_string())
        .path("01. Data.ipynb".to_string())
        .file_type(FileType::File)
        .size(notebook.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(
        file.copyrights.is_empty(),
        "code cell should not yield a copyright: {:?}",
        file.copyrights
    );
    assert!(
        file.holders.is_empty(),
        "code cell should not yield a holder: {:?}",
        file.holders
    );
}

// A genuine copyright notice that lives inside a notebook cell's output text must
// be recovered (the raw JSON wrapping previously hid it from detection).
#[test]
fn test_extract_copyright_information_ipynb_detects_notice_in_output() {
    let notebook = r#"{
      "cells": [
        {"cell_type":"code","source":"solve()",
         "outputs":[{"output_type":"stream","name":"stdout",
           "text":["\t(c) Brendan O'Donoghue, Stanford University, 2012\n"]}]}
      ],
      "nbformat": 4
    }"#;
    let mut builder = FileInfoBuilder::default();

    extract_copyright_information(
        &mut builder,
        Path::new("09. Optimization.ipynb"),
        notebook,
        120.0,
        false,
    );

    let file = builder
        .name("09. Optimization.ipynb".to_string())
        .base_name("09. Optimization".to_string())
        .extension(".ipynb".to_string())
        .path("09. Optimization.ipynb".to_string())
        .file_type(FileType::File)
        .size(notebook.len() as u64)
        .build()
        .expect("builder should produce file info");

    assert!(
        file.copyrights
            .iter()
            .any(|c| c.copyright.contains("Brendan O'Donoghue")),
        "notice in output should be detected: {:?}",
        file.copyrights
    );
}
