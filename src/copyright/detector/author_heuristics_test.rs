// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::copyright::line_tracking::PreparedLineCache;
use crate::copyright::types::AuthorDetection;
use crate::models::LineNumber;

#[test]
fn test_author_colon_multiline_keeps_emails() {
    let input = "/*\n * Authors: Jorge Cwik, <jorge@laser.satlink.net>\n *\t\tArnt Gulbrandsen, <agulbra@nvg.unit.no>\n */\n";

    let raw_lines: Vec<&str> = input.lines().collect();
    let prepared_cache = PreparedLineCache::new(&raw_lines).materialize();
    let mut extracted: Vec<AuthorDetection> = Vec::new();
    extract_author_colon_blocks(&prepared_cache, &mut extracted);
    assert!(
        extracted.iter().any(|ad| ad.author
            == "Jorge Cwik, <jorge@laser.satlink.net> Arnt Gulbrandsen, <agulbra@nvg.unit.no>"),
        "Expected direct author-colon extraction to keep emails, got: {:?}",
        extracted.iter().map(|ad| &ad.author).collect::<Vec<_>>()
    );

    let (_c, _h, a) = super::super::detect_copyrights_from_text(input);

    assert!(
        a.iter().any(|ad| ad.author
            == "Jorge Cwik, <jorge@laser.satlink.net> Arnt Gulbrandsen, <agulbra@nvg.unit.no>"),
        "Expected merged multiline author block, got: {:?}",
        a.iter().map(|ad| &ad.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_author_colon_empty_tail_collects_following_rst_roster_lines() {
    let input =
        "Authors:\n\t Richard Walker,\n\t Jamie Honan,\n\t Michael Hunold\n\nGeneral information\n";

    let raw_lines: Vec<&str> = input.lines().collect();
    let prepared_cache = PreparedLineCache::new(&raw_lines).materialize();
    let mut extracted: Vec<AuthorDetection> = Vec::new();
    extract_author_colon_blocks(&prepared_cache, &mut extracted);
    assert!(
        extracted
            .iter()
            .any(|ad| { ad.author == "Richard Walker, Jamie Honan, Michael Hunold" }),
        "Expected empty-tail Authors: block to merge following roster lines, got: {:?}",
        extracted.iter().map(|ad| &ad.author).collect::<Vec<_>>()
    );

    let (_c, _h, a) = super::super::detect_copyrights_from_text(input);
    assert!(
        a.iter()
            .any(|ad| ad.author == "Richard Walker, Jamie Honan, Michael Hunold"),
        "Expected pipeline to keep merged roster author block, got: {:?}",
        a.iter().map(|ad| &ad.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_extract_authors_from_dense_name_email_list() {
    let input = "John Doe <john@example.com>\nJane Smith <jane@example.com>\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "John Doe <john@example.com>"),
        "authors: {authors:?}"
    );
    assert!(
        authors
            .iter()
            .any(|a| a.author == "Jane Smith <jane@example.com>"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_comment_author_label_authors_keeps_obfuscated_angle_contact() {
    let raw_lines = vec!["* Author: Deepak M <m.deepak at intel.com>"];
    let authors = extract_comment_author_label_authors(&raw_lines);

    assert!(
        authors
            .iter()
            .any(|author| author.author == "Deepak M <m.deepak at intel.com>"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_comment_author_label_authors_requires_comment_evidence() {
    let raw_lines = vec![
        "! Author: Hunter Goatley",
        "| Author: Bill Davidsen",
        "author: package metadata value",
    ];
    let authors = extract_comment_author_label_authors(&raw_lines);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Hunter Goatley"), "authors: {values:?}");
    assert!(values.contains(&"Bill Davidsen"), "authors: {values:?}");
    assert!(
        !values.contains(&"package metadata value"),
        "authors: {values:?}"
    );
}

#[test]
fn test_detect_comment_author_label_when_grammar_has_no_author() {
    let input = "!  Program: CVTHELP.TPU\n!  Author: Hunter Goatley\n!  Date: January 12, 1992\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|author| author.author == "Hunter Goatley"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_comment_author_after_year_only_copyright_remains_holder_only() {
    let raw_lines = vec![
        "* Copyright (C) 2016-2018",
        "* Author: Matt Ranostay <matt.ranostay@konsulko.com>",
    ];
    let authors = extract_comment_author_label_authors(&raw_lines);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_drop_weak_authors_uses_bounded_local_attribution_evidence() {
    let raw_lines = vec![
        "Contributors, please see AUTHORS",
        "let the script author decide",
        "Written by chunchu",
        "re-indented in 2006 by commit 95b2444",
        "there are no missing authors in AUTHORS",
        "The one maintained by the Perl development team",
        "Changes by Gisle: optree dump",
    ];
    let mut authors = vec![
        AuthorDetection {
            author: "please".to_string(),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
        },
        AuthorDetection {
            author: "decide".to_string(),
            start_line: LineNumber::new(2).expect("valid line"),
            end_line: LineNumber::new(2).expect("valid line"),
        },
        AuthorDetection {
            author: "chunchu".to_string(),
            start_line: LineNumber::new(3).expect("valid line"),
            end_line: LineNumber::new(3).expect("valid line"),
        },
        AuthorDetection {
            author: "commit".to_string(),
            start_line: LineNumber::new(4).expect("valid line"),
            end_line: LineNumber::new(4).expect("valid line"),
        },
        AuthorDetection {
            author: "in AUTHORS".to_string(),
            start_line: LineNumber::new(5).expect("valid line"),
            end_line: LineNumber::new(5).expect("valid line"),
        },
        AuthorDetection {
            author: "the Perl".to_string(),
            start_line: LineNumber::new(6).expect("valid line"),
            end_line: LineNumber::new(6).expect("valid line"),
        },
        AuthorDetection {
            author: "Gisle".to_string(),
            start_line: LineNumber::new(7).expect("valid line"),
            end_line: LineNumber::new(7).expect("valid line"),
        },
    ];

    drop_weak_prose_authors(&raw_lines, &mut authors);

    assert_eq!(authors.len(), 2, "authors: {authors:?}");
    assert_eq!(authors[0].author, "chunchu");
    assert_eq!(authors[1].author, "Gisle");
}

#[test]
fn test_author_cleanup_respects_complete_line_and_hyphenated_prose_boundaries() {
    let raw_lines = vec![
        "| Written by Darren Salt",
        "| Assumes that unzipsfx is on Run$Path",
        "originally written by Martin Minow, poss-",
        "ibly modified by Jerry Leichter",
        "Created by Jason Hunter and Brett McLaughlin",
        "Revised by Ryusuke Konishi",
        "Pulled in another direction by Nick Ing-Simmons",
        "<nick AT ing-simmons DOT net>",
    ];
    let mut authors = vec![
        AuthorDetection {
            author: "Darren Salt Assumes".to_string(),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(2).expect("valid line"),
        },
        AuthorDetection {
            author: "Martin Minow, poss".to_string(),
            start_line: LineNumber::new(3).expect("valid line"),
            end_line: LineNumber::new(4).expect("valid line"),
        },
        AuthorDetection {
            author: "Jason Hunter and Brett McLaughlin Revised by Ryusuke Konishi".to_string(),
            start_line: LineNumber::new(5).expect("valid line"),
            end_line: LineNumber::new(6).expect("valid line"),
        },
        AuthorDetection {
            author: "Nick Ing-Simmons nick AT ing-simmons DOT net".to_string(),
            start_line: LineNumber::new(7).expect("valid line"),
            end_line: LineNumber::new(8).expect("valid line"),
        },
    ];

    repair_complete_by_line_author_boundaries(&raw_lines, &mut authors);
    repair_hyphenated_prose_tail_authors(&raw_lines, &mut authors);

    assert_eq!(authors[0].author, "Darren Salt");
    assert_eq!(authors[0].end_line, LineNumber::ONE);
    assert_eq!(authors[1].author, "Martin Minow");
    assert_eq!(authors[1].end_line, LineNumber::new(3).expect("valid line"));
    assert_eq!(
        authors[2].author,
        "Jason Hunter and Brett McLaughlin Revised by Ryusuke Konishi"
    );
    assert_eq!(authors[2].end_line, LineNumber::new(6).expect("valid line"));
    assert_eq!(
        authors[3].author,
        "Nick Ing-Simmons nick AT ing-simmons DOT net"
    );
    assert_eq!(authors[3].end_line, LineNumber::new(8).expect("valid line"));
}

#[test]
fn test_embedded_authors_product_name_is_not_an_author_label() {
    let raw_lines = vec!["Perl Authors Upload Server. Contains module links."];
    let mut authors = vec![AuthorDetection {
        author: "Upload Server".to_string(),
        start_line: LineNumber::ONE,
        end_line: LineNumber::ONE,
    }];

    drop_embedded_authors_title_phrases(&raw_lines, &mut authors);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_shadowed_multiline_author_overrun_stops_at_new_attribution() {
    let raw_lines = vec![
        "Contributed by Artur Bergman <sky AT example DOT net>",
        "Pulled in another direction by Nick Ing-Simmons",
        "Original author: Andy Dougherty andy@example.com.",
        "Additions by Chip Salzenberg",
    ];
    let mut authors = vec![
        AuthorDetection {
            author: "Artur Bergman sky AT example DOT net Pulled".to_string(),
            start_line: LineNumber::ONE,
            end_line: LineNumber::new(2).expect("valid line"),
        },
        AuthorDetection {
            author: "Artur Bergman sky AT example DOT net".to_string(),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
        },
        AuthorDetection {
            author: "Andy Dougherty andy@example.com Additions".to_string(),
            start_line: LineNumber::new(3).expect("valid line"),
            end_line: LineNumber::new(4).expect("valid line"),
        },
        AuthorDetection {
            author: "Andy Dougherty andy@example.com".to_string(),
            start_line: LineNumber::new(3).expect("valid line"),
            end_line: LineNumber::new(3).expect("valid line"),
        },
    ];

    drop_shadowed_multiline_author_overruns(&raw_lines, &mut authors);

    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert_eq!(
        values,
        vec![
            "Artur Bergman sky AT example DOT net",
            "Andy Dougherty andy@example.com"
        ]
    );

    let input = "Contributed by Artur Bergman <sky AT example DOT net>\n\
                 Pulled in another direction by Nick Ing-Simmons\n\
                 <nick AT example DOT net>";
    let (_copyrights, _holders, detected) = super::super::detect_copyrights_from_text(input);
    assert!(
        detected
            .iter()
            .all(|author| !author.author.ends_with(" Pulled")),
        "authors: {detected:?}"
    );
}

#[test]
fn test_conjoined_contact_attribution_continuation_is_preserved() {
    let raw_lines = vec![
        "Written by Sherm Pendley <sherm@example.com>",
        "and subsequently updated by Dominic Dunlop <dom@example.com>",
    ];
    let mut authors = vec![AuthorDetection {
        author: "Sherm Pendley <sherm@example.com>, and subsequently updated by Dominic Dunlop <dom@example.com>".to_string(),
        start_line: LineNumber::ONE,
        end_line: LineNumber::new(2).expect("valid line"),
    }];

    repair_contact_author_before_new_attribution(&raw_lines, &mut authors);

    assert!(authors[0].author.contains("Dominic Dunlop"));
    assert_eq!(authors[0].end_line, LineNumber::new(2).expect("valid line"));
}

#[test]
fn test_passive_product_creation_is_not_human_authorship() {
    let input = "Archive entry names behave like those created by ZipTool's MSDOS port.\n\
                 Therefore archives created by MacTool 1.0 (March 1999) need conversion.";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_passive_creation_keeps_person_and_contact_backed_authors() {
    let raw_lines = vec![
        "Files were created by Jane Doe.",
        "Archives were created by Release Tool 2.0.",
        "Files were created by maintainer@example.com.",
    ];
    let mut authors = vec![
        AuthorDetection {
            author: "Jane Doe".to_string(),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
        },
        AuthorDetection {
            author: "Release Tool 2.0".to_string(),
            start_line: LineNumber::new(2).expect("valid line"),
            end_line: LineNumber::new(2).expect("valid line"),
        },
        AuthorDetection {
            author: "maintainer@example.com".to_string(),
            start_line: LineNumber::new(3).expect("valid line"),
            end_line: LineNumber::new(3).expect("valid line"),
        },
    ];

    drop_passive_product_creation_authors(&raw_lines, &mut authors);

    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert_eq!(values, vec!["Jane Doe", "maintainer@example.com"]);
}

#[test]
fn test_subject_attribution_keeps_contact_backed_people() {
    let input = concat!(
        "under the same terms as Perl itself.\n",
        "\n",
        "This program is distributed in the hope that it will be useful, but\n",
        "without any warranty; without even the implied warranty of\n",
        "merchantability or fitness for a particular purpose.\n",
        "\n",
        "=head1 AUTHOR\n",
        "\n",
        "Pod::Simple was created by Sean M. Burke <sburke@cpan.org>.\n",
        "But don't bother him, he's retired.\n",
        "\n",
        "Pod::Simple is maintained by:\n",
        "\n",
        "=over\n",
        "\n",
        "=item * Allison Randal C<allison@perl.org>\n",
        "\n",
        "=back\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Sean M. Burke <sburke@cpan.org>"),
        "authors: {values:?}"
    );
}

#[test]
fn test_subject_attribution_uses_bounded_pod_author_section() {
    let input = concat!(
        "=head1 AUTHORS\n",
        "\n",
        "Test::Harness 2.64 is maintained by Andy Lester and on which this module is based.\n",
        "\n",
        "=head1 DESCRIPTION\n",
        "\n",
        "Archives were created by Release Tool 2.0.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Andy Lester"), "authors: {values:?}");
    assert!(
        values.iter().all(|author| !author.contains("Release Tool")),
        "authors: {values:?}"
    );
}

#[test]
fn test_contactless_narrative_attributions_require_person_names() {
    let input = concat!(
        "Fcntl and Socket have been rewritten by Nicholas Clark to use the new API.\n",
        "The entire library package, as authored by me, Ozan S.\n",
        "Yigit, is hereby placed in the public domain.\n",
        "The manual, as authored by me, Ada Lovelace,\n",
        "Most of the documentation is taken from JSON::XS by Marc Lehmann\n",
        "Archives were created by Release Tool 2.0.\n",
        "This file is written by Callgrind, and it is upwards compatible.\n",
        "Perl is developed by a global team of volunteers.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    for expected in [
        "Nicholas Clark",
        "Ozan S. Yigit",
        "Ada Lovelace",
        "Marc Lehmann",
    ] {
        assert!(values.contains(&expected), "missing {expected}: {values:?}");
    }
    for rejected in ["Release Tool", "Callgrind", "global team of volunteers"] {
        assert!(
            values.iter().all(|author| !author.contains(rejected)),
            "unexpected {rejected}: {values:?}"
        );
    }
    let ozan = authors
        .iter()
        .find(|author| author.author == "Ozan S. Yigit")
        .expect("wrapped first-person attribution");
    assert_eq!((ozan.start_line.get(), ozan.end_line.get()), (2, 3));
    let ada = authors
        .iter()
        .find(|author| author.author == "Ada Lovelace")
        .expect("single-line first-person attribution");
    assert_eq!((ada.start_line.get(), ada.end_line.get()), (4, 4));
}

#[test]
fn test_subject_maintainer_attribution_keeps_named_collectives() {
    let input = concat!(
        "Filter::Simple is now maintained by the Perl5-Porters. Please submit bugs.\n",
        "This package is currently maintained by the Perl Toolchain Gang.\n",
        "This package is now maintained by the Safe Team. Thus, use the supported API.\n",
        "The cache is maintained by the package manager.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"the Perl5-Porters"), "authors: {values:?}");
    assert!(
        values.contains(&"the Perl Toolchain Gang"),
        "authors: {values:?}"
    );
    assert!(values.contains(&"the Safe Team"), "authors: {values:?}");
    assert!(
        values
            .iter()
            .all(|author| !author.contains("package manager")),
        "authors: {values:?}"
    );
}

#[test]
fn test_contact_backed_change_attributions_cover_varied_actions() {
    let input = concat!(
        "- Updated backport to 5.6.1 by Steffen Mueller <smueller@cpan.org>.\n",
        "- Fix suggested by Joel C. Maslak <jmaslak@example.net>.\n",
        "# Threaded by Jarkko Hietaniemi <jhi@example.net> on 11/18/97\n",
        "sdbm_exists added to the library by Russ Allbery <rra@example.net>.\n",
        "Float and double added by gnb@example.net 22/11/89\n",
        "This fix provided by akt@example.net for Japanese.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    for expected in [
        "Steffen Mueller <smueller@cpan.org>",
        "Joel C. Maslak <jmaslak@example.net>",
        "Jarkko Hietaniemi <jhi@example.net>",
        "Russ Allbery <rra@example.net>",
        "gnb@example.net",
        "akt@example.net",
    ] {
        assert!(values.contains(&expected), "missing {expected}: {values:?}");
    }
}

#[test]
fn test_contact_backed_change_attribution_wraps_onto_next_line() {
    let input = concat!(
        "Additional portability changes by Andy Dougherty\n",
        "doughera@example.net.\n",
        "This work was all done by Roderick Schertler\n",
        "<roderick@example.net>. If you run Linux, thank him.\n",
        "Fix suggested by\n",
        "Slaven Rezic <slaven@example.net>.\n",
    );
    let raw_lines: Vec<&str> = input.lines().collect();
    let prepared = PreparedLineCache::new(&raw_lines).materialize();
    let extracted = extract_line_local_attribution_authors(&prepared);
    assert!(
        extracted
            .iter()
            .any(|author| author.author == "Andy Dougherty <doughera@example.net>"),
        "direct authors: {extracted:?}"
    );
    assert!(
        extracted
            .iter()
            .any(|author| author.author == "Slaven Rezic <slaven@example.net>"),
        "prepared: {:?}; direct authors: {extracted:?}",
        prepared
            .iter()
            .map(|line| line.prepared.to_string())
            .collect::<Vec<_>>()
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Andy Dougherty <doughera@example.net>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Roderick Schertler <roderick@example.net>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Slaven Rezic <slaven@example.net>"),
        "authors: {values:?}"
    );
}

#[test]
fn test_source_header_credit_variants_are_authors() {
    let input = concat!(
        "# Original by Neil Bowers <neil@example.net>; Tue Oct 4 1994\n",
        "# XD88/10 platform hints by Kaveh Ghazi (ghazi@example.net) 2/11/92\n",
        "# Merged with the distribution by\n",
        "# Andy Dougherty <andy@example.net>\n",
        "# First modified 6/30/96 by:\n",
        "# Luther Huffman, Example Systems, Inc., luther@example.net\n",
        "# Last modified May 2020 by:\n",
        "# David Romano, david@example.net\n",
        "# Original by first@example.net, modified by second@example.net\n",
        "# Configured by: Config Author, config@example.net\n",
        "# Improved by Jake Hamby <jake@example.net> to support both compilers\n",
        "# Originally contributed by: Mark Kettenis <mark@example.net> Dec 1998\n",
        "# DCL-ified by Peter Prymmer <peter@example.net> 22-DEC-1995\n",
        "$! DCL-ified by VMS Author <vms@example.net> 22-DEC-1995\n",
        "& Written 02-05-05 by Macro Author (macro@example.net)\n",
        "sub helper { # Compatibility code. Written by Alexandr Ciornii, version 0.23.\n",
        "(contributed by Peter J. Holzer, peter@example.net)\n",
        "DataBase Editor by Janick Bergeron [janick@example.net] for testing\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    for expected in [
        "Neil Bowers <neil@example.net>",
        "Kaveh Ghazi (ghazi@example.net)",
        "Andy Dougherty <andy@example.net>",
        "Luther Huffman, Example Systems, Inc., luther@example.net",
        "David Romano, david@example.net",
        "first@example.net",
        "second@example.net",
        "Config Author, config@example.net",
        "Jake Hamby <jake@example.net>",
        "Mark Kettenis <mark@example.net>",
        "Peter Prymmer <peter@example.net>",
        "VMS Author <vms@example.net>",
        "Macro Author (macro@example.net)",
        "Alexandr Ciornii",
        "Peter J. Holzer, peter@example.net",
        "Janick Bergeron janick@example.net",
    ] {
        assert!(values.contains(&expected), "missing {expected}: {values:?}");
    }
}

#[test]
fn test_source_header_credit_words_without_people_are_not_authors() {
    let input = concat!(
        "The result is original by design.\n",
        "The generated files are merged by the build tool.\n",
        "Compiler hints by default improve diagnostics.\n",
        "This file uses a compiler written by\n",
        "Jean Example. Send mail to support@example.net for a copy.\n",
        "Configured by root@localhost\n",
        "* Configured by     : root@localhost\n",
        "$! Written 02-05-05 by the build system\n",
        "value & Written by Build Generator, then cached.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_bounded_pod_author_action_credits_are_authors() {
    let input = concat!(
        "=head1 AUTHORS and MAINTENANCE\n",
        "\n",
        "Original math code by Mark Biggar, rewritten by Tels L<http://example.net/>\n",
        "in late 2000.\n",
        "\n",
        "Separated from the original and shaped with the help of John Peacock.\n",
        "\n",
        "Further streamlining (api_version 1 etc.) by Tels 2004-2007.\n",
        "\n",
        "Currently maintained by the Perl Toolchain Gang.\n",
        "\n",
        "This program was introduced by Andy Dougherty, based on earlier work by ",
        "Tom Christiansen. It is maintained by the Perl 5 Porters.\n",
        "\n",
        "Existing code by Graham Barr. Currently maintained by the Perl Porters. ",
        "Please report bugs elsewhere.\n",
        "\n",
        "=head1 DESCRIPTION\n",
        "\n",
        "A component written by Build Generator <tool@example.net>.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    for expected in [
        "Mark Biggar",
        "Tels",
        "John Peacock",
        "the Perl Toolchain Gang",
        "Andy Dougherty",
        "Tom Christiansen",
        "the Perl 5 Porters",
        "Graham Barr",
        "the Perl Porters",
    ] {
        assert!(values.contains(&expected), "missing {expected}: {values:?}");
    }
    assert!(
        values
            .iter()
            .all(|author| !author.contains("Build Generator")),
        "authors: {values:?}"
    );
}

#[test]
fn test_mixed_pod_author_copyright_heading_keeps_maintainer_credit() {
    let input = concat!(
        "=head1 AUTHOR AND COPYRIGHT\n",
        "\n",
        "Copyright 2013 Tom Christiansen; now maintained by Perl5 Porters.\n",
        "\n",
        "This documentation is free and may be redistributed.\n",
        "\n",
        "=head1 DESCRIPTION\n",
        "\n",
        "The output is maintained by Build Generator.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Perl5 Porters"), "authors: {values:?}");
    assert!(
        values
            .iter()
            .all(|author| !author.contains("Build Generator")),
        "authors: {values:?}"
    );
}

#[test]
fn test_pod_author_modification_history_recovers_embedded_action_credit() {
    let input = concat!(
        "=head1 Author and Modification History\n",
        "\n",
        "Modified by Damian Conway, 2001-09-10, v0.62.\n",
        "\n",
        "Modified implicit construction of nested objects.\n",
        "\n",
        "Modified by Casey West, 2000-11-08, v0.59.\n",
        "\n",
        "Added the ability for compile time class creation.\n",
        "\n",
        "Renamed to C<Class::Struct> and modified by Jim Miner, 1997-04-02.\n",
        "\n",
        "=cut\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Jim Miner"), "authors: {values:?}");
}

#[test]
fn test_pod_author_section_keeps_leading_author_before_maintainer_credit() {
    let input = concat!(
        "=head1 AUTHOR\n",
        "\n",
        "Graham Barr. Currently maintained by the Perl Porters. Please report all ",
        "bugs elsewhere.\n",
        "\n",
        "=cut\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Graham Barr"), "authors: {values:?}");
    assert!(values.contains(&"the Perl Porters"), "authors: {values:?}");
}

#[test]
fn test_contact_backed_non_author_by_phrases_are_not_authors() {
    let input = concat!(
        "This package is distributed by Example Support <support@example.net>.\n",
        "The service is owned by Example Corp <legal@example.net>.\n",
        "Send reports by email to bugs@example.net.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_wrapped_affiliation_contact_does_not_extend_author() {
    let input = concat!(
        "Written by Joe Meadows Jr, at the Fred Hutchinson Cancer Research Center\n",
        "BITNET: JOE@FHCRCVAX\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .all(|author| !author.author.contains("Cancer Research Center")),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_comment_author_label_authors_detects_doxygen_author_tags() {
    let raw_lines = vec![
        "*> \\author Univ. of California Berkeley",
        "*> \\author Univ. of Colorado Denver",
    ];
    let authors = extract_comment_author_label_authors(&raw_lines);

    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert!(
        values.contains(&"Univ. of California Berkeley"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Univ. of Colorado Denver"),
        "authors: {values:?}"
    );
}

#[test]
fn test_detect_doxygen_author_tag_roster_in_comment_block() {
    let input = concat!(
        "*  Authors:\n",
        "*> \\author Univ. of Tennessee\n",
        "*> \\author Univ. of California Berkeley\n",
        "*> \\author Univ. of Colorado Denver\n",
        "*> \\author NAG Ltd.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert!(
        values.contains(&"Univ. of Tennessee"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Univ. of California Berkeley"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Univ. of Colorado Denver"),
        "authors: {values:?}"
    );
    assert!(values.contains(&"NAG Ltd."), "authors: {values:?}");
}

#[test]
fn test_detect_multiline_comment_authors_block_after_year_only_copyright() {
    let input = concat!(
        "// Copyright (C) 1997-2001\n",
        "// Authors: Andrew Lumsdaine <lums@osl.iu.edu>\n",
        "//          Lie-Quan Lee     <llee@osl.iu.edu>\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|author| {
            author.author == "Andrew Lumsdaine <lums@osl.iu.edu> Lie-Quan Lee <llee@osl.iu.edu>"
        }),
        "authors: {authors:?}"
    );
}

#[test]
fn test_detect_explicit_author_label_roster_with_company_suffix() {
    let input = "// Author    : Antoine YESSAYAN, Paul RASCLE, EDF\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|author| author.author == "Antoine YESSAYAN, Paul RASCLE, EDF"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_split_author_project_copyright_metadata_block() {
    let input = concat!(
        "// Author    : Antoine YESSAYAN, Paul RASCLE, EDF\n",
        "// Project   : SALOME\n",
        "// Copyright : EDF 2001\n",
    );
    let (copyrights, holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|author| author.author == "Antoine YESSAYAN, Paul RASCLE, EDF"),
        "authors: {authors:?}"
    );
    assert!(
        copyrights
            .iter()
            .any(|copyright| copyright.copyright == "Copyright EDF 2001"),
        "copyrights: {copyrights:?}"
    );
    assert!(
        holders.iter().any(|holder| holder.holder == "EDF"),
        "holders: {holders:?}"
    );
}

#[test]
fn test_extract_collective_author_with_contributors_before_email() {
    let input = "authors = [\"Tokio Contributors <team@tokio.rs>\"]\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "Tokio Contributors <team@tokio.rs>"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_toml_singular_author_array_with_handle() {
    let input = "author = [\"Tom Breloff (@tbreloff)\"]\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Tom Breloff"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_toml_singular_author_array_with_comma_handle_suffix() {
    let input = "authors = [\"RustCrypto Developers, zer0x64\"]\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "RustCrypto Developers"),
        "authors: {authors:?}"
    );
    assert!(
        !authors
            .iter()
            .any(|a| a.author == "RustCrypto Developers, zer0x64"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_toml_singular_author_array_keeps_company_suffix() {
    let input = "authors = [\"Jane Street Group, LLC\"]\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Jane Street Group, LLC"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_created_by_author_with_handle() {
    let input = "Created by Tom Breloff (@tbreloff)\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Tom Breloff"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_primary_author_with_handle() {
    let input = "Primary author: Josef Heinen (@jheinen)\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Josef Heinen"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_original_author_with_handle() {
    let input = "Original author: Thomas Breloff (@tbreloff)\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Thomas Breloff"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_primary_package_author_with_handle() {
    let input = "Primary PlotlyJS.jl author: Spencer Lyon (@spencerlyon2)\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Spencer Lyon"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_author_colon_inline_roster_with_handles() {
    let input = "authors: Benoit Pasquier (@briochemc) - David Gustavsson (@gustaphe) - Jan Weidner (@jw3126)\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Benoit Pasquier"),
        "authors: {authors:?}"
    );
    assert!(
        authors.iter().any(|a| a.author == "David Gustavsson"),
        "authors: {authors:?}"
    );
    assert!(
        authors.iter().any(|a| a.author == "Jan Weidner"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_author_colon_markdown_block_strips_trailing_bare_handle() {
    let input = "Authors:\nEvan Sheng (evan.sheng@airbnb.com) @evansheng\n\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|author| author.author == "Evan Sheng (evan.sheng@airbnb.com)"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_markdown_heading_original_author_with_handle() {
    let input = "### Original author: Thomas Breloff (@tbreloff)\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Thomas Breloff"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_original_author_before_maintained_by_clause() {
    let input =
        "### Original author: Thomas Breloff (@tbreloff), maintained by the JuliaPlots members\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Thomas Breloff"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_extract_originally_implemented_by_author_with_parenthesized_email() {
    let input = "LALR(1) support was originally implemented by Elias Ioup (ezioup@alumni.uchicago.edu),\nusing the algorithm found in Aho, Sethi, and Ullman.\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "Elias Ioup (ezioup@alumni.uchicago.edu)"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_was_developed_by_multiline_author_is_extracted() {
    let input = "1. GOST R 34.11-2012 was developed by the Center for Information\nProtection and Special Communications of the Federal Security\nService of the Russian Federation with participation of the Open\n";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    assert!(
        authors.iter().any(|a| {
            a.author
                == "the Center for Information Protection and Special Communications of the Federal Security Service of the Russian Federation"
        }),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_written_by_author_email_for_project_is_extracted() {
    let input = "Written by Andy Polyakov <appro@openssl.org> for the OpenSSL\nproject.";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    assert!(
        authors
            .iter()
            .any(|a| a.author == "Andy Polyakov <appro@openssl.org>"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_written_by_author_with_contact_after_copyright_is_kept() {
    let input = concat!(
        "Copyright 2021-2025 The OpenSSL Project Authors. All Rights Reserved.\n",
        "\n",
        "Written by Ben Avison <bavison@riscosopen.org> for the OpenSSL\n",
        "project. Rights for redistribution and usage in source and binary\n",
        "forms are granted according to the OpenSSL license.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    assert!(
        authors
            .iter()
            .any(|a| a.author == "Ben Avison <bavison@riscosopen.org>"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_standalone_written_by_header_after_copyright_is_extracted() {
    let input = "/* Copyright (C) 2003 Epic Games\n   Written by Jean-Marc Valin */\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Jean-Marc Valin"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_weak_standalone_written_by_header_after_copyright_is_dropped() {
    let input = "/* Copyright (C) 2003 Epic Games\n   Written by Jean-Marc Valin and others */\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_multiline_written_by_and_continuation_is_included() {
    let input =
        "/* Copyright (C) 2003 Epic Games\n   Written by Jean-Marc Valin,\n   and Yunho Huh */\n";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    let author = authors
        .iter()
        .find(|a| a.author == "Jean-Marc Valin, and Yunho Huh")
        .expect("expected multiline written-by author continuation");

    assert_eq!(author.start_line, LineNumber::new(2).expect("valid"));
    assert_eq!(author.end_line, LineNumber::new(3).expect("valid"));
}

#[test]
fn test_single_line_written_by_author_list_preserves_final_author() {
    let input = concat!(
        "/* Copyright (c) 2008-2011 Xiph.Org Foundation, Mozilla Corporation,\n",
        "                           Gregory Maxwell\n",
        "   Written by Jean-Marc Valin, Gregory Maxwell, and Timothy B. Terriberry */\n",
    );

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| { a.author == "Jean-Marc Valin, Gregory Maxwell, and Timothy B. Terriberry" }),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_originally_written_by_for_project_block_without_contact_is_extracted() {
    let input = concat!(
        "Originally written by Christophe Renou and Peter Sylvester,\n",
        "for the EdelKey project.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    assert!(
        authors
            .iter()
            .any(|a| a.author == "Christophe Renou and Peter Sylvester"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_prose_snippet_does_not_report_laboriously_took_the_trouble_as_author() {
    let input = concat!(
        "<para>the authors laboriously took the trouble of searching for workarounds ",
        "to make these compilers happy</para>",
    );

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {:?}", authors);
}

#[test]
fn test_developed_by_sentence_author_is_extracted() {
    let input = "developed by the U.S. Government. BAE Systems is enhancing and supporting the SMP";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    assert!(
        authors
            .iter()
            .any(|a| a.author == "the U.S. Government. BAE Systems"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_developed_by_phrase_author_is_extracted() {
    let input = "to acknowledge that it was\n      developed by the National Center for Supercomputing Applications at the University of Illinois at Urbana-Champaign and to credit the\n      contributors.";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    assert!(
        authors.iter().any(|a| {
            a.author
                == "the National Center for Supercomputing Applications at the University of Illinois at Urbana-Champaign"
        }),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_notice_developed_by_multiline_collective_author_is_extracted() {
    let input = concat!(
        "This product includes software developed by\n",
        "The Apache Software Foundation (http://www.apache.org/).\n",
    );

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "The Apache Software Foundation (http://www.apache.org/)"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_notice_developed_by_quoted_project_author_is_extracted() {
    let input = "\"This product includes software developed by the Spring Framework Project (http://www.springframework.org).\"";

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "the Spring Framework Project (http://www.springframework.org)"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_modified_portion_developed_by_author_with_url_is_extracted() {
    let input = concat!(
        "# This product contains a modified portion of 'Flask App Builder' developed by Daniel Vaz Gaspar.\n",
        "# (https://github.com/dpgaspar/Flask-AppBuilder).\n",
    );

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    assert!(
        authors.iter().any(
            |a| a.author == "Daniel Vaz Gaspar. (https://github.com/dpgaspar/Flask-AppBuilder)"
        ),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_author_colon_block_stops_at_status_and_devices_metadata() {
    let input = "Author: ds\nStatus: works in immediate mode\nDevices: [standard] parallel port\n";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.is_empty(),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_author_colon_block_keeps_named_author_without_devices_tail() {
    let input =
        "Author: Pablo Mejia <pablo.mejia@cctechnol.com>\nDevices: [Access I/O] PC-104 AIO12-8\n";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "Pablo Mejia <pablo.mejia@cctechnol.com>"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_written_by_comma_and_copyright_keeps_parenthesized_email_author() {
    let input =
        "written by Philip Hazel, and copyright\nby the University of Cambridge, England.\n";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Philip Hazel"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_with_additional_hacking_by_keeps_parenthesized_email_author() {
    let input = "With additional hacking by Jeffrey Kuskin (jsk@mojave.stanford.edu)\n";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "Jeffrey Kuskin (jsk@mojave.stanford.edu)"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_adapted_written_by_keeps_parenthesized_email_author() {
    let input = "Adapted from baycom.c driver written by Thomas Sailer (sailer@ife.ee.ethz.ch)\n";

    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|a| a.author == "Thomas Sailer (sailer@ife.ee.ethz.ch)"),
        "authors: {:?}",
        authors.iter().map(|a| &a.author).collect::<Vec<_>>()
    );
}

#[test]
fn test_multiline_written_and_maintained_by_block_extracts_individual_authors() {
    let input = concat!(
        "GNU tar, heavily based on John Gilmore's public domain version of tar,\n",
        "was originally written by Graham Todd.\n",
        "It is now maintained by Sergey Poznyakoff.\n",
        "This package is maintained for Debian by Janos Lenart <ocsi@debian.org>.\n",
    );

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);
    let authors: Vec<String> = authors.into_iter().map(|a| a.author).collect();

    assert!(
        authors.iter().any(|a| a == "Graham Todd"),
        "authors: {authors:#?}"
    );
    assert!(
        authors.iter().any(|a| a == "Sergey Poznyakoff"),
        "authors: {authors:#?}"
    );
    assert!(
        authors
            .iter()
            .any(|a| a == "Janos Lenart <ocsi@debian.org>"),
        "authors: {authors:#?}"
    );
    assert!(
        !authors
            .iter()
            .any(|a| a.contains("GNU tar, heavily based on")),
        "authors: {authors:#?}"
    );
}

#[test]
fn test_rst_field_author_and_maintainer_extracts_single_author() {
    let input = ":License:\t\tGPLv2\n:Author & Maintainer:\tMiguel Ojeda <ojeda@kernel.org>\n:Date:\t\t\t2006-10-27\n";

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert!(
        values.contains(&"Miguel Ojeda <ojeda@kernel.org>"),
        "authors: {values:?}"
    );
}

#[test]
fn test_dash_bullet_changelog_lines_extract_individual_authors() {
    let input = "- Written by Mydraal <vulpyne@vulpyne.net>\n- Updated by Adam Sulmicki <adam@cfar.umd.edu>\n- Updated by Jeremy M. Dolan <jmd@turbogeek.org> 2001/01/28 10:15:59\n- Added to by Crutcher Dunnavant <crutcher+kernel@datastacks.com>\n";

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert!(
        values.contains(&"Mydraal <vulpyne@vulpyne.net>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Adam Sulmicki <adam@cfar.umd.edu>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Jeremy M. Dolan <jmd@turbogeek.org>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Crutcher Dunnavant <crutcher+kernel@datastacks.com>"),
        "authors: {values:?}"
    );
    assert!(
        !values
            .iter()
            .any(|value| value.contains("Updated by Adam Sulmicki")),
        "authors: {values:?}"
    );
}

#[test]
fn test_plaintext_roster_lines_extract_individual_authors() {
    let input = concat!(
        "ada/        by Dmitriy Anisimkov <anisimkov@yahoo.com>\n",
        "        Support for Ada\n",
        "iostream3/  by Ludwig Schwardt <schwardt@sun.ac.za>\n",
        "            and Kevin Ruland <kevin@rodin.wustl.edu>\n",
        "            and Mark Adler <madler@alumni.caltech.edu>\n",
        "minizip/    by Gilles Vollant <info@winimage.com>\n",
        "        Includes Zip64 support by Mathias Svensson <mathias@result42.com>\n",
        "pascal/     by Bob Dellaca <bobdl@xtra.co.nz> et al.\n",
        "        Support for Pascal\n",
    );

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Dmitriy Anisimkov <anisimkov@yahoo.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Ludwig Schwardt <schwardt@sun.ac.za>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Kevin Ruland <kevin@rodin.wustl.edu>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Mark Adler <madler@alumni.caltech.edu>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Gilles Vollant <info@winimage.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Mathias Svensson <mathias@result42.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Bob Dellaca <bobdl@xtra.co.nz> et al"),
        "authors: {values:?}"
    );
    assert!(
        !values
            .iter()
            .any(|value| *value == "Support for Ada" || *value == "Support for Pascal"),
        "authors: {values:?}"
    );
}

#[test]
fn test_written_on_top_of_line_extracts_author() {
    let input = concat!(
        "An experimental package to read and write files in the .zip format, written on top of\n",
        "zlib by Gilles Vollant <info@winimage.com>, is available in the\n",
        "contrib/minizip directory of zlib.\n",
    );

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors
            .iter()
            .any(|author| author.author == "Gilles Vollant <info@winimage.com>"),
        "authors: {authors:?}"
    );
}

#[test]
fn test_author_colon_dash_bullet_hwmon_roster_extracts_individual_authors() {
    let input = "Authors:\n\t- Mark M. Hoffman <mhoffman@lightlink.com>\n\t- Ported to 2.6 by Eric J. Bowersox <ericb@aspsys.com>\n\t- Adapted to 2.6.20 by Carsten Emde <ce@osadl.org>\n\t- Modified for mainline integration by Hans J. Koch <hjk@hansjkoch.de>\n";

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();
    assert!(
        values.contains(&"Mark M. Hoffman <mhoffman@lightlink.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Eric J. Bowersox <ericb@aspsys.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Carsten Emde <ce@osadl.org>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Hans J. Koch <hjk@hansjkoch.de>"),
        "authors: {values:?}"
    );
}

#[test]
fn test_passive_written_phrase_does_not_create_abi_author_false_positive() {
    let input = "Description:\tWhen read, this file returns general data like firmware version.\n\t\tWhen written, the device can be reset.\n\t\tBefore reading this file, control has to be written to select\n\t\twhich profile to read.\n";

    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);
    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_detect_author() {
    let (c, h, a) = super::super::detect_copyrights_from_text("Written by John Doe");
    assert!(c.is_empty(), "Should not detect copyright");
    assert!(h.is_empty(), "Should not detect holder");
    assert_eq!(a.len(), 1, "Should detect one author, got: {:?}", a);
    assert_eq!(a[0].author, "John Doe");
    assert_eq!(a[0].start_line, LineNumber::ONE);
    assert_eq!(a[0].end_line, LineNumber::ONE);
}

#[test]
fn test_written_by_author_stops_before_following_notice_sentence() {
    let input = "Copyright (c) 1986 by University of Toronto.\n\
                 Written by Henry Spencer. Not derived from licensed software.";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert_eq!(authors.len(), 1, "authors: {authors:?}");
    assert_eq!(authors[0].author, "Henry Spencer");
}

#[test]
fn test_detect_author_from_xml_author_attribute() {
    let text = r#"<note author="Vinnie Falco">C++11 is the minimum requirement.</note>"#;
    let (c, h, a) = super::super::detect_copyrights_from_text(text);

    assert!(c.is_empty(), "Should not detect copyright");
    assert!(h.is_empty(), "Should not detect holder");
    assert_eq!(a.len(), 1, "Should detect one author, got: {:?}", a);
    assert_eq!(a[0].author, "Vinnie Falco");
    assert_eq!(a[0].start_line, LineNumber::ONE);
    assert_eq!(a[0].end_line, LineNumber::ONE);
}

#[test]
fn test_detect_author_from_xml_author_attribute_without_note_body_noise() {
    let text = r#"<note author="Chris Kohlhoff">
This compiler does not support enable_if, which is needed by the library.
</note>"#;
    let (_c, _h, a) = super::super::detect_copyrights_from_text(text);

    assert_eq!(a.len(), 1, "Should detect one author, got: {:?}", a);
    assert_eq!(a[0].author, "Chris Kohlhoff");
}

#[test]
fn test_detect_author_from_xml_author_attribute_decodes_entities() {
    let text = r#"<note author="Joaqu&#237;n M L&#243;pez Mu&#241;oz">Compiler bug.</note>"#;
    let (_c, _h, a) = super::super::detect_copyrights_from_text(text);

    assert_eq!(a.len(), 1, "Should detect one author, got: {:?}", a);
    assert_eq!(a[0].author, "Joaquín M López Muñoz");
}

#[test]
fn test_detect_author_from_repeated_xml_author_attributes_keeps_multiple_occurrences() {
    let text = r#"<mark-expected-failures>
<note author="Aleksey Gurtovoy" refid="4"/>
<note author="Aleksey Gurtovoy" refid="19"/>
</mark-expected-failures>"#;
    let (_c, _h, a) = super::super::detect_copyrights_from_text(text);

    let matching: Vec<_> = a
        .iter()
        .filter(|ad| ad.author == "Aleksey Gurtovoy")
        .collect();
    assert_eq!(matching.len(), 2, "authors: {a:#?}");
    assert_eq!(matching[0].start_line, LineNumber::new(2).expect("valid"));
    assert_eq!(matching[1].start_line, LineNumber::new(3).expect("valid"));
}

#[test]
fn test_detect_author_from_xml_author_attribute_splits_obvious_multi_name_lists() {
    let text = r#"<note author="Robert Ramey,Roland Schwarz" date="16 Feb 07" refid="19"/>"#;
    let (_c, _h, a) = super::super::detect_copyrights_from_text(text);

    let names: Vec<&str> = a.iter().map(|ad| ad.author.as_str()).collect();
    assert!(names.contains(&"Robert Ramey"), "authors: {names:?}");
    assert!(names.contains(&"Roland Schwarz"), "authors: {names:?}");
    assert_eq!(names.len(), 2, "authors: {names:?}");
}

#[test]
fn test_detect_docbook_html_authorgroup_authors() {
    let text = r#"<div class="authorgroup">
<div class="author"><h3 class="author"><span class="firstname">John</span> <span class="surname">Maddock</span></h3></div>
<div class="author"><h3 class="author"><span class="firstname">Joel</span> <span class="surname">de Guzman</span></h3></div>
<div class="author"><h3 class="author"><span class="firstname">Eric</span> <span class="surname">Niebler</span></h3></div>
<div class="author"><h3 class="author"><span class="firstname">Matias</span> <span class="surname">Capeletto</span></h3></div>
</div>"#;
    let (_c, _h, a) = super::super::detect_copyrights_from_text(text);
    let names: Vec<&str> = a.iter().map(|d| d.author.as_str()).collect();

    assert!(names.contains(&"John Maddock"), "authors: {names:?}");
    assert!(names.contains(&"Joel de Guzman"), "authors: {names:?}");
    assert!(names.contains(&"Eric Niebler"), "authors: {names:?}");
    assert!(names.contains(&"Matias Capeletto"), "authors: {names:?}");
}

#[test]
fn test_detect_created_by_current_user_comment_is_not_author() {
    let text = "Get the IDs of pipelines created by the current user on the same branch.";
    let (_c, _h, a) = super::super::detect_copyrights_from_text(text);
    assert!(a.is_empty(), "authors: {a:?}");
}

#[test]
fn test_detect_author_written_by() {
    let (_c, _h, a) = super::super::detect_copyrights_from_text("Written by Jane Smith");
    assert_eq!(a.len(), 1, "Should detect one author, got: {:?}", a);
    assert_eq!(a[0].author, "Jane Smith");
    assert_eq!(a[0].start_line, LineNumber::ONE);
    assert_eq!(a[0].end_line, LineNumber::ONE);
}

#[test]
fn test_detect_author_maintained_by() {
    let (_c, _h, a) = super::super::detect_copyrights_from_text("Maintained by Bob Jones");
    assert_eq!(a.len(), 1, "Should detect one author, got: {:?}", a);
    assert_eq!(a[0].author, "Bob Jones");
    assert_eq!(a[0].start_line, LineNumber::ONE);
    assert_eq!(a[0].end_line, LineNumber::ONE);
}

#[test]
fn test_detect_author_authors_keyword() {
    let (_c, _h, a) = super::super::detect_copyrights_from_text("Authors John Smith");
    assert_eq!(
        a.len(),
        1,
        "Should detect author from 'Authors', got: {:?}",
        a
    );
    assert!(
        a[0].author.contains("John Smith"),
        "Author: {}",
        a[0].author
    );
}

#[test]
fn test_detect_author_contributors_keyword() {
    let (_c, _h, a) = super::super::detect_copyrights_from_text("Contributors Jane Doe");
    assert_eq!(
        a.len(),
        1,
        "Should detect author from 'Contributors', got: {:?}",
        a
    );
    assert!(a[0].author.contains("Jane Doe"), "Author: {}", a[0].author);
}

#[test]
fn test_detect_author_spdx_contributor() {
    let (_c, _h, a) =
        super::super::detect_copyrights_from_text("SPDX-FileContributor: Alice Johnson");
    assert_eq!(
        a.len(),
        1,
        "Should detect author from SPDX-FileContributor, got: {:?}",
        a
    );
    assert!(
        a[0].author.contains("Alice Johnson"),
        "Author: {}",
        a[0].author
    );
}

#[test]
fn test_name_contributed_line_is_detected_as_author() {
    let input = "\\author{\nRandall Prium contributed most of the implementation of\n\\code{cut_width()}.\n}";
    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.iter().any(|a| a.author == "Randall Prium"),
        "expected Randall Prium author, got: {:?}",
        authors
    );
}

#[test]
fn test_changes_by_attribution_is_detected_inline_and_across_lines() {
    let input = concat!(
        "This platform file is based on unix.c; changes by Ruslan Nickolaev (nruslan@hotbox.ru)\n",
        "This other port is based on unix.c; changes\n",
        " by Chris Herborth (chrish@pobox.com).\n",
        "Changes by Gisle: optree dump.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Ruslan Nickolaev (nruslan@hotbox.ru)"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Chris Herborth (chrish@pobox.com)"),
        "authors: {values:?}"
    );
    assert!(values.contains(&"Gisle"), "authors: {values:?}");
    assert!(
        values.iter().all(|author| !author.contains("optree dump")),
        "authors: {values:?}"
    );
}

#[test]
fn test_consecutive_line_local_attributions_remain_distinct() {
    let input = concat!(
        "Copyright (C) 2004 Example Corp.\n",
        "Written by Ralph Metzler\n",
        "Overhauled by Holger Waechtler\n",
        "Driver support by Michael Hunold <hunold@example.com>\n",
        "Support by Charles Bailey bailey@example.edu. OS/2 support\n",
        "Ported to HID by Benjamin Tissoires <benjamin@example.com>\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Ralph Metzler"), "authors: {values:?}");
    assert!(values.contains(&"Holger Waechtler"), "authors: {values:?}");
    assert!(
        values.contains(&"Michael Hunold <hunold@example.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Charles Bailey bailey@example.edu"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Benjamin Tissoires <benjamin@example.com>"),
        "authors: {values:?}"
    );
    assert!(
        values
            .iter()
            .all(|author| !author.contains("Overhauled by")),
        "authors: {values:?}"
    );
}

#[test]
fn test_line_local_attribution_does_not_match_passive_prose() {
    let input = concat!(
        "The implementation was revised by adding one check.\n",
        "The default support is maintained by the package manager.\n",
        "Changes were introduced by default.\n",
        "DTMF code (c) 1996 by Christian Mock (cm@example.com).\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_chained_active_attributions_are_distinct_authors() {
    let input = concat!(
        "AUTHORS\n",
        "The implementation was written by Brandon L Black. ",
        "Nicholas Clark created the pluggable interface.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Brandon L Black"), "authors: {values:?}");
    assert!(values.contains(&"Nicholas Clark"), "authors: {values:?}");
    assert!(
        values
            .iter()
            .all(|author| !author.contains("Black. Nicholas")),
        "authors: {values:?}"
    );
}

#[test]
fn test_chained_action_attribution_keeps_contactless_people() {
    let input = concat!(
        "Created by Rodney Brown from an earlier version, modified by Chr. Spieler.\n",
        "This stat() is by Paul Wells, modified by Paul Kienitz.\n",
        "Output created by Release Tool, modified by Build Generator.\n",
        "The output is by default, modified by Build Generator.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Rodney Brown"), "authors: {values:?}");
    assert!(values.contains(&"Chr. Spieler"), "authors: {values:?}");
    assert!(values.contains(&"Paul Wells"), "authors: {values:?}");
    assert!(values.contains(&"Paul Kienitz"), "authors: {values:?}");
    assert!(
        values
            .iter()
            .all(|author| !author.contains("Build Generator")),
        "authors: {values:?}"
    );
}

#[test]
fn test_wrapped_contribution_role_supports_leading_by_author() {
    let input = concat!(
        "AUTHORS\n",
        "Support by Charles Bailey <charles@example.com>. OS/2 support\n",
        "by Ilya Zakharevich <ilya@example.com>.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Charles Bailey <charles@example.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.contains(&"Ilya Zakharevich <ilya@example.com>"),
        "authors: {values:?}"
    );
}

#[test]
fn test_contact_backed_conjoined_attribution_continues_on_next_line() {
    let input = concat!(
        "AUTHORS\n",
        "Mac support by Paul Schinder C<< <paul@example.com> >>, and\n",
        "Thomas Wegner C<< <thomas@example.com> >>.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values
            .contains(&"Paul Schinder <paul@example.com>, and Thomas Wegner <thomas@example.com>"),
        "authors: {values:?}"
    );
}

#[test]
fn test_second_by_author_name_continues_across_pod_lines() {
    let input = concat!(
        "AUTHORS\n",
        "Originally written by Yves Orton, expanded by E<AElig>var ArnfjE<ouml>rE<eth>\n",
        "Bjarmason.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(values.contains(&"Yves Orton"), "authors: {values:?}");
    assert!(
        values.contains(&"Ævar Arnfjörð Bjarmason"),
        "authors: {values:?}"
    );
    assert!(
        values.iter().all(|author| author != &"Ævar Arnfjörð"),
        "authors: {values:?}"
    );
}

#[test]
fn test_pod_contact_author_stops_at_following_sentence() {
    let input = concat!(
        "This project was maintained by Dan Kogai I<< <dankogai@cpan.org> >>.  ",
        "See AUTHORS for everyone involved.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert_eq!(values, vec!["Dan Kogai <dankogai@cpan.org>"]);
}

#[test]
fn test_contribution_author_chain_drops_shadowed_prefix() {
    let input = concat!(
        "=head1 AUTHOR\n",
        "\n",
        "Pod::Simple::JustPod was developed by John SJ Anderson\n",
        "C<genehack@genehack.org>, with contributions from Karl Williamson\n",
        "C<khw@cpan.org>.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(
            &"John SJ Anderson genehack@genehack.org, with contributions from Karl Williamson khw@cpan.org"
        ),
        "authors: {values:?}"
    );
    assert!(
        values
            .iter()
            .all(|author| *author != "John SJ Anderson genehack@genehack.org"),
        "authors: {values:?}"
    );
}

#[test]
fn test_same_span_author_list_drops_shadowed_prefix() {
    let mut authors = vec![
        AuthorDetection {
            author: "Ken Williams <ken@example.net>".to_string(),
            start_line: LineNumber::new(3).expect("line"),
            end_line: LineNumber::new(4).expect("line"),
        },
        AuthorDetection {
            author: concat!(
                "Ken Williams <ken@example.net>, ",
                "Randy Sims <randy@example.net>"
            )
            .to_string(),
            start_line: LineNumber::new(3).expect("line"),
            end_line: LineNumber::new(4).expect("line"),
        },
    ];

    drop_shadowed_author_prefixes(&mut authors);

    assert_eq!(authors.len(), 1, "authors: {authors:?}");
    assert!(authors[0].author.contains("Randy Sims"));
}

#[test]
fn test_wrapped_contact_author_stops_before_new_attribution() {
    let input = concat!(
        "Widget was written by Raphael Manfredi\n",
        "F<E<lt>Raphael_Manfredi@pobox.comE<gt>>\n",
        "Maintenance is now done by the Widget team.\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = authors
        .iter()
        .map(|author| author.author.as_str())
        .collect();

    assert!(
        values.contains(&"Raphael Manfredi <Raphael_Manfredi@pobox.com>"),
        "authors: {values:?}"
    );
    assert!(
        values.iter().all(|author| !author.contains("Maintenance")),
        "authors: {values:?}"
    );
}

#[test]
fn test_pod_copyright_heading_does_not_duplicate_final_line() {
    let input = concat!(
        "=head1 COPYRIGHT\n",
        "\n",
        "Copyright 2002-2014 Dan Kogai I<< <dankogai@cpan.org> >>.\n",
    );
    let (copyrights, _holders, _authors) = super::super::detect_copyrights_from_text(input);
    let values: Vec<&str> = copyrights
        .iter()
        .map(|copyright| copyright.copyright.as_str())
        .collect();

    assert_eq!(
        values,
        vec!["Copyright 2002-2014 Dan Kogai <dankogai@cpan.org>"]
    );
}

#[test]
fn test_changes_by_prose_does_not_create_an_author() {
    let input = concat!(
        "The value changes by adding one to the previous result.\n",
        "The rate changes by 2 percent.\n",
        "MAJOR CHANGES BY BETA VERSION\n",
    );
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(input);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_name_contributed_line_ignores_portions_holder_phrase() {
    let input = "Copyright (c) 2006, Industrial Light & Magic, a division of Lucasfilm\nEntertainment Company Ltd. Portions contributed and copyright held by\nothers as indicated. All rights reserved.";
    let (_c, _h, authors) = super::super::detect_copyrights_from_text(input);

    assert!(
        authors.is_empty(),
        "expected no authors, got: {:?}",
        authors
    );
}

#[test]
fn test_date_by_author() {
    let content = "\
Copyright (c) 1998 Softweyr LLC.  All rights reserved.
strtok_r, from Berkeley strtok
Oct 13, 1998 by Wes Peters <wes@softweyr.com>";
    let (_c, _h, a) = super::super::detect_copyrights_from_text(content);
    assert!(
        a.iter().any(|a| a.author.contains("Wes Peters")),
        "Should detect Wes Peters as author, got: {:?}",
        a
    );
}

#[test]
fn test_originally_by_author() {
    let content = "\
#   Copyright 1996-2006 Free Software Foundation, Inc.
#   Taken from GNU libtool, 2001
#   Originally by Gordon Matzigkeit <gord@gnu.ai.mit.edu>, 1996";
    let (_c, _h, a) = super::super::detect_copyrights_from_text(content);
    assert!(
        a.iter().any(|a| a.author.contains("Gordon Matzigkeit")),
        "Should detect Gordon Matzigkeit as author, got: {:?}",
        a
    );
}

#[test]
fn test_schema_author_column_does_not_create_an_author() {
    let content = "\
CREATE TABLE author(
    id INTEGER PRIMARY KEY,
    name text NOT NULL
);
CREATE TABLE book(
    id INTEGER PRIMARY KEY,
    author INTEGER REFERENCES author (id),
    title text NOT NULL
);";
    let (_copyrights, _holders, authors) = super::super::detect_copyrights_from_text(content);

    assert!(authors.is_empty(), "authors: {authors:?}");
}

#[test]
fn test_markdown_contributor_roster_emits_one_author_per_entry() {
    let raw_lines = vec![
        "## Contributors",
        "",
        "* [Alice Smith](https://example.com/alice)",
        "* [Bob Jones](https://example.com/bob)",
        "* [Docs](https://example.com/docs)",
    ];
    let authors: Vec<_> = extract_markdown_contributor_roster_authors(&raw_lines)
        .into_iter()
        .map(|a| a.author)
        .collect();

    assert_eq!(
        authors,
        vec![
            "Alice Smith (https://example.com/alice)",
            "Bob Jones (https://example.com/bob)",
        ]
    );
}

#[test]
fn test_markdown_contributor_roster_requires_heading_and_two_people() {
    let no_heading = vec![
        "## Links",
        "* [Alice Smith](https://example.com/alice)",
        "* [Bob Jones](https://example.com/bob)",
    ];
    let single_entry = vec![
        "## Contributors",
        "* [Alice Smith](https://example.com/alice)",
    ];
    let no_contact = vec!["## Contributors", "* Alice Smith", "* Bob Jones"];
    let keyword_only_in_url = vec![
        "## [Related reading](https://example.com/authors)",
        "* [Alice Smith](https://example.com/alice)",
        "* [Bob Jones](https://example.com/bob)",
    ];

    assert!(extract_markdown_contributor_roster_authors(&no_heading).is_empty());
    assert!(extract_markdown_contributor_roster_authors(&single_entry).is_empty());
    assert!(extract_markdown_contributor_roster_authors(&no_contact).is_empty());
    assert!(extract_markdown_contributor_roster_authors(&keyword_only_in_url).is_empty());

    let fenced_example = vec![
        "```markdown",
        "## Contributors",
        "* [Alice Smith](https://example.com/alice)",
        "* [Bob Jones](https://example.com/bob)",
        "```",
    ];
    assert!(extract_markdown_contributor_roster_authors(&fenced_example).is_empty());
}
