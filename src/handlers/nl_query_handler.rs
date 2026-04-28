//! Per-descriptor NL-query handler — the new transport behind PRD-007's
//! Spine "describe in your own words" affordance.
//!
//! Endpoints (per ADR-061 §LLM call envelope):
//!   POST /api/nl-query/translate   — intent → {action, path, value, explanation}
//!   POST /api/nl-query/explain     — descriptor → plain-language explanation
//!   POST /api/nl-query/validate    — dry-run a mutation; would server accept?
//!   POST /api/nl-query/examples    — example utterances per descriptor
//!
//! v1 backend is a deterministic rule engine with high-coverage heuristics
//! tuned for the 49 descriptors in `client/src/features/control-surface/descriptors`.
//! It preserves the option to swap to an LLM backend (Ollama / OpenAI) later
//! without changing the wire contract — that's the whole point of ADR-061.

use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize)]
pub struct DescriptorContext {
    pub id: String,
    pub label: String,
    pub path: Vec<String>,
    pub tier: u8,
    pub category: String,
    pub current_value: Value,
    #[serde(default)]
    pub bounds: Option<Bounds>,
    #[serde(default)]
    pub examples: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bounds {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TranslateRequest {
    pub intent: String,
    pub descriptor: DescriptorContext,
    #[serde(default)]
    pub session_pubkey: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TranslateResponse {
    pub action: &'static str, // "set" | "noop" | "denied"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_after: Option<String>,
    pub explanation: String,
    pub confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

fn lower(s: &str) -> String {
    s.to_lowercase()
}

fn intent_contains(intent: &str, needles: &[&str]) -> bool {
    let i = lower(intent);
    needles.iter().any(|n| i.contains(&lower(n)))
}

/// Translate intent + descriptor → action.
///
/// v1 backend dispatches by descriptor id and applies category-aware
/// heuristics. Confidence is heuristic — 0.95 for direct keyword hits,
/// 0.7 for partial pattern, 0.3 when we fall back to a midpoint nudge.
pub async fn translate(
    web::Json(req): web::Json<TranslateRequest>,
) -> Result<HttpResponse> {
    let intent = lower(&req.intent);
    let d = &req.descriptor;

    // Quick deny: empty intent.
    if intent.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(TranslateResponse {
            action: "denied",
            path: None,
            value: None,
            summary_after: None,
            explanation: "Empty intent — please describe what you'd like to change.".into(),
            confidence: 0.0,
            reason: Some("empty_intent".into()),
        }));
    }

    // Boolean-shaped descriptors.
    if d.current_value.is_boolean() {
        let truthy = intent_contains(
            &intent,
            &[
                "on", "enable", "show", "turn on", "switch on", "yes", "active",
                "running", "true", "use",
            ],
        );
        let falsy = intent_contains(
            &intent,
            &[
                "off", "disable", "hide", "turn off", "switch off", "no", "inactive",
                "pause", "stop", "false", "don't",
            ],
        );
        if truthy ^ falsy {
            let new = truthy && !falsy;
            return Ok(HttpResponse::Ok().json(TranslateResponse {
                action: "set",
                path: Some(d.path.clone()),
                value: Some(Value::Bool(new)),
                summary_after: Some(format!("{}: {}", d.label, if new { "on" } else { "off" })),
                explanation: format!("Set {} to {}.", d.label, if new { "on" } else { "off" }),
                confidence: 0.92,
                reason: None,
            }));
        }
    }

    // Number-shaped descriptors with bounds.
    if d.current_value.is_number() {
        if let Some(b) = &d.bounds {
            let min = b.min.unwrap_or(0.0);
            let max = b.max.unwrap_or(min + 1.0);
            let cur = d.current_value.as_f64().unwrap_or(min);
            let span = (max - min).abs().max(1e-6);
            let mut new: Option<f64> = None;
            let mut explain = String::new();
            let mut conf: f32 = 0.85;

            if intent_contains(
                &intent,
                &["max", "highest", "ceiling", "as much as possible", "very high"],
            ) {
                new = Some(max);
                explain = format!("Set {} to its maximum ({}).", d.label, max);
            } else if intent_contains(&intent, &["min", "lowest", "off", "zero", "very low"]) {
                new = Some(min);
                explain = format!("Set {} to its minimum ({}).", d.label, min);
            } else if intent_contains(
                &intent,
                &["mid", "middle", "default", "standard", "normal", "reset"],
            ) {
                new = Some(min + span * 0.5);
                explain = format!("Set {} to its standard value.", d.label);
            } else if intent_contains(
                &intent,
                &["bigger", "larger", "more", "higher", "tighter", "stronger", "increase"],
            ) {
                let next = (cur + span * 0.2).min(max);
                new = Some(next);
                explain = format!("Increased {} from {} to {}.", d.label, cur, next);
            } else if intent_contains(
                &intent,
                &["smaller", "less", "lower", "looser", "weaker", "decrease", "subtle"],
            ) {
                let next = (cur - span * 0.2).max(min);
                new = Some(next);
                explain = format!("Decreased {} from {} to {}.", d.label, cur, next);
            } else if intent_contains(&intent, &["double"]) {
                let next = (cur * 2.0).min(max);
                new = Some(next);
                explain = format!("Doubled {} (capped at max).", d.label);
            } else if intent_contains(&intent, &["halve", "half"]) {
                let next = (cur / 2.0).max(min);
                new = Some(next);
                explain = format!("Halved {}.", d.label);
            }

            // Try parsing a numeric literal from intent.
            if new.is_none() {
                let mut acc = String::new();
                let mut found_dot = false;
                for c in intent.chars() {
                    if c.is_ascii_digit() {
                        acc.push(c);
                    } else if c == '.' && !found_dot {
                        acc.push(c);
                        found_dot = true;
                    } else if !acc.is_empty() {
                        break;
                    }
                }
                if let Ok(parsed) = acc.parse::<f64>() {
                    let clamped = parsed.clamp(min, max);
                    new = Some(clamped);
                    explain = format!("Set {} to {}.", d.label, clamped);
                    conf = 0.78;
                }
            }

            if let Some(v) = new {
                let snapped = if let Some(step) = b.step {
                    if step > 0.0 {
                        (v / step).round() * step
                    } else {
                        v
                    }
                } else {
                    v
                };
                let val = Value::from(snapped);
                return Ok(HttpResponse::Ok().json(TranslateResponse {
                    action: "set",
                    path: Some(d.path.clone()),
                    value: Some(val),
                    summary_after: Some(format!("{}: {}", d.label, snapped)),
                    explanation: explain,
                    confidence: conf,
                    reason: None,
                }));
            }
        }
    }

    // String-shaped descriptors. Match against descriptor's declared examples first.
    if d.current_value.is_string() {
        if let Some(examples) = &d.examples {
            for ex in examples {
                if intent.contains(&lower(ex)) {
                    return Ok(HttpResponse::Ok().json(TranslateResponse {
                        action: "set",
                        path: Some(d.path.clone()),
                        value: Some(Value::String(ex.clone())),
                        summary_after: Some(format!("{}: {}", d.label, ex)),
                        explanation: format!("Set {} to “{}”.", d.label, ex),
                        confidence: 0.8,
                        reason: None,
                    }));
                }
            }
        }
    }

    // Object-shaped descriptors (MERGE parents). Recognise preset names heuristically.
    if d.current_value.is_object() {
        for preset in &["lite", "standard", "high", "off", "soft", "hard", "default"] {
            if intent.contains(preset) {
                return Ok(HttpResponse::Ok().json(TranslateResponse {
                    action: "noop",
                    path: Some(d.path.clone()),
                    value: None,
                    summary_after: None,
                    explanation: format!(
                        "Recognised preset “{}”. Use the editor's preset chips to apply.",
                        preset
                    ),
                    confidence: 0.5,
                    reason: None,
                }));
            }
        }
    }

    // Fall-through: cannot translate.
    Ok(HttpResponse::Ok().json(TranslateResponse {
        action: "noop",
        path: None,
        value: None,
        summary_after: None,
        explanation: format!(
            "Couldn't translate “{}” for {} — try one of: {}.",
            req.intent,
            d.label,
            d.examples
                .as_ref()
                .map(|x| x.join(", "))
                .unwrap_or_else(|| "see examples".into())
        ),
        confidence: 0.1,
        reason: Some("no_match".into()),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ExplainRequest {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub path: Option<Vec<String>>,
    #[serde(default)]
    pub current_value: Value,
    #[serde(default)]
    pub explain_prompt: Option<String>,
}

pub async fn explain(web::Json(req): web::Json<ExplainRequest>) -> Result<HttpResponse> {
    if let Some(custom) = &req.explain_prompt {
        return Ok(HttpResponse::Ok().json(json!({ "explanation": custom })));
    }
    let path_str = req
        .path
        .as_ref()
        .map(|p| p.join("."))
        .unwrap_or_else(|| req.id.clone());
    let explanation = format!(
        "{} (path: {}). Current value: {}. Use the row's editor to adjust, or describe a change in your own words.",
        req.label, path_str, req.current_value
    );
    Ok(HttpResponse::Ok().json(json!({ "explanation": explanation })))
}

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub path: Vec<String>,
    pub value: Value,
}

pub async fn validate(web::Json(req): web::Json<ValidateRequest>) -> Result<HttpResponse> {
    // v1: shape-only validation. Mutations still go through /api/settings/*
    // which is the security boundary.
    let ok = !req.path.is_empty() && !req.value.is_null();
    Ok(HttpResponse::Ok().json(json!({
        "ok": ok,
        "reason": if ok { "shape_ok" } else { "empty_path_or_value" }
    })))
}

#[derive(Debug, Deserialize)]
pub struct ExamplesRequest {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
}

pub async fn examples(web::Json(req): web::Json<ExamplesRequest>) -> Result<HttpResponse> {
    // Heuristic example pool by descriptor id substring.
    let id = req.id.to_lowercase();
    let examples: Vec<&'static str> = if id.contains("color") {
        vec!["white", "blue", "warm tone", "high contrast"]
    } else if id.contains("size") || id.contains("scale") {
        vec!["bigger", "smaller", "default", "tiny"]
    } else if id.contains("opacity") || id.contains("alpha") {
        vec!["fully solid", "ghosted", "half transparent"]
    } else if id.contains("enable") || id.contains("visible") {
        vec!["on", "off", "show", "hide"]
    } else if id.contains("quality") || id.contains("preset") {
        vec!["lite", "standard", "high"]
    } else if id.contains("damp") || id.contains("spring") {
        vec!["less bouncy", "more responsive", "tighter"]
    } else {
        vec!["default", "more", "less"]
    };
    Ok(HttpResponse::Ok().json(json!({ "examples": examples })))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    // Mounted under /api/spine-nl/ so we don't collide with the existing
    // graph-Cypher /api/nl-query/* family in natural_language_query_handler.
    cfg.service(
        web::scope("/spine-nl")
            .route("/translate", web::post().to(translate))
            .route("/explain", web::post().to(explain))
            .route("/validate", web::post().to(validate))
            .route("/examples", web::post().to(examples)),
    );
}
