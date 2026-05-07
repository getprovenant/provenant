// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;

// ── debug tests ──────────────────────────────────────────────────

#[test]
fn test_strip_trailing_original_authors() {
    assert_eq!(
        strip_trailing_original_authors("copyright by the original authors"),
        "copyright by the original"
    );
    assert_eq!(
        strip_trailing_original_authors("the original authors"),
        "the original"
    );
    assert_eq!(
        strip_trailing_original_authors("(c) by the respective authors"),
        "(c) by the respective authors",
        "should not strip 'respective authors'"
    );
    assert_eq!(
        strip_trailing_original_authors("Copyright (c) 2007-2010 the original author or authors"),
        "Copyright (c) 2007-2010 the original author or authors",
        "should not strip 'author or authors'"
    );
    assert_eq!(
        refine_holder("the original authors"),
        Some("the original".to_string())
    );
    assert_eq!(
        refine_copyright("copyright by the original authors"),
        Some("copyright by the original".to_string())
    );
}

#[test]
fn test_refine_copyright_preserves_portions_created_by_prefix() {
    let refined = refine_copyright(
            "Portions created by the Initial Developer are Copyright (C) 1998-2000 the Initial Developer.",
        )
        .unwrap();
    assert_eq!(
        refined,
        "Portions created by the Initial Developer are Copyright (C) 1998-2000 the Initial Developer",
        "refined={refined:?}"
    );
}

#[test]
fn test_refine_copyright_strips_leading_author_label() {
    assert_eq!(
        refine_copyright("author Vlad Roubtsov, (c) 2004"),
        Some("Vlad Roubtsov, (c) 2004".to_string())
    );
}

#[test]
fn test_refine_copyright_keeps_year_only_line() {
    assert_eq!(
        refine_copyright("Copyright 2000"),
        Some("Copyright 2000".to_string())
    );
}

#[test]
fn test_refine_copyright_preserves_holder_obfuscated_email_after_dash() {
    assert_eq!(
        refine_copyright("Copyright (c) 2005, 2006 Nick Galbreath -- nickg at modp dot com"),
        Some("Copyright (c) 2005, 2006 Nick Galbreath - nickg at modp dot com".to_string()),
    );
    assert_eq!(
        refine_copyright("Copyright (c) 2005, 2006 Nick Galbreath - nickg at modp dot com"),
        Some("Copyright (c) 2005, 2006 Nick Galbreath - nickg at modp dot com".to_string()),
    );
}

#[test]
fn test_refine_author_discards_laboriously_took_the_trouble_junk() {
    assert_eq!(
        refine_author(
            "the authors laboriously took the trouble of searching for workarounds to make these compilers happy"
        ),
        None
    );
}

#[test]
fn test_refine_author_drops_generic_role_and_prose_fragments() {
    assert_eq!(refine_author("Philip"), None);
    assert_eq!(refine_author("john"), None);
    assert_eq!(refine_author("chunchu"), Some("chunchu".to_string()));
    assert_eq!(refine_author("chef-client"), None);
    assert_eq!(refine_author("compatible"), None);
    assert_eq!(refine_author("desired"), None);
    assert_eq!(refine_author("document"), None);
    assert_eq!(refine_author("homepage"), None);
    assert_eq!(refine_author("Package Author"), None);
    assert_eq!(refine_author("otherwise"), None);
    assert_eq!(refine_author("performing"), None);
    assert_eq!(refine_author("review"), None);
    assert_eq!(refine_author("reviewer"), None);
    assert_eq!(refine_author("volunteers"), None);
    assert_eq!(refine_author("Automatically generated"), None);
    assert_eq!(refine_author("Guide"), None);
    assert_eq!(refine_author("maintainers with write access"), None);
    assert_eq!(refine_author("schedule and monitor workflows"), None);
    assert_eq!(refine_author("for the sample crypto project"), None);
    assert_eq!(
        refine_author("the pkg-bazaar team"),
        Some("the pkg-bazaar team".to_string())
    );
    assert_eq!(
        refine_author("the University of California, Berkeley and its contributors"),
        Some("the University of California, Berkeley and its contributors".to_string())
    );
    assert_eq!(
        refine_author("the National Center for Supercomputing Applications at the University of Illinois at Urbana-Champaign"),
        Some("the National Center for Supercomputing Applications at the University of Illinois at Urbana-Champaign".to_string())
    );
    assert_eq!(
        refine_author(
            "transition .transition https://github.com/d3/d3-transition/blob/master/README.md"
        ),
        None
    );
    assert_eq!(
        refine_author("Daniel Vaz Gaspar (https://github.com/dpgaspar/Flask-AppBuilder)"),
        Some("Daniel Vaz Gaspar (https://github.com/dpgaspar/Flask-AppBuilder)".to_string())
    );
    assert_eq!(
        refine_author("Daniel Vaz Gaspar"),
        Some("Daniel Vaz Gaspar".to_string())
    );
    assert_eq!(refine_author("the DTD (see Section 13.3).</p>"), None);
    assert_eq!(refine_author("distribute Contributors"), None);
    assert_eq!(refine_author("If fixing it requires an API"), None);
    assert_eq!(
        refine_author("Flutter and Dart have told us they plan to work contributors"),
        None
    );
    assert_eq!(refine_author("Requires translation"), None);
    assert_eq!(refine_author("Autor: author"), None);
    assert_eq!(refine_author("Auctor: author"), None);
    assert_eq!(refine_author("See AUTHORS file"), None);
    assert_eq!(
        refine_author("Author: Jane Doe"),
        Some("Jane Doe".to_string())
    );
}

#[test]
fn test_refine_author_truncates_trailing_prose_after_contact() {
    assert_eq!(
        refine_author("Mark Brown <broonie@sirena.org.uk>. The -d tempdir option"),
        Some("Mark Brown <broonie@sirena.org.uk>".to_string())
    );
    assert_eq!(
        refine_author("Ryan Haksi (//cryogen@infoserve.net) I need random access"),
        Some("Ryan Haksi (//cryogen@infoserve.net)".to_string())
    );
    assert_eq!(
        refine_author("Jean-Loup Gailly <gzip@prep.ai.mit.edu> . Since this"),
        Some("Jean-Loup Gailly <gzip@prep.ai.mit.edu>".to_string())
    );
}

#[test]
fn test_refine_author_preserves_full_written_by_author_list() {
    assert_eq!(
        refine_author("Jean-Marc Valin, Gregory Maxwell, and Timothy B. Terriberry"),
        Some("Jean-Marc Valin, Gregory Maxwell, and Timothy B. Terriberry".to_string())
    );
}

#[test]
fn test_refine_holder_discards_symbol_table_run_junk() {
    assert_eq!(
        refine_holder("(r), & 175, & 176, & 177, & 178, & 179, & 180, & 181, & 182, & 183"),
        None
    );
}

#[test]
fn test_refine_holder_drops_authors_file_reference_note() {
    assert_eq!(refine_holder("See AUTHORS file"), None);
}

#[test]
fn test_refine_holder_drops_document_form_reference_noise() {
    assert_eq!(refine_holder("Office FL-108"), None);
}

// ── strip_some_punct ─────────────────────────────────────────────

#[test]
fn test_strip_some_punct_basic() {
    assert_eq!(strip_some_punct(",hello,"), "hello");
}

#[test]
fn test_strip_some_punct_leading_dot() {
    assert_eq!(strip_some_punct(".hello"), "hello");
}

#[test]
fn test_strip_some_punct_trailing_paren() {
    assert_eq!(strip_some_punct("hello("), "hello");
}

#[test]
fn test_strip_some_punct_empty() {
    assert_eq!(strip_some_punct(""), "");
}

// ── strip_trailing_period ────────────────────────────────────────

#[test]
fn test_strip_trailing_period_normal() {
    assert_eq!(strip_trailing_period("Hello World."), "Hello World");
}

#[test]
fn test_strip_trailing_period_inc() {
    assert_eq!(strip_trailing_period("Acme Inc."), "Acme Inc.");
}

#[test]
fn test_strip_trailing_period_ltd() {
    assert_eq!(strip_trailing_period("Foo Ltd."), "Foo Ltd.");
}

#[test]
fn test_strip_trailing_period_acronym() {
    // "e.V." — second-to-last is uppercase, multi-word
    assert_eq!(strip_trailing_period("Foo e.V."), "Foo e.V.");
}

#[test]
fn test_strip_trailing_period_short_acronym() {
    // "b.v." — third-to-last is a period
    assert_eq!(strip_trailing_period("Foo b.v."), "Foo b.v.");
}

#[test]
fn test_strip_trailing_period_no_period() {
    assert_eq!(strip_trailing_period("Hello"), "Hello");
}

#[test]
fn test_strip_trailing_period_short() {
    assert_eq!(strip_trailing_period("P."), "P.");
}

#[test]
fn test_strip_trailing_period_empty() {
    assert_eq!(strip_trailing_period(""), "");
}

// ── strip_leading_numbers ────────────────────────────────────────

#[test]
fn test_strip_leading_numbers_basic() {
    assert_eq!(strip_leading_numbers("123 456 Hello"), "Hello");
}

#[test]
fn test_strip_leading_numbers_no_numbers() {
    assert_eq!(strip_leading_numbers("Hello World"), "Hello World");
}

#[test]
fn test_strip_leading_numbers_all_numbers() {
    assert_eq!(strip_leading_numbers("123 456"), "");
}

// ── strip_prefixes / strip_suffixes ──────────────────────────────

#[test]
fn test_strip_prefixes_basic() {
    let prefixes: HashSet<&str> = ["by", "and"].into_iter().collect();
    assert_eq!(strip_prefixes("by and John Doe", &prefixes), "John Doe");
}

