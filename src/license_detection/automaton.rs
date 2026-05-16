// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

//! Aho-Corasick automaton wrapper using daachorse.
//!
//! This module provides a `DoubleArrayAhoCorasick`-based automaton that is
//! significantly smaller than the aho-corasick crate's implementation.
//! The daachorse library provides ~85% smaller binary size and built-in
//! serialization support.

use daachorse::DoubleArrayAhoCorasick;
use rancor::Fallible;
use rkyv::with::{ArchiveWith, DeserializeWith, SerializeWith};
use rkyv::{Archive, Deserialize, Place, Serialize};

/// rkyv `with` adapter that archives an `Automaton` as its serialized byte form.
pub struct AsBytes;

impl ArchiveWith<Automaton> for AsBytes {
    type Archived = <Vec<u8> as Archive>::Archived;
    type Resolver = <Vec<u8> as Archive>::Resolver;

    fn resolve_with(field: &Automaton, resolver: Self::Resolver, out: Place<Self::Archived>) {
        field.serialize_bytes().resolve(resolver, out);
    }
}

impl<S: Fallible + rkyv::ser::Writer + rkyv::ser::Allocator + ?Sized> SerializeWith<Automaton, S>
    for AsBytes
{
    fn serialize_with(field: &Automaton, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        field.serialize_bytes().serialize(serializer)
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<<Vec<u8> as Archive>::Archived, Automaton, D> for AsBytes
where
    <Vec<u8> as Archive>::Archived: Deserialize<Vec<u8>, D>,
{
    fn deserialize_with(
        field: &<Vec<u8> as Archive>::Archived,
        deserializer: &mut D,
    ) -> Result<Automaton, D::Error> {
        let bytes: Vec<u8> = field.deserialize(deserializer)?;
        Ok(Automaton::deserialize_unchecked(&bytes))
    }
}

/// A match found by the automaton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Pattern ID (index into the original pattern list).
    pub pattern: usize,
    /// Start position in haystack (bytes, inclusive).
    pub start: usize,
    /// End position in haystack (bytes, exclusive).
    pub end: usize,
}

/// Aho-Corasick automaton using daachorse's double-array implementation.
///
/// This wrapper provides the same interface as the previous FrozenNfa
/// but with significantly smaller memory footprint and serialization support.
pub struct Automaton {
    inner: DoubleArrayAhoCorasick<u32>,
}

impl std::fmt::Debug for Automaton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Automaton")
            .field("num_states", &self.inner.num_states())
            .field("heap_bytes", &self.inner.heap_bytes())
            .finish()
    }
}

impl Clone for Automaton {
    fn clone(&self) -> Self {
        let bytes = self.inner.serialize();
        Self::deserialize_unchecked(&bytes)
    }
}

impl Automaton {
    /// Create a new empty automaton.
    ///
    /// Since daachorse requires at least one non-empty pattern, we use a
    /// dummy pattern that will never match in practice (a unique byte sequence).
    pub fn empty() -> Self {
        // Use a very unlikely byte sequence as a sentinel pattern
        // This will match but never in our token-encoded data
        let dummy_pattern: &[u8] = &[0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA, 0xF9, 0xF8];
        match DoubleArrayAhoCorasick::new([dummy_pattern]) {
            Ok(ac) => Self { inner: ac },
            Err(_) => panic!("Failed to create empty automaton"),
        }
    }

    /// Find all overlapping matches in the haystack.
    ///
    /// Returns an iterator that yields all matches found in the haystack,
    /// including overlapping matches. The matches are yielded in order of
    /// their end position.
    ///
    /// **Important**: This filters matches to only those starting at even
    /// byte positions (token boundaries). Each token is encoded as 2 bytes,
    /// so matches starting at odd byte positions would span token boundaries.
    pub fn find_overlapping_iter(&self, haystack: &[u8]) -> FindOverlappingIter {
        FindOverlappingIter::new(&self.inner, haystack)
    }

    /// Deserialize an automaton from bytes.
    ///
    /// # Safety
    /// The bytes must be valid serialized data from the underlying daachorse automaton.
    pub fn deserialize_unchecked(bytes: &[u8]) -> Self {
        let (ac, _) = unsafe { DoubleArrayAhoCorasick::deserialize_unchecked(bytes) };
        Self { inner: ac }
    }

    /// Get the number of states in the automaton.
    pub fn num_states(&self) -> usize {
        self.inner.num_states()
    }

    /// Get the memory usage in bytes.
    pub fn heap_bytes(&self) -> usize {
        self.inner.heap_bytes()
    }

    /// Serialize the automaton to a byte vector.
    pub fn serialize_bytes(&self) -> Vec<u8> {
        self.inner.serialize()
    }
}

impl Default for Automaton {
    fn default() -> Self {
        Self::empty()
    }
}

