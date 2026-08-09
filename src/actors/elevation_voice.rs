//! Voice-guided elevation signals (ADR-110 extension).
//!
//! Conversations inside the immersive interface are the primary guide for
//! what deserves formalisation: concepts people *talk about* while exploring
//! the graph outrank concepts that are merely well-connected. This module is
//! the pure, testable half of that loop:
//!
//! - [`ConceptIndex`] — normalised n-gram lookup over the graph's frontier and
//!   page labels, rebuilt from each graph snapshot.
//! - [`harvest_mentions`] — extracts concept mentions from a transcription
//!   line.
//! - [`VoiceDemandLedger`] — decaying per-concept demand with utterance
//!   excerpts for case provenance (half-life 30 minutes: conversation guides
//!   *now*, not forever).
//! - [`parse_elevation_intent`] — explicit spoken commands ("elevate X",
//!   "formalise X") that jump the queue entirely.
//!
//! Transcriptions arrive from `SpeechService::subscribe_to_transcriptions`.
//! Today that stream is unattributed (a plain text line per utterance);
//! per-user/per-room attribution lands with the XR voice path (LiveKit) and
//! slots into [`VoiceDemandLedger::note`]'s `speaker` argument.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Demand decay half-life: a mention is worth half as much 30 minutes later.
const DEMAND_HALF_LIFE: Duration = Duration::from_secs(30 * 60);
/// Longest concept phrase (in words) the index will match.
const MAX_NGRAM: usize = 4;
/// How many utterance excerpts to keep per concept for case provenance.
const MAX_EXCERPTS: usize = 3;

/// Normalise text for matching: lowercase, alphanumeric words only.
fn normalise_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Normalised n-gram index over elevatable concept labels.
pub struct ConceptIndex {
    /// normalised phrase -> original label
    phrases: HashMap<String, String>,
}

impl ConceptIndex {
    /// Build from concept labels (frontier `owl_class` stubs and working
    /// pages). Single-word labels shorter than 4 characters are excluded —
    /// they false-positive on ordinary speech ("ai" matters, "it" does not,
    /// so the cut is length, not stop-words).
    pub fn build<'a, I: IntoIterator<Item = &'a str>>(labels: I) -> Self {
        let mut phrases = HashMap::new();
        for label in labels {
            let words = normalise_words(label);
            if words.is_empty() || words.len() > MAX_NGRAM {
                continue;
            }
            if words.len() == 1 && words[0].len() < 4 {
                continue;
            }
            phrases.insert(words.join(" "), label.to_string());
        }
        Self { phrases }
    }

    pub fn len(&self) -> usize {
        self.phrases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.phrases.is_empty()
    }

    /// Exact normalised lookup of a phrase.
    pub fn lookup(&self, phrase: &str) -> Option<&str> {
        self.phrases
            .get(&normalise_words(phrase).join(" "))
            .map(String::as_str)
    }
}

/// Extract concept mentions from one transcription line. Greedy longest-match
/// n-gram scan: each word position tries the longest phrase first so
/// "gaussian splatting" matches as one concept, not two.
pub fn harvest_mentions(transcript: &str, index: &ConceptIndex) -> Vec<String> {
    let words = normalise_words(transcript);
    let mut found = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let mut matched = 0;
        for n in (1..=MAX_NGRAM.min(words.len() - i)).rev() {
            let phrase = words[i..i + n].join(" ");
            if let Some(label) = index.phrases.get(&phrase) {
                if !found.contains(label) {
                    found.push(label.clone());
                }
                matched = n;
                break;
            }
        }
        i += matched.max(1);
    }
    found
}

/// One concept's accumulated conversational demand.
#[derive(Debug, Clone)]
pub struct VoiceDemand {
    pub mentions: u32,
    pub score: f32,
    pub last_seen: Instant,
    pub speakers: Vec<String>,
    pub excerpts: Vec<String>,
}

/// Decaying ledger of conversational demand per concept label.
#[derive(Default)]
pub struct VoiceDemandLedger {
    demands: HashMap<String, VoiceDemand>,
}

impl VoiceDemandLedger {
    pub fn new() -> Self {
        Self::default()
    }

    fn decay_factor(elapsed: Duration) -> f32 {
        0.5_f32.powf(elapsed.as_secs_f32() / DEMAND_HALF_LIFE.as_secs_f32())
    }

    /// Record a mention. `speaker` is the attributed identity when the voice
    /// path provides one (XR room member did), empty otherwise.
    pub fn note(&mut self, label: &str, excerpt: &str, speaker: &str, now: Instant) {
        let d = self
            .demands
            .entry(label.to_string())
            .or_insert(VoiceDemand {
                mentions: 0,
                score: 0.0,
                last_seen: now,
                speakers: Vec::new(),
                excerpts: Vec::new(),
            });
        // Decay the running score to `now`, then add this mention.
        d.score = d.score * Self::decay_factor(now.duration_since(d.last_seen)) + 1.0;
        d.mentions += 1;
        d.last_seen = now;
        if !speaker.is_empty() && !d.speakers.contains(&speaker.to_string()) {
            d.speakers.push(speaker.to_string());
        }
        if d.excerpts.len() >= MAX_EXCERPTS {
            d.excerpts.remove(0);
        }
        let mut e = excerpt.trim().to_string();
        if e.len() > 160 {
            e.truncate(160);
        }
        d.excerpts.push(e);
    }

