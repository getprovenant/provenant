// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use smallvec::SmallVec;
use std::cmp::Ordering;
use std::ops::Deref;

use rkyv::Archive;

use crate::license_detection::index::dictionary::{TokenDictionary, TokenId, TokenKind};

/// A set of token IDs stored as a sorted SmallVec.
///
/// Invariant: elements are always sorted and deduplicated.
/// Construct via `TokenSet::from_token_ids()`, `TokenSet::from_u16_iter()`,
/// or `.collect()` from an iterator of u16.
#[derive(Clone, Debug, PartialEq, Eq, Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct TokenSet(SmallVec<[u16; 64]>);

impl TokenSet {
    /// Create a TokenSet from an iterator of u16 token IDs.
    /// Sorts and deduplicates the input.
    pub fn from_u16_iter<I: IntoIterator<Item = u16>>(iter: I) -> Self {
        let mut inner: SmallVec<[u16; 64]> = iter.into_iter().collect();
        inner.sort_unstable();
        inner.dedup();
        Self(inner)
    }

    /// Create a TokenSet from an iterator of TokenId values.
    pub fn from_token_ids<I: IntoIterator<Item = TokenId>>(iter: I) -> Self {
        Self::from_u16_iter(iter.into_iter().map(|tid| tid.raw()))
    }

    /// Create an empty TokenSet.
    pub fn new() -> Self {
        Self(SmallVec::new())
    }

    /// Number of tokens in the set.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is the set empty?
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return true if the set contains the given token ID.
    pub fn contains_token_id(&self, token_id: TokenId) -> bool {
        self.0.contains(&token_id.raw())
    }

    /// Get the subset containing only high-value (legalese) tokens.
    pub fn high_subset(&self, dictionary: &TokenDictionary) -> Self {
        Self::from_u16_iter(
            self.iter()
                .filter(|&tid| dictionary.token_kind(TokenId::new(tid)) == TokenKind::Legalese),
        )
    }

    /// Count intersection with another TokenSet (no allocation).
    pub fn intersection_count(&self, other: &TokenSet) -> usize {
        let (mut i, mut j, mut count) = (0, 0, 0);
        while i < self.0.len() && j < other.0.len() {
            match self.0[i].cmp(&other.0[j]) {
                Ordering::Less => i += 1,
                Ordering::Greater => j += 1,
                Ordering::Equal => {
                    count += 1;
                    i += 1;
                    j += 1;
                }
            }
        }
        count
    }

    /// Materialize intersection with another TokenSet.
    pub fn intersection(&self, other: &TokenSet) -> TokenSet {
        let mut result = SmallVec::new();
        let (mut i, mut j) = (0, 0);
        while i < self.0.len() && j < other.0.len() {
            match self.0[i].cmp(&other.0[j]) {
                Ordering::Less => i += 1,
                Ordering::Greater => j += 1,
                Ordering::Equal => {
                    result.push(self.0[i]);
                    i += 1;
                    j += 1;
                }
            }
        }
        Self(result)
    }

    /// Iterate over the sorted token IDs.
    pub fn iter(&self) -> impl Iterator<Item = u16> + '_ {
        self.0.iter().copied()
    }
}

impl Deref for TokenSet {
    type Target = [u16];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Fixed-width bitset over high-value (legalese) token IDs.
///
/// Legalese tokens occupy the reserved low ID range `0..len_legalese`, so a
/// rule's (or query's) high-token set maps onto one bit per ID. Intersection
/// count is then a word-wise `AND` + `popcount`, which replaces the sorted
/// two-pointer merge walk in the candidate-selection high-token gate — a
/// branch-free, allocation-free, constant-per-rule cost regardless of set size.
///
/// All bitsets compared together MUST share the same width (`len_legalese`).
#[derive(Clone, Debug, PartialEq, Eq, Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct HighBitset(Box<[u64]>);

impl HighBitset {
    /// Build a bitset of `len_legalese` bits with the high tokens of `set` set.
    ///
    /// `set` is expected to contain only high token IDs (`< len_legalese`); any
    /// out-of-range ID would be a builder invariant violation, so it is ignored
    /// rather than panicking in the hot path.
    pub fn from_token_set(set: &TokenSet, len_legalese: usize) -> Self {
        let mut words = vec![0u64; len_legalese.div_ceil(64)].into_boxed_slice();
        for tid in set.iter() {
            let bit = tid as usize;
            if let Some(word) = words.get_mut(bit / 64) {
                *word |= 1u64 << (bit % 64);
            }
        }
        Self(words)
    }

