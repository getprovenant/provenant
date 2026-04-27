// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

mod dependencies;
mod licenses;
mod properties;
mod state;

use self::{properties::sanitize_template_directives, state::PomParseState};
use super::default_package_data;
use crate::models::{DatasourceId, PackageData};
use crate::parser_warn as warn;
use crate::parsers::utils::{MAX_ITERATION_COUNT, read_file_to_string};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::borrow::Cow;
use std::path::Path;

pub(super) fn parse_pom_xml(path: &Path) -> Vec<PackageData> {
    let content = match read_file_to_string(path, None).map_err(|e| e.to_string()) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to open pom.xml at {:?}: {}", path, e);
            return vec![default_package_data(DatasourceId::MavenPom)];
        }
    };

    let sanitized_content = sanitize_template_directives(&content);
    let mut reader = Reader::from_str(sanitized_content.as_ref());
    reader.config_mut().trim_text(true);

    let mut state = PomParseState::new();
    let mut buf = Vec::new();
    let mut iteration_count: usize = 0;

    loop {
        iteration_count += 1;
        if iteration_count > MAX_ITERATION_COUNT {
            warn!(
                "Exceeded MAX_ITERATION_COUNT ({}) parsing pom.xml at {:?}; stopping early",
                MAX_ITERATION_COUNT, path
            );
            break;
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let element_name = e.name().as_ref().to_vec();
                state.handle_start(element_name);
            }
            Ok(Event::Text(e)) => {
                let text = match e.decode() {
                    Ok(Cow::Borrowed(s)) => s.to_string(),
                    Ok(Cow::Owned(s)) => s,
                    Err(_) => {
                        warn!(
                            "Invalid UTF-8 in XML text content in {:?}; using lossy conversion",
                            path
                        );
                        String::from_utf8_lossy(e.as_ref()).into_owned()
                    }
                };
                state.handle_text(path, text);
            }
            Ok(Event::Comment(e)) => {
                let comment = match e.decode() {
                    Ok(Cow::Borrowed(s)) => s.trim().to_string(),
                    Ok(Cow::Owned(s)) => s.trim().to_string(),
                    Err(_) => {
                        warn!(
                            "Invalid UTF-8 in XML comment in {:?}; using lossy conversion",
                            path
                        );
                        String::from_utf8_lossy(e.as_ref())
                            .into_owned()
                            .trim()
                            .to_string()
                    }
                };
                state.handle_comment(comment);
            }
            Ok(Event::End(e)) => state.handle_end(e.name().as_ref()),
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing pom.xml at {:?}: {}", path, e);
                return vec![state.into_package_data()];
            }
            _ => {}
        }

        buf.clear();
    }

    vec![state.finalize(path)]
}
