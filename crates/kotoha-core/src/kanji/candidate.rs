//! Value types for kanji conversion results and options.
//!
//! Spec: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §5.1, §5.2.

/// A single kanji conversion candidate.
///
/// Phase 2+ may add fields (for example confidence sources or tokenization metadata),
/// so this type is marked `#[non_exhaustive]` per ADR 0006.
///
/// # Invariants
///
/// - `surface` is a UTF-8 string, typically containing kanji, hiragana, and/or katakana.
/// - `score` is the aggregated log-probability of the candidate; larger is better.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Kanji-mixed surface form (UTF-8).
    pub surface: String,
    /// Candidate score (aggregated log-probability; larger is better).
    pub score: f32,
}

impl Candidate {
    /// Constructs a new [`Candidate`].
    ///
    /// # Postconditions
    ///
    /// - The returned value's `surface` equals `surface.into()`.
    /// - The returned value's `score` equals the supplied `score` bit-for-bit.
    pub fn new(surface: impl Into<String>, score: f32) -> Self {
        Self {
            surface: surface.into(),
            score,
        }
    }
}

/// Tunable parameters for a single kanji conversion call.
///
/// Phase 2+ may add fields (for example sampling strategy or penalty knobs),
/// so this type is marked `#[non_exhaustive]` per ADR 0006.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertOptions {
    /// Maximum number of candidates to return. `0` yields an empty `Vec`.
    pub top_k: usize,
    /// Sampling temperature. `0.0` means greedy decoding.
    pub temperature: f32,
    /// Sampling seed. `Some(s)` pins deterministic output; `None` is non-deterministic.
    pub seed: Option<u64>,
}

impl Default for ConvertOptions {
    /// Default options: `top_k = 5`, `temperature = 0.0`, `seed = Some(0)`.
    ///
    /// Combined defaults yield deterministic greedy decoding, which is the
    /// baseline Layer 3 smoke test and E2E smoke test use.
    fn default() -> Self {
        Self {
            top_k: 5,
            temperature: 0.0,
            seed: Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_new_constructs_with_surface_and_score() {
        let c = Candidate::new("日本語", 0.9);
        assert_eq!(c.surface, "日本語");
        assert!((c.score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn candidate_is_clone_and_eq() {
        let a = Candidate::new("漢字", 0.5);
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn convert_options_default_is_top_k_5_temp_0_seed_0() {
        let opt = ConvertOptions::default();
        assert_eq!(opt.top_k, 5);
        assert!((opt.temperature - 0.0).abs() < 1e-6);
        assert_eq!(opt.seed, Some(0));
    }

    #[test]
    fn convert_options_is_clone_and_eq() {
        let a = ConvertOptions::default();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn convert_options_custom_fields_round_trip() {
        let opt = ConvertOptions {
            top_k: 10,
            temperature: 0.7,
            seed: None,
        };
        assert_eq!(opt.top_k, 10);
        assert!((opt.temperature - 0.7).abs() < 1e-6);
        assert!(opt.seed.is_none());
    }
}