    /// Count of shared bits with `other`. Both bitsets must share a width
    /// (built from the same `len_legalese`); `zip` would otherwise silently
    /// truncate to the shorter operand and undercount.
    #[inline]
    pub fn intersection_count(&self, other: &HighBitset) -> usize {
        debug_assert_eq!(
            self.0.len(),
            other.0.len(),
            "HighBitset width mismatch: {} vs {} words",
            self.0.len(),
            other.0.len()
        );
        self.0
            .iter()
            .zip(other.0.iter())
            .map(|(a, b)| (a & b).count_ones() as usize)
            .sum()
    }
}

impl Default for TokenSet {
    fn default() -> Self {
        Self::new()
    }
}

impl std::iter::FromIterator<u16> for TokenSet {
    fn from_iter<T: IntoIterator<Item = u16>>(iter: T) -> Self {
        Self::from_u16_iter(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_detection::index::dictionary::tid;

    #[test]
    fn test_from_token_ids() {
        let set = TokenSet::from_token_ids([tid(4), tid(2), tid(4), tid(1)]);

        assert_eq!(set.iter().collect::<Vec<_>>(), vec![1, 2, 4]);
        assert!(set.contains_token_id(tid(1)));
        assert!(set.contains_token_id(tid(2)));
        assert!(set.contains_token_id(tid(4)));
    }

    #[test]
    fn test_high_subset() {
        let set = TokenSet::from_u16_iter([1, 2, 5, 10]);
        let dict = TokenDictionary::new_with_legalese_pairs(&[("one", 1), ("two", 2)]);

        let high_set = set.high_subset(&dict);

        assert_eq!(high_set.iter().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn high_bitset_intersection_count_overlap_cases() {
        let a = TokenSet::from_u16_iter([1, 5, 9, 63, 64, 200]);
        let b = TokenSet::from_u16_iter([5, 9, 64, 201]);
        // width 256 is not a multiple of 64's underlying word count boundary
        // checks; use a width that leaves a partial trailing word (200 -> 4 words).
        let width = 256;
        let ba = HighBitset::from_token_set(&a, width);
        let bb = HighBitset::from_token_set(&b, width);
        // partial overlap: {5, 9, 64}
        assert_eq!(ba.intersection_count(&bb), 3);
        // full self-overlap
        assert_eq!(ba.intersection_count(&ba), a.len());
        // zero overlap
        let c = HighBitset::from_token_set(&TokenSet::from_u16_iter([2, 3, 4]), width);
        let d = HighBitset::from_token_set(&TokenSet::from_u16_iter([10, 11]), width);
        assert_eq!(c.intersection_count(&d), 0);
        // matches the TokenSet reference on the shared subset
        assert_eq!(ba.intersection_count(&bb), a.intersection_count(&b));
    }

    #[test]
    fn high_bitset_handles_non_multiple_of_64_width_and_boundary_bits() {
        // width 65 -> 2 words; exercises a bit in the trailing partial word (64).
        let width = 65;
        let a = HighBitset::from_token_set(&TokenSet::from_u16_iter([0, 63, 64]), width);
        let b = HighBitset::from_token_set(&TokenSet::from_u16_iter([63, 64]), width);
        assert_eq!(a.intersection_count(&b), 2);
    }

    #[test]
    fn high_bitset_skips_ids_beyond_allocated_words() {
        // Width rounds up to whole u64 words, so `from_token_set` silently skips
        // only IDs at/beyond words*64. In practice high token IDs are always
        // < len_legalese, so this guard is defensive; here width 8 -> 1 word
        // (64 bits), and id 100 (>= 64) is dropped from both sets.
        let width = 8;
        let a = HighBitset::from_token_set(&TokenSet::from_u16_iter([1, 7, 100]), width);
        let b = HighBitset::from_token_set(&TokenSet::from_u16_iter([7, 100]), width);
        // id 100 is skipped in both; only id 7 is shared and in range.
        assert_eq!(a.intersection_count(&b), 1);
    }
}
