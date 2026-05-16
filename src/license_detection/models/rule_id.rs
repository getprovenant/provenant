// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Newtype wrapper for rule IDs used throughout license detection.

use rkyv::Archive;

/// Internal rule ID used as an index into `LicenseIndex::rules_by_rid`.
///
/// Every rule in the license index is assigned a `RuleId` sequentially during
/// index construction. The ID is used for fast Vec lookups and as a key in
/// HashMap/HashSet structures on `LicenseIndex`.
///
/// # Sentinel value
///
/// `RuleId::NONE` (`usize::MAX`) represents an absent or invalid rule ID.
/// It is used where production code previously relied on `rid: 0` as a sentinel,
/// which was unsafe because `0` is a valid index pointing to a real rule.
/// Use `is_valid()` to check whether a `RuleId` refers to a real rule.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(Hash, Eq, PartialEq, PartialOrd, Ord))]
pub struct RuleId(usize);

impl RuleId {
    /// Sentinel value representing an absent or invalid rule ID.
    ///
    /// `usize::MAX` can never be a valid Vec index, so this is guaranteed
    /// not to alias any real rule.
    pub const NONE: Self = Self(usize::MAX);

    /// Creates a `RuleId` from a raw `usize` index.
    ///
    /// # Panics
    ///
    /// Panics if `raw` is `usize::MAX` (the sentinel value).
    /// Use `RuleId::NONE` to construct the sentinel.
    pub const fn new(raw: usize) -> Self {
        assert!(
            raw != usize::MAX,
            "RuleId::new(usize::MAX) is reserved for RuleId::NONE"
        );
        Self(raw)
    }

    /// Returns the raw `usize` index.
    ///
    /// Prefer using `RuleId` directly for indexing and lookups rather than
    /// extracting the raw value.
    pub const fn raw(self) -> usize {
        self.0
    }

    /// Returns `true` if this `RuleId` refers to a real rule (not the sentinel).
    pub const fn is_valid(self) -> bool {
        self.0 != usize::MAX
    }

    /// Returns `true` if this is the `RuleId::NONE` sentinel.
    pub const fn is_none(self) -> bool {
        self.0 == usize::MAX
    }
}

impl From<RuleId> for usize {
    fn from(value: RuleId) -> Self {
        value.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_none() {
            write!(f, "RuleId::NONE")
        } else {
            write!(f, "RuleId({})", self.0)
        }
    }
}
