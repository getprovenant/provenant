// SPDX-FileCopyrightText: nexB Inc. and others
// ScanCode is a trademark of nexB Inc.
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use super::*;

#[test]
fn test_prose_copyright_word_without_proper_holder_is_not_detected() {
    // Prose that merely uses the word "copyright" as a common noun, with no
    // proper-noun/company/year holder, must not become a copyright — mirroring
    // ScanCode's grammar, which has no `<COPY> <NN>` production. The span
    // fallbacks (which run when the grammar builds no node) must not sweep the
    // surrounding common-noun prose into a copyright.
    for prose in [
        "Thanks to Jim Meyering for doing the copyright paperwork.",
        "The whole point is that the copyright and the source code would then be shared.",
        "2. E. I. du Pont de Nemours and Company copyright (ImageMagick was originally based on their code).",
    ] {
        let (copyrights, _holders, _authors) = detect_copyrights_from_text(prose);
        assert!(
            copyrights.is_empty(),
            "prose produced copyrights: {copyrights:?} for {prose:?}"
        );
    }
}

#[test]
fn test_prose_copyright_spanning_a_sentence_boundary_is_not_detected() {
    // Article prose that discusses copyright (github/opensource.guide legal.md and
    // its translations) has a bare `copyright` token sweep the rest of the
    // sentence — and across the sentence period into the next one — into a
    // copyright/holder. ScanCode emits nothing. A real notice never crosses a
    // sentence boundary (`... by default. That is ...`).
    for prose in [
        "that work is under exclusive copyright by default. That is, the law assumes you have a say.",
        "who holds copyright can get complicated and confusing very quickly. Switching to a new license.",
        "you aren't the sole copyright holder. If you're the sole contributor you own it.",
        "hai una licenza copyright esclusivo per impostazione predefinita. Cioè, la legge presuppone.",
        "non puoi semplicemente cambiare la licenza del tuo progetto MIT. In sostanza vale questo.",
    ] {
        let (copyrights, holders, _authors) = detect_copyrights_from_text(prose);
        assert!(
            copyrights.is_empty(),
            "prose produced copyrights: {copyrights:?} for {prose:?}"
        );
        assert!(
            holders.is_empty(),
            "prose produced holders: {holders:?} for {prose:?}"
        );
    }
}

#[test]
fn test_notices_before_a_following_sentence_are_kept() {
    // The sentence-boundary junk rule must keep genuine notices whose holder is
    // named before the boundary: a year-range notice (`1994-1999. The MITRE ...`),
    // and a lowercase collective-agent holder (`by the original authors. ...`).
    for (text, expected) in [
        (
            "Copyright (c) 1994-1999. The MITRE Corporation makes no representations.",
            "MITRE",
        ),
        (
            "Copyright by the original authors. Redistribution is permitted.",
            "original authors",
        ),
    ] {
        let (copyrights, _holders, _authors) = detect_copyrights_from_text(text);
        assert!(
            copyrights.iter().any(|c| c.copyright.contains(expected)),
            "expected {expected:?} kept in {copyrights:?} for {text:?}"
        );
    }
}

#[test]
fn test_notice_with_trailing_email_tld_and_second_copyright_is_kept() {
    // A period after an email TLD that precedes a second `Copyright` clause is not
    // a prose sentence boundary; the whole notice must survive.
    let text = "Pascal Andre, andre@chimay.via.ecp.fr. Copyright (c) 1995, Pascal Andre";
    let (copyrights, _holders, _authors) = detect_copyrights_from_text(text);
    assert!(
        copyrights
            .iter()
            .any(|c| c.copyright.contains("Pascal Andre")),
        "expected the Pascal Andre notice to survive: {copyrights:?}"
    );
}

#[test]
fn test_lowercase_copyright_with_proper_holder_still_detected() {
    // The span guard must not suppress legitimate lowercase notices that carry a
    // real holder: proper noun, company, "by <name>", or an all-caps acronym.
    for (text, expected) in [
        (
            "This program is copyrighted by the Free Software Foundation.",
            "Free Software Foundation",
        ),
        ("This code is copyright CERN.", "CERN"),
        (
            "The following reference appears in all the copies: Scilab (c)INRIA-ENPC.",
            "INRIA-ENPC",
        ),
    ] {
        let (copyrights, _holders, _authors) = detect_copyrights_from_text(text);
        assert!(
            copyrights.iter().any(|c| c.copyright.contains(expected)),
            "expected holder {expected:?} missing in {copyrights:?} for {text:?}"
        );
    }
}