    /// Current decayed score for a label (0.0 when never mentioned).
    pub fn score(&self, label: &str, now: Instant) -> f32 {
        self.demands
            .get(label)
            .map(|d| d.score * Self::decay_factor(now.duration_since(d.last_seen)))
            .unwrap_or(0.0)
    }

    pub fn demand(&self, label: &str) -> Option<&VoiceDemand> {
        self.demands.get(label)
    }

    /// Drop entries whose decayed score is negligible.
    pub fn prune(&mut self, now: Instant) {
        self.demands
            .retain(|_, d| d.score * Self::decay_factor(now.duration_since(d.last_seen)) > 0.05);
    }

    /// Labels currently carrying demand.
    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.demands.keys().map(String::as_str)
    }
}

/// Detect an explicit spoken elevation command and return the concept phrase.
/// Recognised forms (case-insensitive): "elevate <concept>",
/// "formalise/formalize <concept>", "promote <concept> to the ontology",
/// "make <concept> formal/canonical/a class". Trailing politeness/articles
/// are trimmed.
pub fn parse_elevation_intent(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let after = |needle: &str| -> Option<String> {
        let idx = lower.find(needle)?;
        let tail = &text[idx + needle.len()..];
        let cleaned = tail
            .trim()
            .trim_start_matches("the ")
            .trim_start_matches("a ")
            .trim_end_matches(|c: char| ".,!?".contains(c))
            .trim();
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned.to_string())
        }
    };

    if let Some(c) = after("elevate ") {
        // "elevate X to the ontology" → strip the destination clause
        return Some(
            c.split(" to the ontology")
                .next()
                .unwrap_or(&c)
                .trim()
                .to_string(),
        );
    }
    if let Some(c) = after("formalise ").or_else(|| after("formalize ")) {
        return Some(c);
    }
    if let Some(c) = after("promote ") {
        return Some(
            c.split(" to the ontology")
                .next()
                .unwrap_or(&c)
                .trim()
                .to_string(),
        );
    }
    if lower.contains("make ") {
        for suffix in [" formal", " canonical", " a class"] {
            if let Some(end) = lower.find(suffix) {
                if let Some(start) = lower.find("make ") {
                    if start + 5 < end {
                        let concept = text[start + 5..end].trim();
                        let concept = concept
                            .trim_start_matches("the ")
                            .trim_start_matches("a ")
                            .trim();
                        if !concept.is_empty() {
                            return Some(concept.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> ConceptIndex {
        ConceptIndex::build([
            "finality mechanism",
            "search space definition",
            "Gaussian Splatting",
            "AI", // too short for single-word matching — excluded
            "agentic mycelia",
        ])
    }

    #[test]
    fn index_excludes_short_single_words() {
        let idx = index();
        assert_eq!(idx.len(), 4);
        assert!(idx.lookup("ai").is_none());
        assert_eq!(idx.lookup("gaussian splatting"), Some("Gaussian Splatting"));
    }

    #[test]
    fn harvest_finds_multiword_concepts_in_speech() {
        let idx = index();
        let found = harvest_mentions(
            "I think the finality mechanism needs work, and gaussian splatting too.",
            &idx,
        );
        assert_eq!(
            found,
            vec![
                "finality mechanism".to_string(),
                "Gaussian Splatting".to_string()
            ]
        );
    }

    #[test]
    fn harvest_prefers_longest_match_and_dedupes() {
        let idx = ConceptIndex::build(["search space", "search space definition"]);
        let found = harvest_mentions(
            "the search space definition matters; the search space definition really does",
            &idx,
        );
        assert_eq!(found, vec!["search space definition".to_string()]);
    }

    #[test]
    fn ledger_decays_and_accumulates() {
        let mut ledger = VoiceDemandLedger::new();
        let t0 = Instant::now();
        ledger.note("finality mechanism", "we said it", "did:nostr:abc", t0);
        ledger.note("finality mechanism", "again", "", t0);
        assert!((ledger.score("finality mechanism", t0) - 2.0).abs() < 1e-3);
        // One half-life later the score halves.
        let t1 = t0 + DEMAND_HALF_LIFE;
        assert!((ledger.score("finality mechanism", t1) - 1.0).abs() < 1e-3);
        assert_eq!(ledger.score("never mentioned", t0), 0.0);
        let d = ledger.demand("finality mechanism").unwrap();
        assert_eq!(d.mentions, 2);
        assert_eq!(d.speakers, vec!["did:nostr:abc".to_string()]);
        assert_eq!(d.excerpts.len(), 2);
    }

    #[test]
    fn ledger_prunes_stale_entries() {
        let mut ledger = VoiceDemandLedger::new();
        let t0 = Instant::now();
        ledger.note("old concept", "x", "", t0);
        ledger.prune(t0 + DEMAND_HALF_LIFE * 10);
        assert!(ledger.demand("old concept").is_none());
    }

    #[test]
    fn explicit_intents_parse() {
        assert_eq!(
            parse_elevation_intent("Please elevate finality mechanism to the ontology"),
            Some("finality mechanism".to_string())
        );
        assert_eq!(
            parse_elevation_intent("let's formalise gaussian splatting."),
            Some("gaussian splatting".to_string())
        );
        assert_eq!(
            parse_elevation_intent("can we make the search space definition a class"),
            Some("search space definition".to_string())
        );
        assert_eq!(parse_elevation_intent("nothing to see here"), None);
        assert_eq!(parse_elevation_intent("elevate "), None);
    }
}
