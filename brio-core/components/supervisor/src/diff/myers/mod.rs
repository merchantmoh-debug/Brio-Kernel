//! Myers diff algorithm implementation.
//!
//! Myers' algorithm is a classic diff algorithm with O(ND) time complexity,
//! where N is the sum of the lengths of the two sequences and D is the number
//! of differences. It's particularly efficient when the two texts are similar.
//!
//! The algorithm uses a greedy approach to find the shortest edit script (SES)
//! that transforms one sequence into another.

pub mod algorithm;
pub mod optimization;

// Re-export main types
pub use algorithm::MyersDiff;