#[test]
fn test_unicode_break_test_data_lines_do_not_produce_holders() {
    // Unicode Character Database segmentation-test data (unicode-org/icu4x
    // WordBreakTest.txt / GraphemeBreakTest.txt) embeds the copyright codepoint
    // `00A9`/`©` as literal test input on lines dense with the break markers
    // `÷`/`×`. The genuine file header notice is kept; the data lines are not
    // turned into holders/copyrights.
    let input = concat!(
        "# \u{00a9} 2025 Unicode\u{00ae}, Inc.\n",
        "\u{00f7} 0009 \u{00d7} 0308 \u{00f7} 00A9 \u{00f7} # \u{00f7} [0.2] SIGN (ExtPict) \u{00d7} [9.0] COMBINING DIAERESIS\n",
        "\u{00f7} 000D \u{00f7} 00A9 \u{00f7} # \u{00f7} [0.2] (CR) \u{00f7} [3.1] COPYRIGHT SIGN (ExtPict)\n",
    );

    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);

    // The real header notice survives.
    assert!(
        holders.iter().any(|h| h.holder.contains("Unicode")),
        "expected the header holder to survive: {holders:?}"
    );
    // No break-test data line becomes a holder or copyright.
    assert!(
        !holders
            .iter()
            .any(|h| h.holder.contains('\u{00f7}') || h.holder.contains('\u{00d7}')),
        "segmentation data leaked into holders: {holders:?}"
    );
    assert!(
        !copyrights
            .iter()
            .any(|c| c.copyright.contains('\u{00f7}') || c.copyright.contains('\u{00d7}')),
        "segmentation data leaked into copyrights: {copyrights:?}"
    );
}

#[test]
fn test_bpe_tokenizer_data_lines_do_not_produce_copyrights_or_holders() {
    // Hugging Face subword-tokenizer artifacts: merges.txt BPE pairs and
    // vocab.json token:id entries embed the `©` symbol and the literal
    // `copyright` token. They must not be surfaced as copyright/holder notices.
    let input = concat!(
        "Ã© goo gle</w> fren ch</w>\n",
        "âģ ©</w> f ren st y</w>\n",
        "\"©\": 102,\n",
        "\"copyright</w>\": 15778,\n",
    );

    let (copyrights, holders, authors) = detect_copyrights_from_text(input);

    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_hf_tokenizer_json_suppresses_merges_but_keeps_outside_notice() {
    // A Hugging Face tokenizer.json embeds a BPE "merges" array of mojibake
    // byte-pairs (junk) but is a larger document. Only the merges array is
    // suppressed: a genuine notice elsewhere in the same file is preserved.
    let input = concat!(
        "{\n",
        "  \"copyright\": \"Copyright 2024 Acme Inc.\",\n",
        "  \"model\": {\n",
        "    \"type\": \"BPE\",\n",
        "    \"vocab\": { \"a\": 0 },\n",
        "    \"merges\": [\n",
        "      \"pok Ã©\",\n",
        "      \"Â ©\",\n",
        "      \"Ú ©\"\n",
        "    ]\n",
        "  }\n",
        "}\n",
    );

    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);

    // The real notice survives.
    assert!(
        copyrights.iter().any(|c| c.copyright.contains("Acme Inc")),
        "expected the real notice to be detected: {copyrights:?}"
    );
    // None of the BPE merge mojibake pairs leak through as notices.
    assert!(
        !copyrights.iter().any(|c| c.copyright.contains('©')),
        "merge-table junk leaked into copyrights: {copyrights:?}"
    );
    assert!(
        !holders.iter().any(|h| h.holder.contains('©')),
        "merge-table junk leaked into holders: {holders:?}"
    );
}