#[test]
fn test_strip_suffixes_basic() {
    let suffixes: HashSet<&str> = [".", ",", "and"].into_iter().collect();
    assert_eq!(strip_suffixes("John Doe and", &suffixes), "John Doe");
}

// ── strip_unbalanced_parens ──────────────────────────────────────

#[test]
fn test_strip_unbalanced_parens_balanced() {
    assert_eq!(
        strip_unbalanced_parens("This is a super(c) string", '(', ')'),
        "This is a super(c) string"
    );
}

#[test]
fn test_strip_unbalanced_parens_unbalanced_close() {
    assert_eq!(
        strip_unbalanced_parens("This )(is a super(c) string)(", '(', ')'),
        "This  (is a super(c) string) "
    );
}

#[test]
fn test_strip_unbalanced_parens_lone_open() {
    assert_eq!(strip_unbalanced_parens("This ( is", '(', ')'), "This   is");
}

#[test]
fn test_strip_unbalanced_parens_lone_close() {
    assert_eq!(strip_unbalanced_parens("This ) is", '(', ')'), "This   is");
}

#[test]
fn test_strip_unbalanced_parens_single_open() {
    assert_eq!(strip_unbalanced_parens("(", '(', ')'), " ");
}

#[test]
fn test_strip_unbalanced_parens_single_close() {
    assert_eq!(strip_unbalanced_parens(")", '(', ')'), " ");
}

// ── strip_solo_quotes ────────────────────────────────────────────

#[test]
fn test_strip_solo_quotes_url() {
    assert_eq!(
        strip_solo_quotes("https://example.com/'"),
        "https://example.com/"
    );
}

#[test]
fn test_strip_solo_quotes_paren() {
    assert_eq!(strip_solo_quotes("foo)'"), "foo)");
}

// ── remove_dupe_copyright_words ──────────────────────────────────

#[test]
fn test_remove_dupe_spdx() {
    let result = remove_dupe_copyright_words("SPDX-FileCopyrightText 2024 Acme");
    assert_eq!(result, "Copyright 2024 Acme");
}

#[test]
fn test_remove_dupe_double_copyright() {
    let result = remove_dupe_copyright_words("Copyright Copyright 2024 Acme");
    assert_eq!(result, "Copyright 2024 Acme");
}

#[test]
fn test_remove_dupe_cppyright() {
    let result = remove_dupe_copyright_words("Cppyright 2024 Acme");
    assert_eq!(result, "Copyright 2024 Acme");
}

// ── remove_some_extra_words_and_punct ─────────────────────────────

#[test]
fn test_remove_extra_words_html() {
    let result = remove_some_extra_words_and_punct("<p>Hello</a>");
    assert_eq!(result, "Hello");
}

#[test]
fn test_remove_extra_words_mailto() {
    let result = remove_some_extra_words_and_punct("mailto:foo@bar.com");
    assert_eq!(result, "foo@bar.com");
}

#[test]
fn test_remove_extra_words_as_represented_by() {
    let result = remove_some_extra_words_and_punct("Acme Corp as represented by");
    assert_eq!(result, "Acme Corp as represented by");
}

// ── is_junk_copyright ────────────────────────────────────────────

#[test]
fn test_is_junk_copyright_bare_c() {
    assert!(is_junk_copyright("(c)"));
}

#[test]
fn test_is_junk_copyright_bare_copyright_c() {
    assert!(is_junk_copyright("Copyright (c)"));
}

#[test]
fn test_is_junk_copyright_normal() {
    assert!(!is_junk_copyright("Copyright 2024 Acme Inc."));
}

#[test]
fn test_is_junk_copyright_holder_or_simply() {
    assert!(is_junk_copyright("copyright holder or simply foo"));
}

#[test]
fn test_is_junk_copyright_patents_trade_secrets() {
    assert!(is_junk_copyright("copyrights, patents, trade secrets or"));
    assert!(is_junk_copyright(
        "copyright, patent, trademark, and attribution"
    ));
    assert!(is_junk_copyright(
        "copyright, including without limitation by United States"
    ));
    assert!(is_junk_copyright("COPYRIGHTS, TRADEMARKS OR"));
    assert!(is_junk_copyright("COPYRIGHT, TRADEMARK, TRADE SECRET OR"));
    assert!(is_junk_copyright("copyright, to do the following"));
}

#[test]
fn test_is_junk_copyright_trade_secrets_fragments() {
    assert!(is_junk_copyright("copyrights, trade secrets or"));
    assert!(is_junk_copyright("COPYRIGHT, TRADE SECRET OR"));
    assert!(is_junk_copyright("copyright and trade secret"));
    assert!(is_junk_copyright("COPYRIGHT AND TRADE SECRETS"));
    assert!(is_junk_copyright(
        "copyright, trade secret, trademark or other intellectual property rights of"
    ));
    assert!(is_junk_copyright("COPYRIGHT (c) TRADEMARK"));
}

#[test]
fn test_is_junk_copyright_all_caps_placeholders() {
    assert!(is_junk_copyright(
        "Copyright (c) 1999-2008 MODULEAUTHOR endif"
    ));
}

#[test]
fn test_is_junk_copyright_proprietary() {
    assert!(is_junk_copyright("copyright, proprietary"));
    assert!(is_junk_copyright("copyright proprietary"));
    assert!(is_junk_copyright("proprietary"));
}

#[test]
fn test_is_junk_copyright_rsa() {
    assert!(is_junk_copyright("Copyright RSA"));
    assert!(is_junk_copyright("copyright rsa"));
}

#[test]
fn test_is_junk_copyright_single_letter_holder_noise() {
    assert!(is_junk_copyright("copyright p"));
}

#[test]
fn test_is_junk_copyright_math_c_variable() {
    assert!(is_junk_copyright("(c) Convert Chebyshev"));
    assert!(is_junk_copyright("(c) Multiply a Chebyshev"));
}

#[test]
fn test_is_junk_copyright_c_cast_ternary_and_bitwise_patterns() {
    assert!(is_junk_copyright("(c) (const unsigned char*)ptr"));
    assert!(is_junk_copyright("(c) c ? foo : bar"));
    assert!(is_junk_copyright("(c) c & 0x3f"));
    assert!(is_junk_copyright("(c) flags |= 0x80"));
}

#[test]
fn test_is_junk_copyright_year_only() {
    assert!(!is_junk_copyright("Copyright (c) 2003"));
    assert!(!is_junk_copyright("Copyright (C) 1995"));
    assert!(!is_junk_copyright("Copyright 2003"));
    assert!(!is_junk_copyright("(c) 2003"));
}

#[test]
fn test_is_junk_copyright_scan_phrase() {
    assert!(is_junk_copyright(
        "Measures the end-to-end composer copyright scan"
    ));
}

#[test]
fn test_is_junk_copyright_c_sign_path_fragment() {
    assert!(is_junk_copyright("(c) Ljoptsimple/AbstractOptionSpec"));
}

// ── refine_copyright ─────────────────────────────────────────────

#[test]
fn test_refine_copyright_basic() {
    let result = refine_copyright("Copyright 2024 Acme Inc.");
    assert_eq!(result, Some("Copyright 2024 Acme Inc.".to_string()));
}

#[test]
fn test_refine_copyright_empty() {
    assert_eq!(refine_copyright(""), None);
}

