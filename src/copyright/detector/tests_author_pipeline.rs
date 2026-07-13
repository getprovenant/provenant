// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn test_multi_author_by_chain_recovers_all_and_does_not_bleed() {
    // coreutils src/tail.c header: each contribution line is "<phrase> by
    // <Name> <email>.". All four authors must be recovered, and the first must
    // not absorb the leading word of the next line ("Extensions").
    let input = "\
   Original version by Paul Rubin <phr@ocf.berkeley.edu>.
   Extensions by David MacKenzie <djm@gnu.ai.mit.edu>.
   tail -f for multiple files by Ian Lance Taylor <ian@airs.com>.
   inotify back-end by Giuseppe Scrivano <gscrivano@gnu.org>.  */";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);
    let authors: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();

    assert!(
        authors.contains(&"Paul Rubin <phr@ocf.berkeley.edu>"),
        "first author bled or missing: {authors:?}"
    );
    assert!(
        authors.contains(&"David MacKenzie <djm@gnu.ai.mit.edu>"),
        "single-word-lead-in author dropped: {authors:?}"
    );
    assert!(
        authors.contains(&"Ian Lance Taylor <ian@airs.com>"),
        "authors: {authors:?}"
    );
    assert!(
        authors.contains(&"Giuseppe Scrivano <gscrivano@gnu.org>"),
        "authors: {authors:?}"
    );
    assert!(
        !authors.iter().any(|a| a.contains("Extensions")),
        "cross-line bleed into author name: {authors:?}"
    );
}

#[test]
fn test_by_name_with_trailing_comma_email_recovers_author() {
    // coreutils src/cksum_crc.c: "<phrase> by <Name>, <bare-email>." The email
    // trails the name as a sibling token rather than being folded into the name.
    let input = "/* Written by Q. Frank Xia, qx@math.columbia.edu.\n   Cosmetic changes and reorganization by David MacKenzie, djm@gnu.ai.mit.edu.  */";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);
    assert!(
        authors.iter().any(|a| a.author.contains("David MacKenzie")),
        "trailing-comma-email author dropped: {authors:?}"
    );
}

#[test]
fn test_line_initial_by_name_email_is_not_an_author() {
    // A bare, line-initial "by <Name> <email>" with no contribution phrase in
    // front is intentionally left out (matches curated behavior for isolated
    // mid-comment attributions such as posix-timers "by George Anzinger ...").
    let input = "by George Anzinger <george@mvista.com>";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);
    assert!(
        authors.is_empty(),
        "line-initial bare `by` should not yield an author: {authors:?}"
    );
}

#[test]
fn test_texinfo_comment_written_by_is_candidate_author() {
    // doc/sort-version.texi: a `@c` texinfo comment carrying "Written by <name>"
    // must survive the `@directive` skip and yield the author.
    let input = "@c Written by Assaf Gordon";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);
    assert!(
        authors.iter().any(|a| a.author == "Assaf Gordon"),
        "texinfo `@c Written by` author missed: {authors:?}"
    );
}

#[test]
fn test_structural_at_directive_mentioning_by_phrase_is_not_author() {
    // A structural directive (`@param`, `@brief`, ...) that merely mentions an
    // attribution phrase in its prose must stay filtered — only the `@c`/
    // `@comment` texinfo directive and `@author` carry real authorship.
    let input = "@param formatter Developed by John Smith";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);
    assert!(
        authors.is_empty(),
        "structural @param directive leaked an author: {authors:?}"
    );
}

#[test]
fn test_camelcase_provider_not_author_false_positive() {
    let input = "A meter implementation is created by a MeterProvider in this system.\nA trace implementation is created by a TracerProvider in this system.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    let author_values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();
    assert!(
        author_values
            .iter()
            .all(|a| *a != "MeterProvider in" && *a != "TracerProvider in"),
        "Unexpected provider false-positive authors: {author_values:?}"
    );
}

#[test]
fn test_markdown_transition_line_not_author() {
    let input = "The meaning of [*transition*.delay](https://github.com/d3/d3-transition/blob/master/README.md#transition_delay) has changed for chained transitions created by [*transition*.transition](https://github.com/d3/d3-transition/blob/master/README.md#transition_transition).";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        !authors.iter().any(|a| a.author.contains(
            "transition .transition https://github.com/d3/d3-transition/blob/master/README.md"
        )),
        "authors: {authors:?}"
    );
}