#[test]
fn test_compact_tokenizer_json_keeps_outside_notice() {
    // A compact (single-line) tokenizer.json cannot be split by line, so the
    // merge-span filter is not applied and a real notice on that line is not
    // dropped. (Junk suppression for genuine merge data still relies on the
    // per-line fragment checks; preserving a real notice takes priority.)
    let input = r#"{"copyright":"Copyright 2024 Acme Inc.","model":{"type":"BPE","vocab":{"a":0},"merges":["pok Ã©","Â ©"]}}"#;

    let (copyrights, _holders, _authors) = detect_copyrights_from_text(input);

    assert!(
        copyrights.iter().any(|c| c.copyright.contains("Acme Inc")),
        "compact tokenizer.json must keep the real notice: {copyrights:?}"
    );
}

#[test]
fn test_bpe_merges_table_produces_no_copyrights() {
    // A full BPE merges.txt (header + two-token merge rules) embeds `©` and
    // mojibake byte pairs that are not copyright notices.
    let input = concat!(
        "#version: 0.2\n",
        "i n\n",
        "t h\n",
        "Â ©\n",
        "pok Ã©\n",
        "Ú ©\n",
    );

    let (copyrights, holders, authors) = detect_copyrights_from_text(input);

    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_swift_convention_c_signatures_do_not_produce_copyrights_or_holders() {
    let input = concat!(
        "let invokeSuperSetter: @convention(c) (NSObject, AnyClass, Selector, AnyObject?) -> Void = { object, superclass, selector, delegate in\n",
        "typealias Setter = @convention(c) (NSObject, Selector, AnyObject?) -> Void\n",
    );

    let (copyrights, holders, authors) = detect_copyrights_from_text(input);

    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_busybox_env_modified_by_line_does_not_absorb_correct_usage_bullet() {
    let content = "* Modified by Vladimir Oleynik <dzo@simtreas.ru> (C) 2003\n* - correct \"-\" option usage\n";
    let (copyrights, holders, _authors) = detect_copyrights_from_text(content);

    assert!(
        copyrights
            .iter()
            .any(|c| c.copyright == "Vladimir Oleynik <dzo@simtreas.ru> (c) 2003"),
        "copyrights: {:#?}",
        copyrights.iter().map(|c| &c.copyright).collect::<Vec<_>>()
    );
    assert!(
        !copyrights.iter().any(|c| c.copyright.contains("- correct")),
        "copyrights: {:#?}",
        copyrights.iter().map(|c| &c.copyright).collect::<Vec<_>>()
    );
    assert!(
        holders.iter().any(|h| h.holder == "Vladimir Oleynik"),
        "holders: {:#?}",
        holders.iter().map(|h| &h.holder).collect::<Vec<_>>()
    );
}

#[test]
fn test_copyright_span_does_not_absorb_following_author_line() {
    let input = "Copyright (c) Ian F. Darwin 1986\nSoftware written by Ian F. Darwin and others;";
    let (_c, holders, _authors) = detect_copyrights_from_text(input);
    let hs: Vec<String> = holders.into_iter().map(|h| h.holder).collect();
    assert!(hs.iter().any(|h| h == "Ian F. Darwin"), "holders: {hs:#?}");
    assert!(
        !hs.iter().any(|h| h == "Ian F. Darwin Software"),
        "holders: {hs:#?}"
    );
}

#[test]
fn test_copyright_span_does_not_absorb_following_lint_directive_line() {
    let input = concat!(
        "// (c) Example Corp. and affiliates. Confidential and proprietary.\n",
        "// @lint-ignore-every FBOBJCIMPORTORDER1 METHOD_BRACKETSMETHOD_BRACKETS\n",
    );

    let (copyrights, _holders, _authors) = detect_copyrights_from_text(input);
    let values: Vec<String> = copyrights.into_iter().map(|c| c.copyright).collect();

    assert!(
        values
            .iter()
            .any(|c| c == "(c) Example Corp. and affiliates. Confidential and proprietary"),
        "copyrights: {values:#?}"
    );
    assert!(
        !values.iter().any(|c| c.contains("@lint-ignore-every")),
        "copyrights: {values:#?}"
    );
}

#[test]
fn test_html_comment_copyright_does_not_absorb_following_body_text() {
    let input = concat!(
        "<!-- Copyright 2008 ABCD, LLC. -->\n",
        "<html>\n",
        "<body>\n",
        "Periodic Manager\n",
        "</body>\n",
        "</html>\n",
    );

    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);
    let cs: Vec<String> = copyrights.into_iter().map(|c| c.copyright).collect();
    let hs: Vec<String> = holders.into_iter().map(|h| h.holder).collect();

    assert!(
        cs.iter().any(|c| c == "Copyright 2008 ABCD, LLC."),
        "copyrights: {cs:#?}"
    );
    assert!(
        !cs.iter().any(|c| c.contains("Periodic Manager")),
        "copyrights: {cs:#?}"
    );
    assert!(hs.iter().any(|h| h == "ABCD, LLC."), "holders: {hs:#?}");
    assert!(
        !hs.iter().any(|h| h.contains("Periodic Manager")),
        "holders: {hs:#?}"
    );
}

#[test]
fn test_html_comment_noise_does_not_extend_plain_copyright_line() {
    let input = concat!(
        "Copyright 2024 Example Corp.\n",
        "<!-- sponsors end -->\n",
        "<div>body</div>\n",
    );

    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);
    let cs: Vec<String> = copyrights.into_iter().map(|c| c.copyright).collect();
    let hs: Vec<String> = holders.into_iter().map(|h| h.holder).collect();

    assert!(
        cs.iter().any(|c| c == "Copyright 2024 Example Corp."),
        "copyrights: {cs:#?}"
    );
    assert!(
        !cs.iter()
            .any(|c| c.contains("sponsors end") || c.contains("body")),
        "copyrights: {cs:#?}"
    );
    assert!(hs.iter().any(|h| h == "Example Corp."), "holders: {hs:#?}");
}

