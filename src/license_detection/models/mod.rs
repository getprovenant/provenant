// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Core data structures for license detection.

pub mod license;
pub mod license_match;
pub mod loaded_license;
pub mod loaded_rule;
pub mod position_span;
pub mod rule;
pub mod rule_id;

pub use license::License;
pub use license_match::{LicenseMatch, MatchCoordinates, MatcherKind};
pub use loaded_license::LoadedLicense;
pub use loaded_rule::LoadedRule;
pub use position_span::PositionSpan;
pub use rule::{Rule, RuleKind};
pub use rule_id::RuleId;

#[cfg(test)]
mod mod_tests;