#[test]
fn test_json_author_field_does_not_capture_following_metadata_blob() {
    let input = "author: Box UK,\nurl: http://updates.jenkins-ci.org/download/plugins/jslint/0.7.6/jslint.hpi,\nversion: 0.7.6,\nwiki: https://wiki.jenkins-ci.org/display/JENKINS/JSLint+plugin";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    let author_values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();
    assert_eq!(author_values, vec!["Box UK"], "authors: {authors:?}");
}

#[test]
fn test_sentence_fragment_not_author() {
    let input = "with key equal to unescaped token";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_call_ref_fragment_not_author() {
    let input = "call @ref";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_boost_value_stack_call_ref_sentence_not_author() {
    let input = "Then to build a @ref value, first call @ref reset and optionally specify the boost::container::pmr::memory_resource.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_json_excerpt_developed_by_company_author_detected() {
    let input = r#"mes/0.2/jsgames.hpi","version":"0.2","wiki":"https://wiki.jenkins-ci.org/display/JENKINS/JSGames+Plugin"},"jslint":{"buildDate":"Jan 03, 2013","dependencies":[],"developers":[{"developerId":"gavd","email":"gavin.davies@boxuk.com","name":"Gavin Davies"}],"excerpt":"Lint JavaScript files, outputting to checkstyle format. Supports all JSLint options. Developed by Box UK.","gav":"org.jenkins-ci.plugins:jslint:0.7.6","labels":["misc"],"name":"jslint","previousTimestamp":"2013-01-03T20:22:38.00Z","previousVersion":"0.7.5","releaseTimestamp":"2013-01-03T20:29:06.00Z","requiredCore":"1.474","scm":"github.com""#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Box UK"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_plain_json_author_string_preserved() {
    let input = r#""author": "Google's Web DevRel Team","#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert_eq!(
        authors
            .iter()
            .map(|a| a.author.as_str())
            .collect::<Vec<_>>(),
        vec!["Google's Web DevRel Team"],
        "authors: {authors:?}"
    );
}

#[test]
fn test_plain_json_author_string_with_parenthesized_url_preserved() {
    let input = r#""author": "Qix (http://github.com/qix-)","#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert_eq!(
        authors
            .iter()
            .map(|a| a.author.as_str())
            .collect::<Vec<_>>(),
        vec!["Qix (http://github.com/qix-)"],
        "authors: {authors:?}"
    );
}

#[test]
fn test_plain_json_author_string_with_parenthesized_url_and_following_key_preserved() {
    let input = concat!(
        "   \"author\": \"Qix (http://github.com/qix-)\",\n",
        "   \"keywords\": [\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert_eq!(
        authors
            .iter()
            .map(|a| a.author.as_str())
            .collect::<Vec<_>>(),
        vec!["Qix (http://github.com/qix-)"],
        "authors: {authors:?}"
    );
}