#[test]
fn test_consecutive_html_comment_copyright_lines_keep_continuation_only() {
    let input = concat!(
        "<!-- Copyright 2024 Example Corp. -->\n",
        "<!-- All rights reserved. -->\n",
        "<!-- sponsors end -->\n",
        "<div>body</div>\n",
    );

    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);
    let cs: Vec<String> = copyrights.into_iter().map(|c| c.copyright).collect();
    let hs: Vec<String> = holders.into_iter().map(|h| h.holder).collect();

    assert!(
        cs.iter().any(|c| c == "Copyright 2024 Example Corp."),
        "copyrights: {cs:#?}"
    );
    assert!(
        !cs.iter()
            .any(|c| c.contains("sponsors end") || c.contains("body")),
        "copyrights: {cs:#?}"
    );
    assert!(hs.iter().any(|h| h == "Example Corp."), "holders: {hs:#?}");
}

#[test]
fn test_detect_arch_floppy_h_bare_1995_dropped_for_x86() {
    let content =
        "* Copyright (C) 1995\n */\n#ifndef _ASM_X86_FLOPPY_H\n#define _ASM_X86_FLOPPY_H\n";
    let (copyrights, _holders, _authors) = detect_copyrights_from_text(content);
    assert!(copyrights.is_empty());
}

#[test]
fn test_detect_changelog_single_timestamp_is_ignored() {
    let content = "updated year in copyright\n\n2008-01-26 11:46  vruppert\n";
    let (copyrights, holders, _authors) = detect_copyrights_from_text(content);
    assert!(copyrights.is_empty());
    assert!(holders.is_empty());
}

#[test]
fn test_drop_obfuscated_email_year_only_copyright() {
    let content = "Copyright (C) 2008 <srinivasa.deevi at conexant dot com>\n";
    let (copyrights, _holders, _authors) = detect_copyrights_from_text(content);
    assert!(copyrights.is_empty());
}

#[test]
fn test_glide_3dfx_copyright_notice_does_not_trigger_for_notice_s_plural() {
    let content = "copyright notice(s)\n";
    let (copyrights, _holders, _authors) = detect_copyrights_from_text(content);
    assert!(!copyrights.iter().any(|c| {
        c.copyright
            .to_ascii_lowercase()
            .contains("copyright notice")
    }));
}

