// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::models::Party;
use crate::parsers::maven::pom::tags::KnownTag;

impl Party {
    pub(super) fn apply_text(&mut self, current: Option<KnownTag>, text: &str) {
        match current {
            Some(KnownTag::Name) => self.name = Some(text.to_string()),
            Some(KnownTag::Email) => self.email = Some(text.to_string()),
            Some(KnownTag::Url) => self.url = Some(text.to_string()),
            Some(KnownTag::Organization) => self.organization = Some(text.to_string()),
            Some(KnownTag::OrganizationUrl) => self.organization_url = Some(text.to_string()),
            Some(KnownTag::Timezone) => self.timezone = Some(text.to_string()),
            _ => {}
        }
    }
}