#[test]
fn test_plain_json_author_string_machine_token_dropped_without_metadata_context() {
    let input = r#""author": "makeappicon", "images": []"#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_nested_json_author_string_machine_token_dropped_with_metadata_context() {
    let input = concat!(
        "{\n",
        "  \"info\": {\n",
        "    \"version\": 1,\n",
        "    \"author\": \"makeappicon\"\n",
        "  }\n",
        "}\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_metadata_label_placeholder_not_detected() {
    let input = "author: Package Author\nhomepage: Homepage\nrepository: Repository";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_media_title_metadata_does_not_merge_into_author() {
    let input = "Author: Chinmay Garde\nTitle: Bay Bridge At Night\nDescription: Embarcadero in the evening on Delta 3200";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert_eq!(
        authors
            .iter()
            .map(|a| a.author.as_str())
            .collect::<Vec<_>>(),
        vec!["Chinmay Garde"],
        "authors: {authors:?}"
    );
}

#[test]
fn test_author_span_prose_continuation_not_detected() {
    let input = "contributors, for example.\nIf you want to help, start with docs.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_span_legal_clause_not_detected() {
    let input = "authors, grants you the right to use and distribute this work.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_uc_berkeley_notice_author_does_not_absorb_following_legal_prose() {
    let input = concat!(
        "This product includes software developed by the University of\n",
        "California, Berkeley and its contributors.\n",
        "Effective immediately, licensees and distributors are no longer required to include the acknowledgement within advertising materials.\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    let author_values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();
    assert!(
        author_values.contains(&"the University of California, Berkeley and its contributors"),
        "authors: {authors:?}"
    );
    assert!(
        !author_values
            .iter()
            .any(|a| a.contains("Effective immediately")),
        "authors: {authors:?}"
    );
}

#[test]
fn test_author_markdown_link_prose_not_detected() {
    let input = "the command [#7403] (https://github.com/pnpm/pnpm/issues/7403) changed behavior.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_markdown_link_label_not_detected() {
    let input = "[becoming a sponsor] (https://opencollective.com/pnpm#sponsor)";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_flutter_issue_hygiene_link_not_detected() {
    let input = "[All open issues sorted by thumbs-up] (https://github.com/flutter/flutter/issues?q=is%3Aissue+is%3Aopen+sort%3Areactions-%2B1-desc)";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_flutter_api_sentence_not_detected() {
    let input =
        "If fixing it requires an API that is not yet available on stable, add the waiting label.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_windows_filedescription_resource_not_detected() {
    let input = concat!(
        "VALUE \"LegalCopyright\", \"Copyright 2014 The Flutter Authors. All rights reserved.\" \"\\0\"\n",
        "VALUE \"FileDescription\", \"A sample application demonstrating Flutter APIs.\" \"\\0\"\n",
        "VALUE \"FileVersion\", VERSION_AS_STRING \"\\0\"\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .all(|a| !a.author.contains("FileDescription") && !a.author.contains("FileVersion")),
        "authors: {authors:?}"
    );
}

#[test]
fn test_author_camel_case_class_phrase_not_detected() {
    let input = "A delegate used by RenderListWheelViewport to manage its children. the ListWheelChildManager";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_written_by_notice_tail_not_detected() {
    let input = "This software was written by Alexander Peslyak in d+. No copyright is claimed.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert_eq!(
        authors
            .iter()
            .map(|a| a.author.as_str())
            .collect::<Vec<_>>(),
        vec!["Alexander Peslyak"],
        "authors: {authors:?}"
    );
}

#[test]
fn test_author_homepage_label_not_detected() {
    let input = "homepage Homepage repository Repository";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_pnpm_readme_sponsor_line_not_detected() {
    let input =
        "Support this project by [becoming a sponsor](https://opencollective.com/pnpm#sponsor).";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_pnpm_changelog_issue_link_not_detected() {
    let input = "- Fixed `minimumReleaseAgeExclude` not being respected by `pnpm dlx` [#10338](https://github.com/pnpm/pnpm/issues/10338).";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_pnpm_readme_snippet_not_detected() {
    let input = concat!(
        "</table>\n\n",
        "<!-- sponsors end -->\n\n",
        "Support this project by [becoming a sponsor](https://opencollective.com/pnpm#sponsor).\n\n",
        "## Background\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_pnpm_changelog_snippet_not_detected() {
    let input = concat!(
        "- Fixed `allowBuilds` not working when set via `.pnpmfile.cjs` [#10516](https://github.com/pnpm/pnpm/issues/10516).\n",
        "- When the [`enableGlobalVirtualStore`](https://pnpm.io/settings#enableglobalvirtualstore) option is set, the `pnpm deploy` command would incorrectly create symlinks to the global virtual store. To keep the deploy directory self-contained, `pnpm deploy` now ignores this setting and always creates a localized virtual store within the deploy directory.\n",
        "- Fixed `minimumReleaseAgeExclude` not being respected by `pnpm dlx` [#10338](https://github.com/pnpm/pnpm/issues/10338).\n\n",
        "## 10.29.2\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_json_author_object_name_preferred_over_url_tail() {
    let input =
        "\"author\": { \"name\": \"Chen Fengyuan\", \"url\": \"https://chenfengyuan.com/\" }";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert_eq!(
        authors
            .iter()
            .map(|a| a.author.as_str())
            .collect::<Vec<_>>(),
        vec!["Chen Fengyuan"],
        "authors: {authors:?}"
    );
}

#[test]
fn test_multiline_json_author_object_name_detected() {
    let input = concat!(
        "  \"author\": {\n",
        "    \"name\": \"Chen Fengyuan\",\n",
        "    \"url\": \"https://chenfengyuan.com/\"\n",
        "  }\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Chen Fengyuan"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_metadata_json_author_fallback_keeps_collective_and_single_word_names() {
    let input = r#"{
  "components": [
    {
      "supplier": { "name": "Google LLC" },
      "author": "gRPC authors",
      "name": "gRPC (C++)"
    },
    {
      "supplier": { "name": "Meta Open Source" },
      "author": "Meta",
      "name": "folly"
    },
    {
      "supplier": { "name": "The libunwind project" },
      "author": "The libunwind project",
      "name": "libunwind"
    },
    {
      "supplier": { "name": "Google LLC" },
      "author": "S2Geometry",
      "name": "S2 Geometry Library"
    }
  ]
}"#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    let values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();
    assert!(values.contains(&"gRPC authors"), "authors: {authors:?}");
    assert!(values.contains(&"Meta"), "authors: {authors:?}");
    assert!(
        values.contains(&"The libunwind project"),
        "authors: {authors:?}"
    );
    assert!(values.contains(&"S2Geometry"), "authors: {authors:?}");
}

#[test]
fn test_json_code_example_author_fields_do_not_create_authors() {
    let input = r#"{
  "expectedStages": [
    {
      "$match": {
        "author": "Agatha Christie"
      }
    },
    {
      "$setMetadata": {
        "score": {
          "$divide": [1, 2]
        }
      }
    }
  ]
}"#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_code_pipeline_author_match_not_detected_as_author() {
    let input = r#"{
  $scoreFusion: {
    input: {
      pipelines: {
        pipeOne: [
          { $match : { author : "Agatha Christie" } },
          { $sort: {author: 1} }
        ]
      }
    }
  }
}"#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_json_sponsor_description_does_not_create_authors() {
    let input = r#"{
  "description": "A useful plugin",
  "sponsor": {
    "@type": "Organization",
    "name": "Example Org",
    "description": "Developer as a service Plugin in the cloud when a workflow runs."
  }
}"#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_xml_author_elements_do_not_create_file_authors() {
    let input = concat!(
        "<bookstore>\n",
        "  <book><author>Joe Bob</author></book>\n",
        "  <book><author>Mary Bob</author></book>\n",
        "</bookstore>\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_atom_feed_identifier_fields_do_not_create_authors() {
    let input = concat!(
        "<feed>\n",
        "  <id>tag:contoso.com,2000</id>\n",
        "  <updated>2006-04-25T12:12:12Z</updated>\n",
        "  <email>jerry@Contoso.com</email>\n",
        "</feed>\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_xml_nested_name_tags_inside_author_do_not_create_file_authors() {
    let input = concat!(
        "<book>\n",
        "  <author>\n",
        "    <first-name>Joe</first-name>\n",
        "    <last-name>Bob</last-name>\n",
        "  </author>\n",
        "</book>\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_xml_lang_attribute_value_does_not_create_author() {
    let input = r#"<bookstore xml:lang="en-usabcd"></bookstore>"#;
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_written_by_sentence_trims_following_description_clause() {
    let input = "JUnit is a regression testing framework written by Erich Gamma and Kent Beck. It is used by the developer who implements unit tests in Java.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    let values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();
    assert!(
        values.contains(&"Erich Gamma and Kent Beck"),
        "authors: {authors:?}"
    );
    assert!(
        !values
            .iter()
            .any(|value| value.contains("It is used by the developer")),
        "authors: {authors:?}"
    );
}

#[test]
fn test_multiline_xml_description_written_by_sentence_keeps_only_author_names() {
    let input = concat!(
        "<description>JUnit is a regression testing framework written by Erich Gamma and Kent Beck.\n",
        "It is used by the developer who implements unit tests in Java.</description>\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    let values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();
    assert!(
        values.contains(&"Erich Gamma and Kent Beck"),
        "authors: {authors:?}"
    );
    assert!(
        !values
            .iter()
            .any(|value| value.contains("JUnit is a regression testing framework")),
        "authors: {authors:?}"
    );
    assert!(
        !values
            .iter()
            .any(|value| value.contains("It is used by the developer")),
        "authors: {authors:?}"
    );
}

#[test]
fn test_required_scope_word_is_not_author() {
    let input = "required";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_notice_developed_by_org_list_without_url_is_not_author() {
    let input = concat!(
        "This product includes software developed by NASA Ames Research Center,\n",
        "Lawrence Livermore National Laboratory, and Veridian Information Solutions,\n",
        "Inc. Visit www.OpenPBS.org for OpenPBS software support,\n",
        "products, and information.\n",
    );
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_fast_path_proposal_phrase_not_author() {
    let input = "Clinger's fast path, inspired by Jakub Jelínek's proposal";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_readme_security_review_prose_not_author() {
    let input = "application developers can trust, the C++ Alliance has commissioned Bishop Fox to perform a security audit of the Boost.JSON library. The report linked here";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_tomcat_html_doc_prose_not_author() {
    let input = "the order defined by the DTD (see Section 13.3).</p>";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_tomcat_contributing_prose_not_author() {
    let input = "time as all committers are volunteers on the project. If a significant amount";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_license_file_reference_note_not_author() {
    let input = "Licence: BSD 3-Clause + Condition for any enhancements shared publicly or with the author (see LICENSE.txt).";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_update_center_metadata_blob_not_multiple_authors() {
    let input = "author: Box UK, url: http://updates.jenkins-ci.org/download/plugins/jslint/0.7.6/jslint.hpi, version: 0.7.6, wiki: https://wiki.jenkins-ci.org/display/JENKINS/JSLint+plugin, title: JSLint plugin, buildDate: Jan 03, 2013, developerId: gavd";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    let values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();
    assert_eq!(values, vec!["Box UK"], "authors: {authors:?}");
}

#[test]
fn test_gsoc_javascript_language_phrase_not_author() {
    let input = "My proposal is based on getting full support for JavaScript within the RoboComp framework. For this, the current state of generation of written components in the JavaScript language must be improved.";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        !authors
            .iter()
            .any(|a| a.author == "components in the JavaScript language"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_apostrophized_person_name_metadata_not_author() {
    let input = "type' Person name' AadityaNair";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_language_fragment_not_author() {
    let input = "in PHP";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_single_word_stopword_not_author() {
    let input = "In";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_short_comma_metadata_phrase_not_author() {
    let input = "GENIVI, several standard";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_short_url_suffix_phrase_not_author() {
    let input = "J.L.Blanco (https://github.com/jlblancoc)";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_lowercase_starting_multiword_fragment_not_author() {
    let input = "around the world. It";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_dangling_two_word_phrase_not_author() {
    let input = "Sandcastle and";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_created_by_without_handle_or_email_is_rejected() {
    let (_copyrights, _holders, authors) =
        detect_copyrights_from_text("Created by IntelliJ IDEA\n");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_created_by_tool_banner_is_rejected() {
    let (_copyrights, _holders, authors) =
        detect_copyrights_from_text("created by Grunt and NPM.\n");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_extract_toml_authors_array_as_single_combined_detection() {
    let input = "authors = [\"The Rand Project Developers\", \"The Rust Project Developers\"]\n";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "The Rand Project Developers The Rust Project Developers"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_normalize_original_author_current_tail() {
    let input = "* Original author: Chris Pallotta <chris@allmedia.com>\n* Current maintainer: Jim Van Zandt <jrv@vanzandt.mv.com>\n";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "Chris Pallotta <chris@allmedia.com>"),
        "authors: {authors:?}"
    );
    assert!(
        authors
            .iter()
            .any(|a| a.author == "Jim Van Zandt <jrv@vanzandt.mv.com>"),
        "authors: {authors:?}"
    );
    assert!(
        !authors.iter().any(|a| a.author.contains("Current")),
        "authors: {authors:?}"
    );
}

#[test]
fn test_normalize_original_authors_multiline_to_separate_people() {
    let input = "* Original Authors: Robert Jennings <rcj@linux.vnet.ibm.com>\n*                   Seth Jennings <sjenning@linux.vnet.ibm.com>\n";
    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "Robert Jennings <rcj@linux.vnet.ibm.com>"),
        "authors: {authors:?}"
    );
    assert!(
        authors
            .iter()
            .any(|a| a.author == "Seth Jennings <sjenning@linux.vnet.ibm.com>"),
        "authors: {authors:?}"
    );
    assert!(
        !authors.iter().any(|a| {
            a.author
                == "Robert Jennings <rcj@linux.vnet.ibm.com> Seth Jennings <sjenning@linux.vnet.ibm.com>"
        }),
        "authors: {authors:?}"
    );
}

#[test]
fn test_maintainers_label_extracts_author_and_trims_gitrepo_suffix() {
    let input = "Maintainers Tianon Gravi <admwiggin@gmail.com> (@tianon) GitRepo https://github.com/tianon/docker-bash.git\n";

    let (_c, _h, authors) = detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert!(
        values.contains(&"Tianon Gravi <admwiggin@gmail.com> (@tianon)"),
        "authors: {values:?}"
    );
    assert!(
        !values.iter().any(|value| value.contains("GitRepo")),
        "authors: {values:?}"
    );
}

#[test]
fn test_maintainer_comment_extracts_author_without_label() {
    let input = "# Maintainer: Sébastien Luttringer <seblu@archlinux.org>\n";

    let (_c, _h, authors) = detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Sébastien Luttringer <seblu@archlinux.org>"),
        "authors: {values:?}"
    );
    assert!(
        !values.iter().any(|value| value.contains("Maintainer")),
        "authors: {values:?}"
    );
}

#[test]
fn test_author_comment_extracts_obfuscated_angle_contact_author() {
    let input = "* Author: Deepak M <m.deepak at intel.com>\n";

    let (_c, _h, authors) = detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Deepak M m.deepak at intel.com"),
        "authors: {values:?}"
    );
}

#[test]
fn test_maintainers_label_without_email_does_not_extract_author() {
    let input = "Maintainers the Docker Community\n";

    let (_c, _h, authors) = detect_copyrights_from_text(input);
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_markup_authors_section_collective_author_detected() {
    let input = r#"<section name="Authors"><p>The PulseAudio Developers &lt;bugs@example.org&gt;; PulseAudio is available from <url href="https://example.org"/></p></section>"#;

    let (_copyrights, _holders, authors) = detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "The PulseAudio Developers"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_second_by_chain_splits_into_separate_author() {
    // musl COPYRIGHT: a single coordinated sentence with two `by` clauses.
    // ScanCode makes each distinct `by` clause its own author, so both names
    // must be recovered as separate authors (previously John Spencer was
    // dropped when the second clause was left unattached).
    let input = "The powerpc port was also originally written by Richard \
Pennington, and later supplemented and integrated by John Spencer. It is \
licensed under the standard MIT terms.\n";

    let (_c, _h, authors) = detect_copyrights_from_text(input);
    let values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();

    assert!(
        values.contains(&"Richard Pennington"),
        "first by-clause author missing: {values:?}"
    );
    assert!(
        values.contains(&"John Spencer"),
        "second by-clause author missing: {values:?}"
    );
}

#[test]
fn test_second_by_chain_splits_across_wrapped_lines() {
    // Same coordination as above but line-wrapped mid-name, as in the real
    // musl COPYRIGHT file.
    let input = "The powerpc port was also originally written by Richard\n\
Pennington, and later supplemented and integrated by John Spencer. It is licensed\n\
under the standard MIT terms.\n";

    let (_c, _h, authors) = detect_copyrights_from_text(input);
    let values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();

    assert!(
        values.contains(&"Richard Pennington") && values.contains(&"John Spencer"),
        "wrapped two-author split failed: {values:?}"
    );
}

#[test]
fn test_single_by_conjoined_name_list_stays_one_author() {
    // A single `by` clause with a conjoined name list must remain one author
    // string (matching the curated multi-name author expectations); only a
    // *second* `by` clause introduces a new author.
    let input = "/* Written by David Ihnat, David MacKenzie, and Jim Meyering.  */\n";

    let (_c, _h, authors) = detect_copyrights_from_text(input);
    let values: Vec<&str> = authors.iter().map(|a| a.author.as_str()).collect();

    assert!(
        !values.contains(&"David MacKenzie") && !values.contains(&"Jim Meyering"),
        "single-by conjoined list was wrongly split: {values:?}"
    );
    assert!(
        values.iter().any(|a| a.contains("David Ihnat")),
        "primary author missing: {values:?}"
    );
}