#[test]
fn test_copyright_notice_of_prose_does_not_emit_xerox_holder() {
    let content = "the above copyright notice of Xerox Corporation,";
    let (copyrights, holders, authors) = detect_copyrights_from_text(content);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:#?}");
    assert!(holders.is_empty(), "holders: {holders:#?}");
    assert!(authors.is_empty(), "authors: {authors:#?}");
}

#[test]
fn test_play_header_does_not_emit_bare_c_from_year_shadow() {
    let content = "Copyright (C) from 2022 The Play Framework Contributors <https://github.com/playframework>, 2011-2021 Lightbend Inc. <https://www.lightbend.com>\n";
    let (copyrights, holders, _authors) = detect_copyrights_from_text(content);

    assert!(
        copyrights
            .iter()
            .any(|c| c.copyright.contains("The Play Framework Contributors")),
        "copyrights: {copyrights:?}"
    );
    assert!(
        !copyrights.iter().any(|c| c.copyright == "(c) from 2022"),
        "copyrights: {copyrights:?}"
    );
    assert!(
        holders
            .iter()
            .any(|h| h.holder.contains("The Play Framework Contributors")),
        "holders: {holders:?}"
    );
}

#[test]
fn test_drop_symbol_year_only_copyright() {
    let input = "Copyright © 2021\nCopyright (c) 2017\n";
    let (c, _h, _a) = detect_copyrights_from_text(input);
    assert!(
        !c.iter().any(|cr| cr.copyright == "Copyright (c) 2021"),
        "Expected © year-only to be dropped, got: {:?}",
        c.iter().map(|cr| &cr.copyright).collect::<Vec<_>>()
    );
    assert!(
        c.iter().any(|cr| cr.copyright == "Copyright (c) 2017"),
        "Expected non-© year-only to be kept, got: {:?}",
        c.iter().map(|cr| &cr.copyright).collect::<Vec<_>>()
    );
}

#[test]
fn test_c_sign_path_fragment_is_not_detected_as_copyright() {
    let input = "(C)Ljoptsimple/AbstractOptionSpec";
    let (c, h, a) = detect_copyrights_from_text(input);
    assert!(c.is_empty(), "copyrights: {c:#?}");
    assert!(h.is_empty(), "holders: {h:#?}");
    assert!(a.is_empty(), "authors: {a:#?}");
}

#[test]
fn test_copyright_scan_phrase_is_not_detected_as_copyright() {
    let input = "Measures the end-to-end composer copyright scan";
    let (c, h, a) = detect_copyrights_from_text(input);
    assert!(c.is_empty(), "copyrights: {c:#?}");
    assert!(h.is_empty(), "holders: {h:#?}");
    assert!(a.is_empty(), "authors: {a:#?}");
}

#[test]
fn test_generated_annotation_line_is_not_absorbed_into_copyright() {
    let input = "/* Copyright (C) 2024 Acme Corp.\n * @generated by protobuf */";
    let (c, h, _a) = detect_copyrights_from_text(input);
    assert!(
        c.iter()
            .any(|cr| cr.copyright == "Copyright (c) 2024 Acme Corp."),
        "copyrights: {c:#?}"
    );
    assert!(
        !c.iter().any(|cr| cr.copyright.contains("@generated")),
        "copyrights: {c:#?}"
    );
    assert!(
        h.iter().any(|holder| holder.holder == "Acme Corp."),
        "holders: {h:#?}"
    );
}

#[test]
fn test_detect_no_copyright() {
    let (c, h, a) = detect_copyrights_from_text("This is just some random code.");
    assert!(c.is_empty());
    assert!(h.is_empty());
    assert!(a.is_empty());
}

#[test]
fn test_detect_junk_filtered() {
    let (c, _h, _a) = detect_copyrights_from_text("Copyright (c)");
    assert!(
        c.is_empty(),
        "Bare 'Copyright (c)' should be filtered as junk"
    );
}

