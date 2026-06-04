// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use super::*;

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