/// Iterator over all overlapping matches in a haystack.
///
/// This iterator finds all matches, including those that overlap, by
/// continuing to search after each match rather than skipping past it.
///
/// **Token Boundary Filtering**: This iterator only yields matches that
/// start at even byte positions. Since each token is encoded as 2 bytes,
/// matches at odd positions would incorrectly span token boundaries.
pub struct FindOverlappingIter {
    inner: std::vec::IntoIter<daachorse::Match<u32>>,
}

impl FindOverlappingIter {
    fn new(automaton: &DoubleArrayAhoCorasick<u32>, haystack: &[u8]) -> Self {
        let matches: Vec<_> = automaton.find_overlapping_iter(haystack).collect();
        Self {
            inner: matches.into_iter(),
        }
    }
}

impl Iterator for FindOverlappingIter {
    type Item = Match;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let m = self.inner.next()?;
            // Token boundary check: each token is 2 bytes, so matches must
            // start at even byte positions. Odd positions would span tokens.
            if m.start() % 2 == 0 {
                return Some(Match {
                    pattern: m.value() as usize,
                    start: m.start(),
                    end: m.end(),
                });
            }
            // Skip matches at odd byte positions (invalid token boundaries)
        }
    }
}

/// Builder for constructing automatons incrementally.
///
/// This mirrors the `FrozenNfaBuilder` interface for compatibility.
pub struct AutomatonBuilder {
    patterns: Vec<Vec<u8>>,
    values: Vec<u32>,
}

impl AutomatonBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Add a pattern to the automaton with an associated value.
    ///
    /// Empty patterns are skipped. Daachorse 3.0+ supports duplicate patterns;
    /// each occurrence gets its own value.
    pub fn add_pattern_with_value(&mut self, pattern: &[u8], value: u32) {
        if !pattern.is_empty() {
            self.patterns.push(pattern.to_vec());
            self.values.push(value);
        }
    }

    /// Add a pattern to the automaton.
    ///
    /// Empty patterns are skipped. Assigns sequential IDs (0, 1, 2, ...).
    pub fn add_pattern(&mut self, pattern: &[u8]) {
        let value = self.patterns.len() as u32;
        self.add_pattern_with_value(pattern, value);
    }

    /// Build the automaton.
    ///
    /// Uses `with_values()` so each pattern's value is directly accessible
    /// via `Match::value()`, eliminating the need for an external pattern_id-to-rid mapping.
    pub fn build(self) -> Automaton {
        if self.patterns.is_empty() {
            return Automaton::empty();
        }

        let patvals: Vec<(&[u8], u32)> = self
            .patterns
            .iter()
            .zip(self.values.iter())
            .map(|(p, &v)| (p.as_slice(), v))
            .collect();

        match DoubleArrayAhoCorasick::with_values(patvals) {
            Ok(ac) => Automaton { inner: ac },
            Err(_) => Automaton::empty(),
        }
    }
}

impl Default for AutomatonBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_boundary_filtering() {
        let pattern: &[u8] = &[31, 49];
        let mut builder = AutomatonBuilder::new();
        builder.add_pattern(pattern);
        let ac = builder.build();

        // The pattern [31, 49] appears at bytes 1-2 (odd position)
        // which would span token boundaries - should NOT match
        let haystack: &[u8] = &[109, 31, 49, 74];
        let matches: Vec<_> = ac.find_overlapping_iter(haystack).collect();
        assert!(
            matches.is_empty(),
            "Should not match across token boundaries"
        );
    }

    #[test]
    fn test_valid_token_match() {
        let pattern: &[u8] = &[31, 49];
        let mut builder = AutomatonBuilder::new();
        builder.add_pattern(pattern);
        let ac = builder.build();

        let haystack: &[u8] = &[0, 0, 31, 49, 0, 0];
        let matches: Vec<_> = ac.find_overlapping_iter(haystack).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start, 2);
        assert_eq!(matches[0].end, 4);
    }

    #[test]
    fn test_builder_skips_empty_patterns() {
        let mut builder = AutomatonBuilder::new();
        builder.add_pattern(b"");
        builder.add_pattern(b"hello");
        builder.add_pattern(b"");
        let ac = builder.build();

        let matches: Vec<_> = ac.find_overlapping_iter(b"hello").collect();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_builder_with_values() {
        let mut builder = AutomatonBuilder::new();
        builder.add_pattern_with_value(b"hello", 42);
        builder.add_pattern_with_value(b"world", 99);
        let ac = builder.build();

        let matches: Vec<_> = ac.find_overlapping_iter(b"hello world").collect();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].pattern, 42);
        assert_eq!(matches[1].pattern, 99);
    }

    #[test]
    fn test_builder_duplicate_patterns() {
        let mut builder = AutomatonBuilder::new();
        builder.add_pattern_with_value(b"hello", 10);
        builder.add_pattern_with_value(b"hello", 20);
        let ac = builder.build();

        let matches: Vec<_> = ac.find_overlapping_iter(b"hello").collect();
        assert_eq!(matches.len(), 2);
        let mut values: Vec<usize> = matches.iter().map(|m| m.pattern).collect();
        values.sort();
        assert_eq!(values, vec![10, 20]);
    }
}