#[test]
fn test_detect_filters_code_like_c_marker_lines() {
    let text = "(c) (const unsigned char*)ptr\n(c) c ? foo : bar\n(c) c & 0x3f\n(c) flags |= 0x80";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_windows_versioninfo_line_is_not_detected_as_copyright_or_holder() {
    let text = "Copyright (c) 2050 VALUE OriginalFilename NativeConsoleApp.exe";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);

    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_dtd_declaration_line_is_not_detected_as_copyright_or_holder() {
    let text = "copyright <!ELEMENT A ( PCDATA) > aaaa";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);

    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_copyright_property_access_line_is_not_detected_as_copyright_or_holder() {
    let text = "Copyright clone.Copyright.Text";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);

    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_unicode_escape_c_marker_line_is_not_detected_as_copyright_or_holder() {
    let text = "(c) HeaderType.Content u00AD u00AE";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);

    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_regexoptions_holder_token_is_not_detected() {
    let text = "RegexOptions.None";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_encoding_ascii_holder_token_is_not_detected() {
    let text = "Some-Header3 Encoding.ASCII";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_api_member_access_holder_token_is_not_detected() {
    let text = "FeedUtils.CloneTextContent";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_marshal_api_holder_token_is_not_detected() {
    let text = "Marshal.GetObjectForIUnknown(i) ComObject obj";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_console_writeline_holder_token_is_not_detected() {
    let text = "header Console.WriteLine $'.NET Xml Serialization Generation Utility";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_file_reference_prose_copyright_token_is_not_detected() {
    let text = "copyright and update THIRD-PARTY-NOTICES.TXT.";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_detect_copyright_does_not_absorb_unexpected_as_represented() {
    let text = "Copyright 1993 United States Government as represented by the\nDirector, National Security Agency.";
    let (c, h, _a) = detect_copyrights_from_text(text);
    assert!(
        c.iter()
            .any(|cr| cr.copyright == "Copyright 1993 United States Government"),
        "Should keep only government without continuation: {:?}",
        c
    );
    assert!(
        h.iter().any(|hd| hd.holder == "United States Government"),
        "Should keep only government holder without continuation: {:?}",
        h
    );
}

#[test]
fn test_doc_doc_no_overabsorb() {
    let input = "are copyrighted by Douglas C. Schmidt and his research group at Washington University, University of California, Irvine, and Vanderbilt University, Copyright (c) 1993-2008, all rights reserved.";
    let (c, _h, _a) = detect_copyrights_from_text(input);
    assert!(
        c.iter().any(|cr| cr.copyright == "copyrighted by Douglas C. Schmidt and his research group at Washington University, University of California, Irvine, and Vanderbilt University, Copyright (c) 1993-2008"),
        "Should merge trailing Copyright (c) clause, got: {:?}",
        c
    );
}

#[test]
fn test_meta_sdk_license_false_positive_detector_drops_legal_prose_fragments() {
    let input = concat!(
        "Copyright © Meta Platform Technologies, LLC and its affiliates. All rights reserved.\n",
        "1.2.5 alter, restrict, or interfere with the normal operation or functionality of the SDK, the MPT hardware or software, or MPT Approved Products, including, but not limited to: (a) the behavior of the “Meta Quest button” and “XBox button” implemented by the MPT system software; (b) any on-screen messages or information; (c) the behavior of the proximity sensor in the MPT hardware implemented by the MPT system software; (d) any MPT hardware or software security features; (e) any end user's settings; and (f) Health and Safety Warnings;\n",
        "1.3 Distribution and Sublicense Restrictions. The redistribution and sublicense rights under this Section are further subject to the following restrictions: (1) redistribution of sample source code or other materials must include the following copyright notice: “Copyright © Meta Platform Technologies, LLC and its affiliates. All rights reserved;” and (2) if the sample source code or other materials include a \"License\" or \"Notice\" text file, you must provide a copy of the License or Notice file with the sample code.\n",
        "3.1 Ownership. As between you and MPT, MPT and/or its affiliates or licensors own all rights, title, and interest, including all Intellectual Property Rights (defined below), in and to the SDK (including associated MPT content and sample code) and all derivatives thereof. MPT reserves all rights not expressly granted under the License. As between you and MPT, you and/or your licensors own all rights, title, and interest in and to your Application, (excluding our SDK), including all Intellectual Property Rights. “Intellectual Property Rights” means any and all worldwide rights under applicable laws of patent, copyright, trade secret, trademark, rights of publicity and privacy, and other proprietary rights.\n",
        "3.4 Brand Attribution. This Agreement does not grant you or any third party permission to use our trade names, trademarks, service marks, logos, domain names, and other distinctive brand features (collectively, “Brand Features”) except as required for reasonable and customary use in describing the origin of the SDK or reproduction of the copyright notice as required under the License grant.\n",
        "7.4.2 If you reside in the US or your business is located in the US: You and we agree to arbitrate any claim, cause of action, or dispute between you and us that arises out of or relates to any access or use of the SDK for business or commercial purposes (“commercial claim”). This provision does not cover any commercial claims relating to violations of your or our intellectual property rights, including, but not limited to, copyright infringement, patent infringement, trademark infringement, violations of the brand guidelines, violations of your or our confidential information or trade secrets, or efforts to interfere with our products or engage with our products in unauthorized ways (for example, automated ways).\n",
    );

    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);
    let copyright_values: Vec<&str> = copyrights.iter().map(|c| c.copyright.as_str()).collect();
    let holder_values: Vec<&str> = holders.iter().map(|h| h.holder.as_str()).collect();

    assert!(
        copyright_values
            .contains(&"Copyright (c) Meta Platform Technologies, LLC and its affiliates"),
        "copyrights: {copyright_values:#?}"
    );
    assert!(
        holder_values.contains(&"Meta Platform Technologies, LLC and its affiliates"),
        "holders: {holder_values:#?}"
    );
    assert_eq!(
        copyright_values
            .iter()
            .filter(|value| **value
                == "Copyright (c) Meta Platform Technologies, LLC and its affiliates")
            .count(),
        1,
        "copyrights: {copyright_values:#?}"
    );
    assert_eq!(
        holder_values
            .iter()
            .filter(|value| **value == "Meta Platform Technologies, LLC and its affiliates")
            .count(),
        1,
        "holders: {holder_values:#?}"
    );
    assert!(
        !copyright_values
            .iter()
            .any(|value| value
                .contains("rights of publicity and privacy, and other proprietary rights")),
        "copyrights: {copyright_values:#?}"
    );
    for unexpected in [
        "as required",
        "infringement, patent infringement, trademark infringement, violations of the brand guidelines, violations of your or our confidential",
        "the behavior of the proximity sensor in the MPT hardware implemented by the MPT system software",
        "Copyright",
    ] {
        assert!(
            !holder_values.contains(&unexpected),
            "unexpected holder {unexpected:?} in {holder_values:#?}"
        );
    }
}

#[test]
fn test_dnf_copr_command_does_not_produce_copyright() {
    let text = concat!(
        "If you used restic from copr previously, remove the copr repo as follows:\n",
        "   $ dnf copr remove copart/restic\n",
        "For RHEL7/CentOS there is a copr repository available:\n",
        "    $ yum copr enable copart/restic\n",
    );
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_bash_array_expansion_does_not_produce_copyright() {
    let text = concat!(
        "if __restic_contains_word '${words[c]}' '${two_word_flags[@]}'; then\n",
        "elif __restic_contains_word '${words[c]}' '${must_have_one_noun[@]}'; then\n",
        "elif __restic_contains_word '${words[c]}' '${commands[@]}'; then\n",
        "elif __restic_contains_word '${words[c]}' '${command_aliases[@]}'; then\n",
    );
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_author_of_work_does_not_produce_author() {
    let text = "We've added the ability to use rclone to store backup data on all\nbackends that it supports. This was done in collaboration with\nNick, the author of rclone.\n";
    let (copyrights, holders, authors) = detect_copyrights_from_text(text);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:?}");
    assert!(holders.is_empty(), "holders: {holders:?}");
    assert!(
        !authors.iter().any(|a| a.author == "rclone"),
        "authors should not contain 'rclone': {authors:?}"
    );
}

#[test]
fn test_copyright_holder_placeholder_and_code_fragments_do_not_emit_detections() {
    let input = concat!(
        "Copyright (c) 2014 PulseAudio's COPYRIGHT HOLDER
",
        "PulseAudio's COPYRIGHT HOLDER
",
        "copyright sections were added
",
        "pa_log_debug(\"Copyright: %s\", d->Copyright)\n",
        "PA_REFCNT_INIT(c); c->core = core
",
        "applying to the plugin. If
",
        "applies the
",
        "s d- Copyright
",
        "c- core core
",
    );
    let (copyrights, holders, authors) = detect_copyrights_from_text(input);
    assert!(copyrights.is_empty(), "copyrights: {copyrights:#?}");
    assert!(holders.is_empty(), "holders: {holders:#?}");
    assert!(authors.is_empty(), "authors: {authors:#?}");
}

#[test]
fn test_pulseaudio_ladspa_rfc_and_contact_fragments_do_not_emit_junk() {
    let input = concat!(
        "copyright applying to the plugin. If
",
        "sections were added
",
        "Copyright 2009 Nokia Corporation Contact: Maemo Multimedia <multimedia@maemo.org>
",
    );
    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);
    let copyright_values: Vec<&str> = copyrights.iter().map(|c| c.copyright.as_str()).collect();
    let holder_values: Vec<&str> = holders.iter().map(|h| h.holder.as_str()).collect();
    assert!(
        copyright_values.contains(&"Copyright 2009 Nokia Corporation"),
        "copyrights: {copyright_values:?}"
    );
    assert!(
        !copyright_values
            .iter()
            .any(|v| v.contains("applying to the plugin")
                || v.contains("sections were added")
                || v.contains("Contact:")),
        "copyrights: {copyright_values:?}"
    );
    assert!(
        holder_values.contains(&"Nokia Corporation"),
        "holders: {holder_values:?}"
    );
    assert!(
        !holder_values
            .iter()
            .any(|v| v.contains("sections were added") || v.contains("Contact:")),
        "holders: {holder_values:?}"
    );
}

#[test]
fn test_xml_copyright_element_tag_is_not_detected_as_copyright() {
    // Wayland protocol descriptions (glfw's vendored deps/wayland/*.xml) wrap
    // notices in a `<copyright>` element. The bare tag opener must not itself be
    // surfaced as a copyright/holder, while the real notices inside it are kept.
    let input = concat!(
        "  <copyright>\n",
        "    Copyright \u{00a9} 2014 Jonas \u{00c5}dahl\n",
        "  </copyright>\n",
    );
    let (copyrights, holders, _authors) = detect_copyrights_from_text(input);
    assert!(
        !copyrights.iter().any(|c| c.copyright == "<copyright>"),
        "bare XML tag leaked into copyrights: {copyrights:?}"
    );
    assert!(
        !holders.iter().any(|h| h.holder == "<copyright>"),
        "bare XML tag leaked into holders: {holders:?}"
    );
    assert!(
        copyrights.iter().any(|c| c.copyright.contains("Jonas")),
        "expected the real notice to survive: {copyrights:?}"
    );
}

#[test]
fn test_javadoc_author_with_html_anchor_is_extracted() {
    // Java Javadoc `@author` tags frequently wrap the name in an HTML anchor with
    // an http(s) href (eclipse-vertx/vert.x uses
    // `@author <a href="http://tfox.org">Tim Fox</a>` across hundreds of files).
    // The href is a homepage link, not the name; the author resolves to the name.
    for (text, expected) in [
        (
            " * @author <a href=\"http://tfox.org\">Tim Fox</a>",
            "Tim Fox",
        ),
        (
            " * @author <a href=\"https://github.com/cescoffier\">Clement Escoffier</a>",
            "Clement Escoffier",
        ),
        // Plain-text name followed by a trailing handle/homepage anchor.
        (
            " * @author Francesco Guardiani <a href=\"https://slinkydeveloper.github.io/\">@slinkydeveloper</a>",
            "Francesco Guardiani",
        ),
    ] {
        let (_c, _h, authors) = detect_copyrights_from_text(text);
        let vals: Vec<&String> = authors.iter().map(|a| &a.author).collect();
        assert!(
            vals.iter().any(|a| a.as_str() == expected),
            "expected {expected:?} in {vals:?} for {text:?}"
        );
    }
}