#[test]
fn test_refine_copyright_keeps_confidential_and_proprietary_phrase() {
    let result =
        refine_copyright("(c) Example Corp. and affiliates. Confidential and proprietary.");
    assert_eq!(
        result,
        Some("(c) Example Corp. and affiliates. Confidential and proprietary".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_authors_file_reference_clause() {
    assert_eq!(
        refine_copyright("Copyright 2015 See AUTHORS file"),
        Some("Copyright 2015".to_string())
    );
}

#[test]
fn test_refine_copyright_drops_document_form_reference_noise() {
    assert_eq!(refine_copyright("Copyright Office FL-108"), None);
}

#[test]
fn test_refine_copyright_strips_junk_prefix() {
    let result = refine_copyright("by Copyright 2024 Acme");
    assert_eq!(result, Some("Copyright 2024 Acme".to_string()));
}

#[test]
fn test_refine_copyright_removes_space_before_comma() {
    let result = refine_copyright("Copyright (c) Free Software Foundation, Inc. , 2006");
    assert_eq!(
        result,
        Some("Copyright (c) Free Software Foundation, Inc., 2006".to_string())
    );
}

#[test]
fn test_refine_copyright_removes_space_before_internal_commas() {
    let result = refine_copyright("Copyright (c) 1989 , 1991 Free Software Foundation , Inc.");
    assert_eq!(
        result,
        Some("Copyright (c) 1989, 1991 Free Software Foundation, Inc.".to_string())
    );
}

#[test]
fn test_normalize_angle_bracket_comma_spacing_email() {
    assert_eq!(
        normalize_angle_bracket_comma_spacing("Acme <dev@acme.test>, Foo"),
        "Acme <dev@acme.test>, Foo"
    );
}

#[test]
fn test_normalize_angle_bracket_comma_spacing_non_email_tag_unchanged() {
    assert_eq!(
        normalize_angle_bracket_comma_spacing("Acme </p>, Foo"),
        "Acme </p>, Foo"
    );
    assert_eq!(
        normalize_angle_bracket_comma_spacing("Acme <www.example.com>, Foo"),
        "Acme <www.example.com>, Foo"
    );
}

#[test]
fn test_refine_copyright_normalizes_angle_bracket_email_comma_spacing() {
    let result = refine_copyright("Copyright 2024 Acme <dev@acme.test>, Foo");
    assert_eq!(
        result,
        Some("Copyright 2024 Acme <dev@acme.test>, Foo".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_x509_dn_fields_after_holder() {
    let result = refine_copyright(
        "Copyright (c) 1997 Microsoft Corp., OU Microsoft Corporation, CN Microsoft Root",
    );
    assert_eq!(
        result,
        Some("Copyright (c) 1997 Microsoft Corp.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_x509_dn_fields_after_ou() {
    let result = refine_copyright(
        "Copyright (c) 2005, OU OISTE Foundation Endorsed, CN OISTE WISeKey Global Root",
    );
    assert_eq!(
        result,
        Some("Copyright (c) 2005, OU OISTE Foundation".to_string())
    );
}

#[test]
fn test_refine_copyright_removes_space_before_comma_after_c_sign() {
    let result = refine_copyright("Copyright (c) , 2001-2011, Omega Tech. Co., Ltd.");
    assert_eq!(
        result,
        Some("Copyright (c), 2001-2011, Omega Tech. Co., Ltd.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_portions_of_fragment() {
    let result =
        refine_copyright("Copyright (c) 1991, 1999 Free Software Foundation, Inc. Portions of");
    assert_eq!(
        result,
        Some("Copyright (c) 1991, 1999 Free Software Foundation, Inc.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_dot_software() {
    let result = refine_copyright(
        "Copyright (c) Ian F. Darwin 1986, 1987, 1989, 1990, 1991, 1992, 1994, 1995. Software",
    );
    assert_eq!(
        result,
        Some(
            "Copyright (c) Ian F. Darwin 1986, 1987, 1989, 1990, 1991, 1992, 1994, 1995"
                .to_string()
        )
    );
}

#[test]
fn test_refine_copyright_strips_trailing_some_parts_of_fragment() {
    let result = refine_copyright(
        "copyright (c) 2012 The FreeType Project (www.freetype.org). Some parts of",
    );
    assert_eq!(
        result,
        Some("copyright (c) 2012 The FreeType Project (www.freetype.org)".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_angle_bracketed_www_domain_without_by() {
    let result = refine_copyright("Copyright (C) 2012 Altera <www.altera.com>");
    assert_eq!(result, Some("Copyright (C) 2012 Altera".to_string()));
}

#[test]
fn test_refine_copyright_keeps_angle_bracketed_www_domain_with_by() {
    let result = refine_copyright("Copyright 2011 by BitRouter <www.BitRouter.com>");
    assert_eq!(
        result,
        Some("Copyright 2011 by BitRouter <www.BitRouter.com>".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_comma_delimited_www_domain_clause() {
    let result = refine_copyright(
        "(c) Copyright 2004 Texas Instruments, <www.ti.com> Richard Woodruff <r-woodruff2@ti.com>",
    );
    assert_eq!(
        result,
        Some(
            "(c) Copyright 2004 Texas Instruments, Richard Woodruff <r-woodruff2@ti.com>"
                .to_string()
        )
    );
}

#[test]
fn test_refine_copyright_strips_trailing_mountain_view_ca() {
    let result = refine_copyright("Copyright 1993 by Sun Microsystems, Inc. Mountain View, CA.");
    assert_eq!(
        result,
        Some("Copyright 1993 by Sun Microsystems, Inc. Mountain View".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_comma_with_unicode_whitespace() {
    let result = refine_copyright("(c) by the respective authors,\u{00A0}");
    assert_eq!(result, Some("(c) by the respective authors".to_string()));
}

#[test]
fn test_refine_copyright_strips_trailing_paren_email_after_c_by() {
    let result = refine_copyright("(c) by Monty (xiphmont@mit.edu)");
    assert_eq!(result, Some("(c) by Monty".to_string()));
}

#[test]
fn test_refine_copyright_strips_trailing_division_of_company_suffix() {
    let input = "Copyright (c) 2006, Industrial Light & Magic, a division of Lucasfilm Entertainment Company Ltd.";
    assert_eq!(
        refine_copyright(input),
        Some("Copyright (c) 2006, Industrial Light & Magic".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_independent_jpeg_group_software_tail() {
    let result = refine_copyright(
        "(c) 1991-1992, Thomas G. Lane, Part of the Independent JPEG Group's software.",
    );
    assert_eq!(
        result,
        Some("(c) 1991-1992, Thomas G. Lane, Part of the Independent JPEG Group's".to_string())
    );
}

#[test]
fn test_refine_copyright_keeps_plain_email_after_comma() {
    let result = refine_copyright("Parts (c) 1999 David Airlie, airlied@linux.ie");
    assert_eq!(
        result,
        Some("Parts (c) 1999 David Airlie, airlied@linux.ie".to_string())
    );
}

#[test]
fn test_refine_copyright_keeps_year_range_angle_email_suffix() {
    let result = refine_copyright(
        "Copyright (c) 2021-2023 Sebastian Ramacher <sebastian.ramacher@ait.ac.at>",
    );
    assert_eq!(
        result,
        Some(
            "Copyright (c) 2021-2023 Sebastian Ramacher <sebastian.ramacher@ait.ac.at>".to_string()
        )
    );
}

#[test]
fn test_refine_copyright_strips_trailing_by_person_after_holder() {
    let result = refine_copyright(
        "Copyright (C) 2004 Nokia Corporation by Tony Lindrgen <tony@atomide.com>",
    );
    assert_eq!(
        result,
        Some("Copyright (C) 2004 Nokia Corporation".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_fsf_address_tail() {
    let result = refine_copyright(
        "Copyright (c) 1989 Free Software Foundation, Inc. 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA",
    );
    assert_eq!(
        result,
        Some("Copyright (c) 1989 Free Software Foundation, Inc.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_fsf_675_mass_ave_tail() {
    let result = refine_copyright(
        "Copyright (c) 1989 Free Software Foundation, Inc. 675 Mass Ave, Cambridge, MA",
    );
    assert_eq!(
        result,
        Some("Copyright (c) 1989 Free Software Foundation, Inc.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_sun_address_tail() {
    let result = refine_copyright(
        "Copyright 1997, 1998 by Sun Microsystems, Inc., 901 San Antonio Road, Palo Alto, California, 94303, U.S.A.",
    );
    assert_eq!(
        result,
        Some("Copyright 1997, 1998 by Sun Microsystems, Inc.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_realnetworks_address_tail() {
    let result = refine_copyright(
        "Copyright (c) 1995-2002 RealNetworks, Inc. and/or its suppliers. 2601 Elliott Avenue, Suite 1000, Seattle, Washington 98121 U.S.A.",
    );
    assert_eq!(
        result,
        Some("Copyright (c) 1995-2002 RealNetworks, Inc.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_and_or_its_suppliers_tail() {
    let result =
        refine_copyright("Copyright (c) 1995-2002 RealNetworks, Inc. and/or its suppliers");
    assert_eq!(
        result,
        Some("Copyright (c) 1995-2002 RealNetworks, Inc.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_write_to_fsf_tail() {
    let result = refine_copyright(
        "copyrighted by the Free Software Foundation, write to the Free Software Foundation we sometimes make exceptions for",
    );
    assert_eq!(
        result,
        Some("copyrighted by the Free Software Foundation".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_their_notice_reproduced_below_tail() {
    let result = refine_copyright(
        "parts (c) RSA Data Security, Inc. Their notice reproduced below in its entirety",
    );
    assert_eq!(
        result,
        Some("parts (c) RSA Data Security, Inc.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_license_name() {
    let result = refine_copyright(
        "(c) Copyright 2009 Hewlett-Packard Development Company, L.P. GNU GENERAL PUBLIC LICENSE",
    );
    assert_eq!(
        result,
        Some("(c) Copyright 2009 Hewlett-Packard Development Company, L.P.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_doc_generated_by() {
    let result = refine_copyright(
        "(c) Copyright 2010 by the http://wtforms.simplecodes.com WTForms Team, documentation generated by http://sphinx.pocoo.org/ Sphinx",
    );
    assert_eq!(
        result,
        Some("(c) Copyright 2010 by the http://wtforms.simplecodes.com WTForms Team".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_dash_software() {
    let result =
        refine_copyright("copyright (c) 1999, IBM Corporation., http://www.ibm.com. - software");
    assert_eq!(
        result,
        Some("copyright (c) 1999, IBM Corporation., http://www.ibm.com".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_et_al() {
    let result =
        refine_copyright("Copyright (c) 1998-2001, Daniel Stenberg, <daniel@haxx.se> , et al");
    assert_eq!(
        result,
        Some("Copyright (c) 1998-2001, Daniel Stenberg, <daniel@haxx.se>".to_string())
    );
}

#[test]
fn test_refine_copyright_drops_bare_copyrighted_software_phrase() {
    assert_eq!(refine_copyright("copyrighted software"), None);
}

#[test]
fn test_is_junk_copyright_template_placeholders() {
    let refined = refine_copyright("Copyright 2014-$ date.year pkg.author").unwrap();
    assert!(is_junk_copyright(&refined));

    let refined = refine_copyright("Copyright (c) 2019 pkg.author").unwrap();
    assert!(is_junk_copyright(&refined));

    let refined = refine_copyright("Copyright (c) 2012 pkg.author pkg.homepage").unwrap();
    assert!(is_junk_copyright(&refined));

    let refined = refine_copyright("(c) 2004-2010 year .format YYYY-MM-DD, -04").unwrap();
    assert!(is_junk_copyright(&refined));

    let refined = refine_copyright("Copyright 2010- < pkg.author >").unwrap();
    assert!(is_junk_copyright(&refined));
}

#[test]
fn test_strip_some_punct_trailing_comma() {
    assert_eq!(
        strip_some_punct("copyright Free Software Foundation,"),
        "copyright Free Software Foundation"
    );
    assert_eq!(
        refine_copyright("copyright Free Software Foundation , and is licensed under the"),
        Some("copyright Free Software Foundation".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_generated_tag() {
    let result = refine_copyright("Copyright (c) 2024 Acme Corp. @generated by protobuf");
    assert_eq!(result, Some("Copyright (c) 2024 Acme Corp.".to_string()));
}

#[test]
fn test_normalize_comma_spacing_normalizes_space_before_comma() {
    assert_eq!(
        normalize_comma_spacing("Stephan Mueller , Design"),
        "Stephan Mueller, Design"
    );
    assert_eq!(
        normalize_comma_spacing("Free Software Foundation , Inc."),
        "Free Software Foundation, Inc."
    );
    assert_eq!(normalize_comma_spacing("1989 , 1991"), "1989, 1991");
}

#[test]
fn test_truncate_trailing_boilerplate_baslerstr_address() {
    assert_eq!(
        refine_holder("SVOX AG, Baslerstr. 30, 8048 Zuerich, Switzerland"),
        Some("SVOX AG, Baslerstr. 30, 8048 Zuerich, Switzerland".to_string())
    );
    assert_eq!(
        refine_copyright(
            "Copyright (c) 2008-2009 SVOX AG, Baslerstr. 30, 8048 Zuerich, Switzerland",
        ),
        Some(
            "Copyright (c) 2008-2009 SVOX AG, Baslerstr. 30, 8048 Zuerich, Switzerland".to_string(),
        )
    );
}

#[test]
fn test_truncate_trailing_boilerplate_begin_license_block() {
    assert_eq!(
        refine_holder("Google Inc BEGIN LICENSE BLOCK"),
        Some("Google Inc".to_string())
    );
    assert_eq!(
        refine_copyright("Copyright (c) 2011 Google Inc BEGIN LICENSE BLOCK"),
        Some("Copyright (c) 2011 Google Inc".to_string())
    );
}

#[test]
fn test_strip_trailing_isc_after_inc() {
    assert_eq!(
        refine_holder("Internet Systems Consortium, Inc. ISC"),
        Some("Internet Systems Consortium, Inc.".to_string())
    );
    assert_eq!(
        refine_copyright("Copyright (c) 2004,2007 by Internet Systems Consortium, Inc. ISC"),
        Some("Copyright (c) 2004,2007 by Internet Systems Consortium, Inc.".to_string())
    );
}

#[test]
fn test_refine_holder_drops_notice_disclaimer_license() {
    assert_eq!(refine_holder("NOTICE, DISCLAIMER, and LICENSE"), None);
}

#[test]
fn test_refine_holder_truncates_lzo_version_tail() {
    assert_eq!(
        refine_holder("Markus Franz Xaver Johannes Oberhumer LZO version v"),
        Some("Markus Franz Xaver Johannes Oberhumer".to_string())
    );
}

// ── refine_holder ────────────────────────────────────────────────

#[test]
fn test_refine_holder_basic() {
    let result = refine_holder("Acme Inc.");
    assert_eq!(result, Some("Acme Inc.".to_string()));
}

#[test]
fn test_refine_holder_strips_trailing_confidentiality_qualifiers() {
    assert_eq!(
        refine_holder("Motorola, Inc. - Motorola Confidential Proprietary"),
        Some("Motorola, Inc. - Motorola".to_string())
    );
    assert_eq!(
        refine_holder("Foo Platforms, Inc. and affiliates. Confidential and proprietary."),
        Some("Foo Platforms, Inc. and affiliates".to_string())
    );
    assert_eq!(
        refine_holder("Acme Confidential, Proprietary"),
        Some("Acme".to_string())
    );
    assert_eq!(refine_holder("Confidential"), None);
    assert_eq!(refine_holder("Confidential Information"), None);
    assert_eq!(refine_holder("Confidential, Proprietary"), None);
}

#[test]
fn test_refine_holder_removes_embedded_url_token() {
    let result = refine_holder("the http://wtforms.simplecodes.com WTForms Team");
    assert_eq!(result, Some("the WTForms Team".to_string()));
}

#[test]
fn test_refine_holder_strips_angle_bracketed_www_domain() {
    let result = refine_holder("Texas Instruments, <www.ti.com> Richard Woodruff");
    assert_eq!(
        result,
        Some("Texas Instruments, Richard Woodruff".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_mountain_view_ca() {
    let result = refine_holder("Sun Microsystems, Inc. Mountain View, CA.");
    assert_eq!(
        result,
        Some("Sun Microsystems, Inc. Mountain View".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_url_separator() {
    let result = refine_holder("Continuum Analytics, Inc. / http://continuum.io");
    assert_eq!(result, Some("Continuum Analytics, Inc.".to_string()));
}

#[test]
fn test_refine_holder_strips_trailing_dash_after_url_removal() {
    let result = refine_holder("Pouya Saadeghi - https://daisyui.com");
    assert_eq!(result, Some("Pouya Saadeghi".to_string()));
}

#[test]
fn test_refine_holder_strips_reserved_font_name_clause() {
    let result = refine_holder("Adobe (http://www.adobe.com/), with Reserved Font Name ‘Source’");
    assert_eq!(result, Some("Adobe".to_string()));
}

#[test]
fn test_refine_holder_empty() {
    assert_eq!(refine_holder(""), None);
}

#[test]
fn test_refine_holder_junk() {
    assert_eq!(refine_holder("the"), None);
}

#[test]
fn test_refine_holder_drops_bare_software_token() {
    assert_eq!(refine_holder("software"), None);
}

#[test]
fn test_refine_holder_junk_contributors_as_and_public() {
    assert_eq!(refine_holder("contributors as"), None);
    assert_eq!(refine_holder("public"), None);
}

#[test]
fn test_refine_holder_junk_patents_trade_secrets_fragments() {
    assert_eq!(refine_holder("patents, trade secrets"), None);
    assert_eq!(refine_holder("patent, or trademark"), None);
    assert_eq!(
        refine_holder("including without limitation by United States"),
        None
    );
    assert_eq!(refine_holder("TRADEMARKS"), None);
}

#[test]
fn test_refine_holder_junk_notice_and_do_the_following() {
    assert_eq!(refine_holder("notice"), None);
    assert_eq!(refine_holder("do the following"), None);
}

#[test]
fn test_refine_holder_junk_changelog_timestamp_username() {
    assert_eq!(refine_holder("11:46 vruppert"), None);
}

#[test]
fn test_refine_holder_junk_template_placeholders() {
    assert_eq!(refine_holder("date.year pkg.author"), None);
    assert_eq!(refine_holder("pkg.author"), None);
    assert_eq!(refine_holder("format YYYY-MM-DD, -04"), None);
    assert_eq!(refine_holder("< pkg.author >"), None);
}

#[test]
fn test_refine_holder_junk_symbol_conversion_table() {
    assert_eq!(refine_holder("(tm) (TM) → ™ (r) (R) → ®"), None);
    assert_eq!(refine_holder("Dot ⟶ ˙"), None);
}

#[test]
fn test_refine_holder_drops_mojibake_unicode_table_runs() {
    assert_eq!(refine_holder("ÃÃÃÃÃÃÃÃÃÃÃÃ"), None);
}

#[test]
fn test_refine_holder_junk_legal_disclaimer_fragments() {
    assert_eq!(
        refine_holder("NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES"),
        None
    );
    assert_eq!(refine_holder("TRADEMARK, TRADE SECRET"), None);
    assert_eq!(refine_holder("NOTICE, LICENSE AND DISCLAIMER."), None);
    assert_eq!(refine_holder("the Standard"), None);
    assert_eq!(refine_holder("The Product"), None);
    assert_eq!(refine_holder("proprietary"), None);
}

#[test]
fn test_refine_holder_junk_short_rsa_and_ecos_title() {
    assert_eq!(refine_holder("RSA"), None);
    assert_eq!(refine_holder("the Sample Embedded Operating System"), None);
}

#[test]
fn test_refine_holder_junk_math_c_functions() {
    assert_eq!(refine_holder("Convert Chebyshev"), None);
    assert_eq!(refine_holder("Multiply a Chebyshev"), None);
}

#[test]
fn test_refine_holder_strips_ecos_title_prefix_keeps_company() {
    assert_eq!(
        refine_holder("the Sample Embedded Operating System., Red Hat, Inc."),
        Some("Red Hat, Inc.".to_string())
    );
}

#[test]
fn test_refine_holder_junk_all_caps_placeholders() {
    assert_eq!(refine_holder("MODULEAUTHOR endif"), None);
    assert_eq!(refine_holder("THE PACKAGE'S"), None);
    assert_eq!(refine_holder("THE TOOLKIT'S"), None);
}

#[test]
fn test_refine_holder_strips_trailing_authors_section_label() {
    assert_eq!(
        refine_holder("IBM, Corp. Authors Anthony Liguori"),
        Some("IBM, Corp.".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_authors_clause() {
    let result =
        refine_copyright("Copyright IBM, Corp. 2007 Authors Anthony Liguori <aliguori@us.ibm.com>");
    assert_eq!(result, Some("Copyright IBM, Corp. 2007".to_string()));
}

#[test]
fn test_refine_copyright_strips_trailing_document_authors_clause() {
    let result = refine_copyright(
        "Copyright (c) 2011 Joyent, Inc. and the persons identified as document authors.",
    );
    assert_eq!(result, Some("Copyright (c) 2011 Joyent, Inc.".to_string()));
}

#[test]
fn test_refine_copyright_keeps_authors_clause_when_multiple_names() {
    let result = refine_copyright(
        "Copyright (c) 2006-2008 One Laptop Per Child Authors Zephaniah E. Hull Andres Salomon <dilinger@debian.org>",
    );
    assert_eq!(
            result,
            Some(
                "Copyright (c) 2006-2008 One Laptop Per Child Authors Zephaniah E. Hull Andres Salomon <dilinger@debian.org>"
                    .to_string()
            )
        );
}

#[test]
fn test_refine_copyright_keeps_authors_when_part_of_product_name() {
    let result =
        refine_copyright("Copyright (c) 2019 The Bootstrap Authors https://getbootstrap.com");
    assert_eq!(
        result,
        Some("Copyright (c) 2019 The Bootstrap Authors https://getbootstrap.com".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_document_authors_clause() {
    let result = refine_holder("Joyent, Inc. and the persons identified as document authors.");
    assert_eq!(result, Some("Joyent, Inc.".to_string()));
}

#[test]
fn test_refine_copyright_preserves_maintainer_suffix() {
    let result = refine_copyright("Copyright (c) 1998-2000 Michel Aubry, Maintainer");
    assert_eq!(
        result,
        Some("Copyright (c) 1998-2000 Michel Aubry, Maintainer".to_string())
    );
}

#[test]
fn test_refine_holder_preserves_maintainer_suffix() {
    assert_eq!(
        refine_holder("Michel Aubry, Maintainer"),
        Some("Michel Aubry, Maintainer".to_string())
    );
}

#[test]
fn test_refine_holder_junk_patent_and_treaties_fragments() {
    assert_eq!(refine_holder("treaties"), None);
    assert_eq!(
        refine_holder("patent or other licenses necessary and to obtain"),
        None
    );
}

#[test]
fn test_meta_sdk_license_false_positive_refiner_drops_holder_fragments() {
    assert_eq!(refine_holder("as required"), None);
    assert_eq!(
        refine_holder(
            "infringement, patent infringement, trademark infringement, violations of the brand guidelines, violations of your or our confidential"
        ),
        None
    );
    assert_eq!(
        refine_holder(
            "the behavior of the proximity sensor in the MPT hardware implemented by the MPT system software"
        ),
        None
    );
}

#[test]
fn test_meta_sdk_license_false_positive_refiner_drops_copyright_fragment() {
    let refined = refine_copyright(
        "copyright, trade secret, trademark, rights of publicity and privacy, and other proprietary rights. 3.2 Third-Party Materials. Our SDK may"
    )
    .unwrap();
    assert!(is_junk_copyright(&refined));
}

#[test]
fn test_refine_copyright_preserves_european_community_notice() {
    assert_eq!(
        refine_copyright("(c) the European Community 2007"),
        Some("(c) the European Community 2007".to_string())
    );
}

#[test]
fn test_refine_holder_preserves_copyright_prefixed_notice_holder() {
    assert_eq!(
        refine_holder("Copyright (c) 1988, 1993"),
        Some("Copyright (c) 1988, 1993".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_x509_dn_fields() {
    assert_eq!(
        refine_holder("Microsoft Corp., OU Microsoft Corporation, CN Microsoft Root"),
        Some("Microsoft Corp.".to_string())
    );
    assert_eq!(
        refine_holder("OISTE Foundation Endorsed, CN OISTE WISeKey Global Root"),
        Some("OISTE Foundation".to_string())
    );
}

#[test]
fn test_refine_holder_normalizes_angle_bracket_email_comma_spacing() {
    let result = refine_holder("Acme <dev@acme.test>, Foo");
    assert_eq!(result, Some("Acme <dev@acme.test>, Foo".to_string()));
}

#[test]
fn test_refine_holder_strips_trailing_comma_software() {
    let result = refine_holder("Ian F. Darwin,,, Software");
    assert_eq!(result, Some("Ian F. Darwin".to_string()));
}

#[test]
fn test_refine_author_drops_config_and_legal_junk_fragments() {
    assert_eq!(refine_author("with the mode of 000"), None);
    assert_eq!(
        refine_author("kernel afs. skip AFS metadata and ACLs"),
        None
    );
    assert_eq!(refine_author("with a FSF"), None);
    assert_eq!(refine_author("with a DCO"), None);
    assert_eq!(refine_author("gives unlimited"), None);
    assert_eq!(
        refine_author(
            "Word Assigns past and future changes. new src/libgcrypt.pc.in, src/Makefile.am, src/secmem.c"
        ),
        None
    );
}

#[test]
fn test_refine_author_truncates_bug_reports_tail() {
    assert_eq!(
        refine_author(
            "Werner Koch <wk@gnupg.org> Bug reports https://bugs.gnupg.org Security related bug reports <security@gnupg.org> End-of-life TBD"
        ),
        Some("Werner Koch <wk@gnupg.org>".to_string())
    );
}

#[test]
fn test_refine_author_strips_trailing_comma_and() {
    assert_eq!(
        refine_author("Philip Hazel, and"),
        Some("Philip Hazel".to_string())
    );
}

#[test]
fn test_refine_author_drops_glibc_prose_fragments() {
    assert_eq!(
        refine_author(
            "Maintainers <debian-glibc@lists.debian.org> from https://sourceware.org/git/glibc.git"
        ),
        None
    );
    assert_eq!(refine_author("versions, and"), None);
    assert_eq!(refine_author("makes"), None);
    assert_eq!(refine_author("grants"), None);
    assert_eq!(refine_author("grants irrevocable"), None);
    assert_eq!(refine_author("version information"), None);
    assert_eq!(refine_author("example. If"), None);
    assert_eq!(refine_author("doxygen. Using"), None);
    assert_eq!(refine_author("final String?"), None);
    assert_eq!(
        refine_author(
            "VALUE FileDescription A sample application demonstrating Flutter APIs VALUE FileVersion"
        ),
        None
    );
    assert_eq!(refine_author("the ListWheelChildManager"), None);
    assert_eq!(
        refine_author("Alexander Peslyak in d+. No copyright is"),
        None
    );
    assert_eq!(
        refine_author("[becoming a sponsor] (https://opencollective.com/pnpm#sponsor)"),
        None
    );
    assert_eq!(
        refine_author("the command [#7403] (https://github.com/pnpm/pnpm/issues/7403)"),
        None
    );
    assert_eq!(
        refine_author("not responsible for the consequences of use of"),
        None
    );
    assert_eq!(
        refine_author("at SunPro, a Sun Microsystems, Inc. business"),
        None
    );
}

#[test]
fn test_refine_holder_drops_cc0_and_libgcrypt_junk_fragments() {
    assert_eq!(
        refine_holder("Related Rights (defined below) upon the creator and subsequent"),
        None
    );
    assert_eq!(refine_holder("Related"), None);
    assert_eq!(refine_holder("related or neighboring"), None);
    assert_eq!(refine_holder("was owned solely by FSF"), None);
    assert_eq!(refine_holder("years may be listed"), None);
}

#[test]
fn test_is_junk_copyright_drops_cc0_and_libgcrypt_junk_fragments() {
    assert!(is_junk_copyright(
        "copyright and related or neighboring rights"
    ));
    assert!(is_junk_copyright(
        "copyright and related or neighboring legal rights in the Work"
    ));
    assert!(is_junk_copyright("copyright was owned solely by FSF"));
    assert!(is_junk_copyright("copyright years may be listed"));
}

#[test]
fn test_is_junk_copyright_drops_code_signature_and_commentary_fragments() {
    assert!(is_junk_copyright("copyright, String? description, bool"));
    assert!(is_junk_copyright(
        "copyright $template .replaceAllMapped RegExp r ( ^ +), (Match match) final"
    ));
    assert!(is_junk_copyright(
        "copyright and comment directing the reader to the original source"
    ));
    assert!(is_junk_copyright(
        "copyright referencing The Flutter Authors"
    ));
    assert!(is_junk_copyright(
        "line.startswith Copyright (c) Microsoft Corporation"
    ));
    assert!(is_junk_copyright("not copyrighted The Flutter Authors"));
    assert!(is_junk_copyright(
        "copyright comments are original works produced specifically for use as"
    ));
    assert!(is_junk_copyright("copyright, resulting in confusion over"));
    assert!(is_junk_copyright(
        "Copyright Flutter code sample for MyElement"
    ));
    assert!(is_junk_copyright(
        "Copyright d+(?:- d+)?, the V8 project authors"
    ));
    assert!(is_junk_copyright("Copyright (c ) d+ Google Inc."));
    assert!(is_junk_copyright("Copyright 201 34"));
    assert!(is_junk_copyright("Copyright 0 absl::StrCat(errors 0 )"));
    assert!(is_junk_copyright("Copyright void"));
    assert!(!is_junk_copyright("Not copyrighted 1992 by Mark Adler"));
}

#[test]
fn test_refine_copyright_strips_trailing_or_and_noise_descriptors() {
    assert_eq!(
        refine_copyright("Copyright (c) 1993,2004 Sun Microsystems or"),
        Some("Copyright (c) 1993,2004 Sun Microsystems".to_string())
    );
    assert_eq!(
        refine_copyright("Copyright (c) 2011 by Ashima Arts (Simplex noise)"),
        Some("Copyright (c) 2011 by Ashima Arts".to_string())
    );
    assert_eq!(
        refine_copyright("Copyright (c) 2011-2016 by Stefan Gustavson (Classic noise and others)"),
        Some("Copyright (c) 2011-2016 by Stefan Gustavson".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_et_al() {
    let result = refine_holder("Daniel Stenberg, et al");
    assert_eq!(result, Some("Daniel Stenberg".to_string()));
}

#[test]
fn test_refine_holder_drops_flutter_compare_noise_fragments() {
    assert_eq!(refine_holder("String? description, bool"), None);
    assert_eq!(refine_holder("String? description late bool"), None);
    assert_eq!(
        refine_holder("$template .replaceAllMapped RegExp r ^ +), (Match match) final"),
        None
    );
    assert_eq!(
        refine_holder("comment directing the reader to the original source"),
        None
    );
    assert_eq!(refine_holder("Flutter code sample for MyElement"), None);
    assert_eq!(refine_holder("not The Flutter Authors"), None);
    assert_eq!(refine_holder("referencing The Flutter Authors"), None);
    assert_eq!(
        refine_holder("comments original works produced specifically for use as part of"),
        None
    );
    assert_eq!(refine_holder("resulting in confusion over"), None);
    assert_eq!(refine_holder("absl::StrCat(errors 0"), None);
}

#[test]
fn test_refine_holder_strips_trailing_noise_descriptors() {
    assert_eq!(
        refine_holder("Ashima Arts (Simplex noise)"),
        Some("Ashima Arts".to_string())
    );
    assert_eq!(
        refine_holder("Stefan Gustavson Classic noise and others"),
        Some("Stefan Gustavson".to_string())
    );
}

#[test]
fn test_refine_holder_drops_dense_unicode_symbol_runs() {
    assert_eq!(
        refine_holder("˙∆˚¬…æ≈ç√∫˜µ≤≥≥≥≥÷¡™£¢∞ ¶•ªº-≠⁄€‹›ﬁﬂ‡°·‚—±Œ„´‰Á¨Ø∏”’"),
        None
    );
}

#[test]
fn test_refine_author_normalizes_angle_bracket_email_comma_spacing() {
    let result = refine_author("dev <dev@acme.test>, Foo");
    assert_eq!(result, Some("dev <dev@acme.test>, Foo".to_string()));
}

#[test]
fn test_refine_author_keeps_obfuscated_angle_contact_author() {
    let result = refine_author("Deepak M <m.deepak at intel.com>");
    assert_eq!(result, Some("Deepak M m.deepak at intel.com".to_string()));
}

#[test]
fn test_refine_author_strips_trailing_comma_year() {
    let result = refine_author("Paul Vixie, 1996");
    assert_eq!(result, Some("Paul Vixie".to_string()));
}

#[test]
fn test_refine_author_strips_better_known_as_clause() {
    let result =
        refine_author("Alexander Peslyak, better known as Solar Designer <solar at openwall.com>");
    assert_eq!(result, Some("Alexander Peslyak".to_string()));
}

#[test]
fn test_refine_author_strips_distribution_metadata_tails() {
    assert_eq!(
        refine_author("Armin Ronacher Author-email armin.ronacher@active-4.com"),
        Some("Armin Ronacher".to_string())
    );
    assert_eq!(
        refine_author("OWASP Foundation Maintainer-email security@owasp.org"),
        Some("OWASP Foundation".to_string())
    );
}

#[test]
fn test_refine_author_drops_generated_resource_identifiers() {
    assert_eq!(refine_author("icon-app-20x20@2x.png.img.tmpl"), None);
}

#[test]
fn test_refine_author_drops_markup_feed_identifiers() {
    assert_eq!(refine_author("doi:10.1038/nature05582"), None);
    assert_eq!(refine_author("tag:contoso.com,2000"), None);
    assert_eq!(refine_author("id/1234"), None);
    assert_eq!(refine_author("James 2006-04-25T12:12:12Z"), None);
    assert_eq!(refine_author("authorauthor"), None);
    assert_eq!(refine_author("XmlLang en-usabcd"), None);
}

#[test]
fn test_refine_copyright_drops_versioninfo_and_dtd_junk() {
    assert_eq!(
        refine_copyright("Copyright (c) 2050 VALUE OriginalFilename NativeConsoleApp.exe"),
        None
    );
    assert_eq!(
        refine_copyright("copyright <!ELEMENT A ( PCDATA) > aaaa"),
        None
    );
    assert_eq!(refine_copyright("Copyright get set"), None);
    assert_eq!(refine_copyright("copyright public void"), None);
    assert_eq!(refine_copyright("Copyright clone.Copyright.Text"), None);
    assert_eq!(
        refine_copyright("Copyright HeaderType.Content u00AD u00AE"),
        None
    );
}

#[test]
fn test_refine_copyright_drops_prose_fragments_from_license_boilerplate() {
    assert_eq!(
        refine_copyright("copyright licenses specified in the"),
        None
    );
    assert_eq!(refine_copyright("copyright in its"), None);
}

#[test]
fn test_refine_holder_drops_license_boilerplate_fragment() {
    assert_eq!(refine_holder("licenses specified in the"), None);
}

#[test]
fn test_refine_copyright_strips_flutter_wrapper_context() {
    assert_eq!(
        refine_copyright("applicationLegalese: '© 2014 The Flutter Authors',"),
        Some("(c) 2014 The Flutter Authors".to_string())
    );
    assert_eq!(
        refine_copyright(
            "PRODUCT_COPYRIGHT = Copyright © 2014 The Flutter Authors. All rights reserved."
        ),
        Some("Copyright (c) 2014 The Flutter Authors".to_string())
    );
    assert_eq!(
        refine_copyright(
            r#"<label opaque="NO" text="© 2018 The Flutter Authors. All rights reserved." />"#
        ),
        Some("(c) 2018 The Flutter Authors".to_string())
    );
    assert_eq!(
        refine_copyright(
            r#"VALUE "LegalCopyright", "Copyright (C) {{year}} {{organization}}. All rights reserved." "\0""#
        ),
        None
    );
}

#[test]
fn test_refine_copyright_drops_flutter_generated_code_fragments() {
    assert_eq!(
        refine_copyright(
            r#"<i class="material-icons-sharp md-36">copyright</i> &#x2014; material icon named "copyright" (sharp)."#
        ),
        None
    );
    assert_eq!(
        refine_copyright("verifyEntry(mapping, 'KeyC', <String>[r'c', r'C', r'©', r'¢'], 'c');"),
        None
    );
    assert_eq!(refine_copyright("r'u3 u©g˝g' r'v2˚kk' r'w2ÂzÅz'"), None);
}

#[test]
fn test_refine_copyright_strips_all_rights_reserved_clause() {
    assert_eq!(
        refine_copyright("Copyright 2024 Apple Inc. All rights reserved."),
        Some("Copyright 2024 Apple Inc.".to_string())
    );
}

#[test]
fn test_refine_holder_drops_versioninfo_and_dtd_junk() {
    assert_eq!(
        refine_holder("VALUE OriginalFilename NativeConsoleApp.exe"),
        None
    );
    assert_eq!(refine_holder("PCDATA"), None);
    assert_eq!(refine_holder("clone.Copyright.Text"), None);
    assert_eq!(refine_holder("HeaderType.Content u00AD u00AE"), None);
}

#[test]
fn test_refine_holder_drops_prose_fragments_from_license_boilerplate() {
    assert_eq!(
        refine_holder("notice, list of conditions, and disclaimer when submitting"),
        None
    );
}

#[test]
fn test_refine_holder_drops_flutter_generated_code_fragments() {
    assert_eq!(refine_holder("x2014 material icon named"), None);
    assert_eq!(refine_holder("r'¢"), None);
    assert_eq!(refine_holder("void"), None);
    assert_eq!(refine_holder("organization"), None);
}

#[test]
fn test_refine_author_drops_template_token_runs_and_numeric_fragments() {
    assert_eq!(refine_author("AUTH CONTRIBUTORS AUTHS+ + 2660"), None);
    assert_eq!(refine_author("AUTH AUTHS 2730"), None);
    assert_eq!(refine_author("COMPANY 1411"), None);
    assert_eq!(refine_author("MAINT 26382"), None);
    assert_eq!(refine_author("2645-1"), None);
}

#[test]
fn test_refine_holder_does_not_strip_normal_comma_separated_names() {
    assert_eq!(
        refine_holder("Sam Leffler, Errno Consulting, Atheros Communications, Inc."),
        Some("Sam Leffler, Errno Consulting, Atheros Communications, Inc.".to_string())
    );
}

#[test]
fn test_refine_holder_does_not_strip_lp_suffix() {
    assert_eq!(
        refine_holder("Hewlett-Packard Development Company, L.P."),
        Some("Hewlett-Packard Development Company, L.P.".to_string())
    );
}

#[test]
fn test_refine_holder_strips_prefix() {
    let result = refine_holder("by Acme Corp");
    assert_eq!(result, Some("Acme Corp".to_string()));
}

#[test]
fn test_refine_holder_strips_trailing_period() {
    let result = refine_holder("IBM Corporation.");
    assert_eq!(result, Some("IBM Corporation".to_string()));
}

#[test]
fn test_refine_holder_keeps_xerox_corporation() {
    let result = refine_holder("Xerox Corporation");
    assert_eq!(result, Some("Xerox Corporation".to_string()));
}

#[test]
fn test_refine_holder_strips_trailing_division_of_company_suffix() {
    let input = "Industrial Light & Magic, a division of Lucas Digital Ltd. LLC";
    assert_eq!(
        refine_holder(input),
        Some("Industrial Light & Magic".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_period_after_trailing_comma() {
    let result = refine_holder("Sun Microsystems.,");
    assert_eq!(result, Some("Sun Microsystems".to_string()));
}

#[test]
fn test_refine_holder_strips_independent_jpeg_group_software_tail() {
    let result = refine_holder("Thomas G. Lane, Part of the Independent JPEG Group's software");
    assert_eq!(
        result,
        Some("Thomas G. Lane, Part of the Independent JPEG Group's".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_contributor_parens_after_org() {
    let result = refine_copyright(
        "Copyright (c) 1998-2001 VideoLAN (Johan Bilien <jobi@via.ecp.fr> and Gildas Bazin <gbazin@netcourrier.com> )",
    );
    assert_eq!(
            result,
            Some(
                "Copyright (c) 1998-2001 VideoLAN Johan Bilien <jobi@via.ecp.fr> and Gildas Bazin <gbazin@netcourrier.com>".to_string()
            )
        );
}

#[test]
fn test_refine_holder_strips_contributor_parens_after_org() {
    let result = refine_holder("VideoLAN (Johan Bilien and Gildas Bazin)");
    assert_eq!(
        result,
        Some("VideoLAN Johan Bilien and Gildas Bazin".to_string())
    );
}

#[test]
fn test_refine_holder_strips_see_authors_suffix() {
    let result = refine_holder("Carsten Haitzler and various contributors (see AUTHORS)");
    assert_eq!(
        result,
        Some("Carsten Haitzler and various contributors".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_javadoc_tags() {
    let result = refine_holder("Michal Migurski @version 1.0");
    assert_eq!(result, Some("Michal Migurski".to_string()));
}

#[test]
fn test_refine_holder_strips_trailing_batch_comment_marker() {
    let result = refine_holder_in_copyright_context("the original author or authors. @rem");
    assert_eq!(result, Some("the original author or authors".to_string()));
}

#[test]
fn test_refine_holder_drops_compare_triage_code_fragments() {
    assert_eq!(refine_holder("isInstanceOf"), None);
    assert_eq!(refine_holder("contributor, path"), None);
    assert_eq!(refine_holder("final cProvider"), None);
    assert_eq!(refine_holder("c.isExactly(element)"), None);
    assert_eq!(
        refine_holder(
            "handle(argument) Stream result LambdaSafe .callbacks(GenericFactory.class, Collections.singleton(callbackInstance), argument)"
        ),
        None
    );
}

#[test]
fn test_refine_holder_drops_translation_and_placeholder_labels() {
    assert_eq!(refine_holder("trademark msgstr"), None);
    assert_eq!(refine_holder("trademark violation msgstr"), None);
    assert_eq!(refine_holder("project"), None);
    assert_eq!(refine_holder("placeholder"), None);
}

#[test]
fn test_refine_holder_drops_lowercase_enum_blobs() {
    assert_eq!(refine_holder("malware 7, other"), None);
    assert_eq!(refine_holder("copyright 6, malware 7, other"), None);
}

#[test]
fn test_refine_holder_keeps_plain_dotted_org_names() {
    assert_eq!(refine_holder("abc.org"), Some("abc.org".to_string()));
    assert_eq!(refine_holder("ibm.com"), Some("ibm.com".to_string()));
}

#[test]
fn test_refine_holder_keeps_collective_company_contributors_phrase() {
    let input = "Digia Plc and/or its subsidiary(-ies) and other contributors";
    assert_eq!(refine_holder(input), Some(input.to_string()));
}

#[test]
fn test_refine_holder_keeps_affiliate_s_parenthetical_phrase() {
    let input = "HERE Global B.V. and its affiliate(s)";
    assert_eq!(refine_holder(input), Some(input.to_string()));
}

#[test]
fn test_refine_holder_keeps_lowercase_hyphenated_project_name_in_copyright_context() {
    assert_eq!(
        refine_holder_in_copyright_context("dynamic-evaluation"),
        Some("dynamic-evaluation".to_string())
    );
    assert_eq!(
        refine_holder_in_copyright_context("rds-snapshot-encrypted"),
        Some("rds-snapshot-encrypted".to_string())
    );
}

#[test]
fn test_refine_holder_strips_lowercase_handle_angle_email() {
    assert_eq!(
        refine_holder("dead_horse <dead_horse@qq.com>"),
        Some("dead_horse".to_string())
    );
}

#[test]
fn test_refine_holder_keeps_lowercase_company_with_inc_suffix() {
    assert_eq!(
        refine_holder_in_copyright_context("craigslist, inc."),
        Some("craigslist, inc.".to_string())
    );
    assert_eq!(
        refine_holder_in_copyright_context("craigslist, inc"),
        Some("craigslist, inc".to_string())
    );
}

#[test]
fn test_refine_holder_in_copyright_context_strips_no_rights_reserved_clause() {
    assert_eq!(
        refine_holder_in_copyright_context("FontTools. No rights reserved."),
        Some("FontTools".to_string())
    );
}

#[test]
fn test_refine_holder_in_copyright_context_keeps_ato_gear_notice_holder() {
    assert_eq!(
        refine_holder_in_copyright_context("ATO Gear."),
        Some("ATO Gear".to_string())
    );
}

#[test]
fn test_refine_holder_in_copyright_context_strips_onwards_prefix() {
    assert_eq!(
        refine_holder_in_copyright_context("and onwards The Apache Software Foundation"),
        Some("The Apache Software Foundation".to_string())
    );
    assert_eq!(
        refine_holder_in_copyright_context("onwards The Apache Software Foundation"),
        Some("The Apache Software Foundation".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_ansi_escape_suffix() {
    assert_eq!(
        refine_copyright("(c) 1996 Id Software, inc. x1b 21;1H"),
        Some("(c) 1996 Id Software, inc.".to_string())
    );
}

#[test]
fn test_refine_holder_in_copyright_context_strips_trailing_ansi_escape_suffix() {
    assert_eq!(
        refine_holder_in_copyright_context("Id Software, inc. x1b 21;1H"),
        Some("Id Software, inc.".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_placeholder_dollar() {
    assert_eq!(
        refine_holder("Markus Franz Xaver Johannes Oberhumer $"),
        Some("Markus Franz Xaver Johannes Oberhumer".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_see_authors_suffix() {
    let result = refine_copyright(
        "Copyright (c) 2000 Carsten Haitzler and various contributors (see AUTHORS)",
    );
    assert_eq!(
        result,
        Some("Copyright (c) 2000 Carsten Haitzler and various contributors".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_everyone_is_permitted_to_copy_clause() {
    let result =
        refine_copyright("Copyright (C) 2001 Project Mayo. Everyone is permitted to copy a");
    assert_eq!(result, Some("Copyright (C) 2001 Project Mayo".to_string()));
}

#[test]
fn test_refine_copyright_keeps_affiliate_s_parenthetical_phrase() {
    let input = "Copyright (C) 2016-2018 HERE Global B.V. and its affiliate(s).";
    assert_eq!(
        refine_copyright(input),
        Some("Copyright (C) 2016-2018 HERE Global B.V. and its affiliate(s)".to_string())
    );
}

#[test]
fn test_is_junk_copyright_drops_html_entity_regex_fragments() {
    assert!(is_junk_copyright("(c) 169 reg 174 hellip 8230 x2F 47 /g"));
}

#[test]
fn test_refine_copyright_strips_trailing_javadoc_tags() {
    let result = refine_copyright("copyright 2005 Michal Migurski @version 1.0");
    assert_eq!(result, Some("copyright 2005 Michal Migurski".to_string()));
}

#[test]
fn test_refine_copyright_strips_trailing_batch_comment_marker() {
    let result = refine_copyright("Copyright 2015 the original author or authors. @rem");
    assert_eq!(
        result,
        Some("Copyright 2015 the original author or authors".to_string())
    );
}

#[test]
fn test_refine_copyright_drops_compare_triage_code_fragments() {
    assert!(is_junk_copyright("(c) contributor, path"));
    assert!(is_junk_copyright("(c) final cProvider"));
}

#[test]
fn test_refine_copyright_drops_c_sign_code_expressions() {
    assert_eq!(refine_copyright("(c) c.filePath, c"), None);
    assert_eq!(
        refine_copyright("(c) puts Candidate foundational flow changes"),
        None
    );
    assert_eq!(refine_copyright("(c) and I have not modified it"), None);
}

// ── refine_author ────────────────────────────────────────────────

#[test]
fn test_refine_author_basic() {
    let result = refine_author("John Doe");
    assert_eq!(result, Some("John Doe".to_string()));
}

#[test]
fn test_refine_author_empty() {
    assert_eq!(refine_author(""), None);
}

#[test]
fn test_refine_author_junk() {
    assert_eq!(refine_author("james hacker"), None);
    assert_eq!(refine_author("who hopes"), None);
}

#[test]
fn test_refine_author_strips_author_prefix() {
    let result = refine_author("author John Doe");
    assert_eq!(result, Some("John Doe".to_string()));
}

#[test]
fn test_refine_author_strips_maintainers_prefix() {
    let result = refine_author("Maintainers Hadley <h.wickham@gmail.com>");
    assert_eq!(result, Some("Hadley <h.wickham@gmail.com>".to_string()));
}

#[test]
fn test_refine_author_email_and_name() {
    let result = refine_author("@author stephane@hillion.org Stephane Hillion");
    assert_eq!(
        result,
        Some("stephane@hillion.org Stephane Hillion".to_string())
    );
}

#[test]
fn test_refine_author_strips_trailing_javadoc_tags() {
    let result = refine_author("stephane@hillion.org Stephane Hillion @version 1.0");
    assert_eq!(
        result,
        Some("stephane@hillion.org Stephane Hillion".to_string())
    );
}

#[test]
fn test_refine_author_drops_bare_version_token() {
    assert_eq!(refine_author("version"), None);
}

#[test]
fn test_refine_author_strips_trailing_paren_years() {
    let result = refine_author("author: Theo de Raadt (1995-1999)");
    assert_eq!(result, Some("Theo de Raadt".to_string()));
}

#[test]
fn test_refine_author_strips_trailing_bare_c_clause() {
    let result = refine_author(
        "Denis Joseph Barrow (djbarrow@de.ibm.com,barrow_dj@yahoo.com) (c) 2000 IBM Corp",
    );
    assert_eq!(
        result,
        Some("Denis Joseph Barrow (djbarrow@de.ibm.com,barrow_dj@yahoo.com)".to_string())
    );
}

#[test]
fn test_refine_author_junk_prefix() {
    assert_eq!(refine_author("httpProxy something"), None);
}

#[test]
fn test_refine_author_drops_code_assignment_fragments() {
    assert_eq!(
        refine_author("Maintainers <- utils::as.person(people)"),
        None
    );
}

// ── strip_all_unbalanced_parens ──────────────────────────────────

#[test]
fn test_strip_all_unbalanced_parens_mixed() {
    let result = strip_all_unbalanced_parens("Hello ) World < Foo >");
    // The lone ) and the balanced <> should be handled.
    assert_eq!(result, "Hello   World < Foo >");
}

// ── URL slash stripping ──────────────────────────────────────────

#[test]
fn test_refine_copyright_url_trailing_slash() {
    let result =
        refine_copyright("Copyright (c) 2007 Free Software Foundation, Inc. http://fsf.org/");
    assert_eq!(
        result,
        Some("Copyright (c) 2007 Free Software Foundation, Inc. http://fsf.org".to_string())
    );
}

#[test]
fn test_refine_copyright_keeps_w3c_registered_paren_group() {
    let result = refine_copyright("Copyright (c) YEAR W3C(r) (MIT, ERCIM, Keio, Beihang).");
    assert_eq!(
        result,
        Some("Copyright (c) YEAR W3C(r) (MIT, ERCIM, Keio, Beihang)".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_reserved_font_name_clause() {
    let result =
        refine_copyright("© 2023 Adobe (http://www.adobe.com/), with Reserved Font Name ‘Source’");
    assert_eq!(
        result,
        Some("© 2023 Adobe (http://www.adobe.com/)".to_string())
    );
}

#[test]
fn test_refine_holder_sk() {
    assert_eq!(refine_holder("S K (xz64)"), Some("S K".to_string()));
    assert_eq!(refine_holder("S K"), Some("S K".to_string()));
}

#[test]
fn test_refine_holder_strips_trailing_single_digit_token() {
    assert_eq!(
        refine_holder("Waterloo Micro. 8"),
        Some("Waterloo Micro".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_trailing_digit_then_period() {
    assert_eq!(
        refine_copyright("(c) 1985 Waterloo Micro. 8"),
        Some("(c) 1985 Waterloo Micro".to_string())
    );
}

#[test]
fn test_refine_author_drops_d3_transition_markdown_link_fragment() {
    assert_eq!(
        refine_author(
            "transition .transition https://github.com/d3/d3-transition/blob/master/README.md"
        ),
        None
    );
}

#[test]
fn test_refine_author_drops_path_like_fragment() {
    assert_eq!(refine_author("from/authors/alphabetic"), None);
}

#[test]
fn test_refine_author_drops_file_reference_note() {
    assert_eq!(refine_author("see LICENSE.txt"), None);
    assert_eq!(refine_author("refer to docs/NOTICE.md"), None);
}

#[test]
fn test_refine_author_drops_dollar_prefixed_code_tokens() {
    assert_eq!(refine_author("Agatha Christie, $sort"), None);
    assert_eq!(refine_author("$limit 10"), None);
}

#[test]
fn test_refine_author_drops_annotation_like_prose_without_breaking_email_authors() {
    assert_eq!(
        refine_author("the observation proposal even intended for @Observable to work with value"),
        None
    );
    assert_eq!(
        refine_author("stephane@hillion.org Stephane Hillion"),
        Some("stephane@hillion.org Stephane Hillion".to_string())
    );
}

#[test]
fn test_refine_author_drops_structured_key_with_hex_value() {
    assert_eq!(
        refine_author("TargetAttributes 33CC10EC2044A3C60003C045"),
        None
    );
}

#[test]
fn test_refine_author_keeps_name_with_parenthesized_url() {
    assert_eq!(
        refine_author("Qix (http://github.com/qix-)"),
        Some("Qix (http://github.com/qix-)".to_string())
    );
}

#[test]
fn test_refine_author_drops_the_current_user_phrase() {
    assert_eq!(refine_author("the current user"), None);
}

#[test]
fn test_refine_author_drops_generic_field_labels_and_template_tokens() {
    assert_eq!(refine_author("current_user"), None);
    assert_eq!(refine_author("username"), None);
    assert_eq!(refine_author("created-at"), None);
    assert_eq!(refine_author("gl-link"), None);
}

#[test]
fn test_refine_author_drops_code_call_and_graphql_fragments() {
    assert_eq!(refine_author("params.delete(:author)"), None);
    assert_eq!(
        refine_author("expand_author_with_user_emails(author)"),
        None
    );
    assert_eq!(refine_author("UserWithType ...UserAvailability"), None);
}

#[test]
fn test_refine_author_drops_point_to_the_phrase() {
    assert_eq!(refine_author("point to the"), None);
}

#[test]
fn test_refine_author_drops_html_and_machine_colon_fragments() {
    assert_eq!(refine_author("the bad guy</textarea>"), None);
    assert_eq!(refine_author("references:users:unique"), None);
}

#[test]
fn test_refine_copyright_drops_css_footer_noise() {
    assert!(is_junk_copyright("Copyright footer"));
    assert!(is_junk_copyright("Copyright, Legal Notice"));
    assert!(is_junk_copyright("copyright color 666666"));
    assert!(is_junk_copyright("copyright font-size color 666"));
    assert!(is_junk_copyright(
        "copyrighted and may only be modified in the following manner. The"
    ));
}

#[test]
fn test_refine_holder_drops_css_selector_noise() {
    assert_eq!(refine_holder("footer"), None);
    assert_eq!(refine_holder("Legal Notice"), None);
    assert_eq!(refine_holder("color 666666"), None);
}

#[test]
fn test_refine_author_strips_generated_month_year_and_from_lib_tail() {
    assert_eq!(
        refine_author("Intel Corporation Generated November"),
        Some("Intel Corporation".to_string())
    );
    assert_eq!(
        refine_author("L. Plagne <laurent.plagne@edf.fr > from boost lib"),
        Some("L. Plagne <laurent.plagne@edf.fr >".to_string())
    );
}

#[test]
fn test_refine_author_drops_code_itself_and_lapack_package_prose() {
    assert_eq!(
        refine_author(
            "the code itself Stefan I. Larimore and Timothy A. Davis (davis@cise.ufl.edu), University of Florida. The algorithm was in collaboration with John Gilbert, Xerox PARC, and Esmond Ng, Oak Ridge National Laboratory"
        ),
        None
    );
    assert_eq!(
        refine_author(
            "LAPACK is a software package provided by Univ. of Tennessee, Univ. of California Berkeley, Univ. of Colorado Denver and NAG Ltd"
        ),
        None
    );
}

#[test]
fn test_refine_holder_drops_exclude_disclaimer_and_trailing_heavily() {
    assert_eq!(refine_holder("EXCLUDE"), None);
    assert_eq!(refine_holder("with the"), None);
    assert_eq!(
        refine_holder(
            "THE UNITED STATES, THE UNITED STATES DEPARTMENT OF ENERGY, AND THEIR EMPLOYEES"
        ),
        None
    );
    assert_eq!(
        refine_holder("Konstantinos Margaritis Heavily"),
        Some("Konstantinos Margaritis".to_string())
    );
}

#[test]
fn test_refine_holder_and_copyright_strip_single_letter_obfuscated_email_tail() {
    assert_eq!(
        refine_holder("Mark Borgerding mark a borgerding net"),
        Some("Mark Borgerding".to_string())
    );
    assert_eq!(
        refine_copyright("Copyright (c) 2009 Mark Borgerding mark a borgerding net"),
        Some("Copyright (c) 2009 Mark Borgerding".to_string())
    );
}

#[test]
fn test_refine_copyright_drops_exclude_and_mpl_fair_use_noise() {
    assert_eq!(refine_copyright("copyright EXCLUDE"), None);
    assert_eq!(
        refine_copyright("copyright doctrines of fair use, fair dealing, or other equivalents"),
        None
    );
}

#[test]
fn test_refine_copyright_strips_trailing_heavily_based_clause() {
    assert_eq!(
        refine_copyright("Copyright (c) 2010 Konstantinos Margaritis <markos@freevec.org> Heavily"),
        Some("Copyright (c) 2010 Konstantinos Margaritis <markos@freevec.org>".to_string())
    );
}

#[test]
fn test_refine_copyright_keeps_structured_copyright_notice_with_year() {
    assert_eq!(
        refine_copyright("Copyright Notice (1999) University of Chicago"),
        Some("Copyright Notice (1999) University of Chicago".to_string())
    );
}

#[test]
fn test_refine_copyright_strips_locale_timestamp_before_year() {
    assert_eq!(
        refine_copyright("Copyright (C) EDF R&D, lun sep 30 14:23:19 CEST 2002"),
        Some("Copyright (C) EDF R&D 2002".to_string())
    );
}

#[test]
fn test_refine_holder_strips_locale_timestamp_suffix() {
    assert_eq!(
        refine_holder("EDF R&D, lun sep 30 14:23:19 CEST"),
        Some("EDF R&D".to_string())
    );
}

#[test]
fn test_refine_holder_strips_trailing_prose_clauses() {
    assert_eq!(
        refine_holder("Alexander Peslyak and it is hereby released to the"),
        Some("Alexander Peslyak".to_string())
    );
    assert_eq!(
        refine_holder("Andreas Dilger, are derived from libpng-0.88"),
        Some("Andreas Dilger".to_string())
    );
}
