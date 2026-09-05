/// Image Generation Handler
/// Submits Flux2 jobs to local ComfyUI, saves results to the user's Solid pod.
///
/// Flow (user session):
///   POST /api/image-gen/submit → build Flux2 workflow → ComfyUI :8188/prompt
///   → poll /history → fetch PNG → PUT to /api/solid/pods/{user}/images/
///   → return { job_id, pod_image_url, width, height, seed }
///
/// Flow (agent / MCP):
///   POST /api/image-gen/agent-submit  (X-Agent-Key auth, no user session)
///   → same ComfyUI pipeline → PUT to embedded solid-pod-rs for target user_npub pod
///   → return { job_id, pod_image_url, comfyui_filename, seed }
///
///   GET  /api/image-gen/status/{job_id} → proxy to ComfyUI /history/{job_id}
use actix_web::{web, HttpRequest, HttpResponse};
use log::{error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(feature = "solid-pod-embed")]
use solid_pod_rs::Storage;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use crate::services::nostr_service::NostrService;

// ─── env helpers ──────────────────────────────────────────────────────────────

fn comfyui_base() -> String {
    std::env::var("COMFYUI_URL").unwrap_or_else(|_| "http://comfyui:8188".to_string())
}

fn comfyui_salad() -> String {
    // Salad wrapper: synchronous, returns { id, images:[base64], filenames, stats }
    std::env::var("COMFYUI_SALAD_URL").unwrap_or_else(|_| "http://comfyui:3000".to_string())
}

fn solid_base() -> String {
    // Internal base — goes through nginx→Rust solid proxy
    std::env::var("SOLID_INTERNAL_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4001/api/solid".to_string())
}

/// Constant-time byte comparison, so a timing side channel cannot recover the
/// agent key one byte at a time. Dependency-free fold — `subtle` and
/// `constant_time_eq` are only transitive deps here. Mirrors
/// `liveness_harness_handler::constant_time_eq` (ADR-2093).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Pure credential check, split out so the fail-closed semantics are unit
/// testable without constructing an `HttpRequest`.
///
/// ADR-2093: authorised **only** when a non-empty `VISIONCLAW_AGENT_KEY` is
/// configured and the request presents an exactly matching `X-Agent-Key`. An
/// unset or empty key fails closed — it is never substituted with a default, so
/// an unconfigured deployment cannot be driven with a publicly-known literal.
fn check_agent_key(expected: Option<&str>, provided: Option<&str>) -> bool {
    match expected.filter(|s| !s.is_empty()) {
        Some(key) => match provided {
            Some(got) => constant_time_eq(key.as_bytes(), got.as_bytes()),
            None => false,
        },
        None => false,
    }
}

/// Release posture: the key must be configured, and it is compared in constant
/// time. There is no bypass codepath here (ADR-2093, estate fail-closed posture).
#[cfg(not(any(debug_assertions, feature = "dev-auth")))]
fn agent_key_authorised(provided: Option<&str>) -> bool {
    check_agent_key(
        std::env::var("VISIONCLAW_AGENT_KEY").ok().as_deref(),
        provided,
    )
}

/// Dev / `dev-auth` builds keep the unauthenticated agent-submit flow, matching
/// the bypass `liveness_harness_handler` already uses for the same threat model.
/// This codepath does not exist in a release build.
#[cfg(any(debug_assertions, feature = "dev-auth"))]
fn agent_key_authorised(_provided: Option<&str>) -> bool {
    true
}

// ─── Request / Response types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ImageGenRequest {
    /// The text prompt to generate
    pub prompt: String,
    /// Negative prompt (optional, unused by Flux2 but stored for future use)
    #[serde(default)]
    pub negative_prompt: Option<String>,
    /// Image width (default 1024)
    #[serde(default = "default_width")]
    pub width: u32,
    /// Image height (default 1024)
    #[serde(default = "default_height")]
    pub height: u32,
    /// Number of steps (default 20)
    #[serde(default = "default_steps")]
    pub steps: u32,
    /// CFG / guidance (default 3.5 for Flux2)
    #[serde(default = "default_guidance")]
    pub guidance: f32,
    /// Random seed (-1 = random)
    #[serde(default = "default_seed")]
    pub seed: i64,
    /// Target subfolder inside pod (default "images")
    #[serde(default = "default_folder")]
    pub pod_folder: String,
}

fn default_width() -> u32 {
    1024
}
fn default_height() -> u32 {
    1024
}
fn default_steps() -> u32 {
    20
}
fn default_guidance() -> f32 {
    3.5
}
fn default_seed() -> i64 {
    -1
}
fn default_folder() -> String {
    "images".to_string()
}

#[derive(Debug, Serialize)]
pub struct ImageGenResponse {
    pub job_id: String,
    pub status: String,
    pub pod_image_url: Option<String>,
    pub comfyui_filename: Option<String>,
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    pub job_id: String,
    pub status: String,
    pub outputs: Option<Value>,
}

/// Request body for agent/MCP endpoint (no Nostr session required)
#[derive(Debug, Deserialize)]
pub struct AgentImageGenRequest {
    pub prompt: String,
    /// Nostr pubkey of the target pod owner (images stored under their pod)
    pub user_npub: Option<String>,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_steps")]
    pub steps: u32,
    #[serde(default = "default_guidance")]
    pub guidance: f32,
    #[serde(default = "default_seed")]
    pub seed: i64,
    #[serde(default = "default_folder")]
    pub pod_folder: String,
}

// ─── Flux2 workflow builder ───────────────────────────────────────────────────

fn build_flux2_workflow(req: &ImageGenRequest, seed: u64, filename_prefix: &str) -> Value {
    // Node IDs as strings (ComfyUI convention)
    // 1: UNETLoader  2: CLIPLoader  3: VAELoader
    // 4: CLIPTextEncode  5: FluxGuidance  6: BasicGuider
    // 7: RandomNoise  8: EmptyLatentImage (Flux2)  9: BasicScheduler
    // 10: SamplerCustomAdvanced  11: VAEDecode  12: SaveImage
    json!({
        "1": {
            "class_type": "UNETLoader",
            "inputs": {
                "unet_name": "flux2_dev_fp8mixed.safetensors",
                "weight_dtype": "fp8_e4m3fn"
            }
        },
        "2": {
            "class_type": "CLIPLoader",
            "inputs": {
                "clip_name": "mistral_3_small_flux2_fp8.safetensors",
                "type": "flux2"
            }
        },
        "3": {
            "class_type": "VAELoader",
            "inputs": {
                "vae_name": "flux2-vae.safetensors"
            }
        },
        "4": {
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": req.prompt,
                "clip": ["2", 0]
            }
        },
        "5": {
            "class_type": "FluxGuidance",
            "inputs": {
                "conditioning": ["4", 0],
                "guidance": req.guidance
            }
        },
        "6": {
            "class_type": "BasicGuider",
            "inputs": {
                "model": ["1", 0],
                "conditioning": ["5", 0]
            }
        },
        "7": {
            "class_type": "RandomNoise",
            "inputs": {
                "noise_seed": seed
            }
        },
        "8": {
            "class_type": "EmptySD3LatentImage",
            "inputs": {
                "width": req.width,
                "height": req.height,
                "batch_size": 1
            }
        },
        "9": {
            "class_type": "BasicScheduler",
            "inputs": {
                "model": ["1", 0],
                "scheduler": "beta",
                "steps": req.steps,
                "denoise": 1.0
            }
        },
        "13": {
            "class_type": "KSamplerSelect",
            "inputs": {
                "sampler_name": "euler"
            }
        },
        "10": {
            "class_type": "SamplerCustomAdvanced",
            "inputs": {
                "noise": ["7", 0],
                "guider": ["6", 0],
                "sampler": ["13", 0],
                "sigmas": ["9", 0],
                "latent_image": ["8", 0]
            }
        },
        "11": {
            "class_type": "VAEDecode",
            "inputs": {
                "samples": ["10", 0],
                "vae": ["3", 0]
            }
        },
        "12": {
            "class_type": "SaveImage",
            "inputs": {
                "images": ["11", 0],
                "filename_prefix": filename_prefix
            }
        }
    })
}

// ─── Helper: get authenticated user from request ──────────────────────────────

async fn get_user_npub(req: &HttpRequest, nostr_service: &NostrService) -> Option<String> {
    // Extract session token from Authorization header or cookie
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let token = token?;

    // Split "pubkey:token"
    let parts: Vec<&str> = token.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }

    let pubkey = parts[0];
    if nostr_service.validate_session(pubkey, parts[1]).await {
        Some(pubkey.to_string())
    } else {
        None
    }
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// POST /api/image-gen/submit
pub async fn submit_image_job(
    req: HttpRequest,
    body: web::Json<ImageGenRequest>,
    nostr_service: web::Data<NostrService>,
) -> HttpResponse {
    // Auth check
    let user_npub = match get_user_npub(&req, &nostr_service).await {
        Some(u) => u,
        None => {
            return HttpResponse::Unauthorized().json(json!({
                "error": "Authentication required",
                "details": "Valid Nostr session required to submit image jobs"
            }));
        }
    };

    let seed: u64 = if body.seed < 0 {
        rand::random()
    } else {
        body.seed as u64
    };

    let job_id = Uuid::new_v4().to_string();
    let filename_prefix = format!("visionclaw/{}/{}", user_npub, job_id);
    let workflow = build_flux2_workflow(&body, seed, &filename_prefix);

    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_default();

    let comfyui_url = format!("{}/prompt", comfyui_base());
    info!(
        "Submitting image job {} for user {} to ComfyUI",
        job_id,
        &user_npub[..8]
    );

    // Submit to ComfyUI native API
    let submit_resp = match client
        .post(&comfyui_url)
        .json(&json!({ "prompt": workflow, "client_id": job_id }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("ComfyUI submit failed: {}", e);
            return HttpResponse::ServiceUnavailable().json(json!({
                "error": "ComfyUI unreachable",
                "details": e.to_string()
            }));
        }
    };

    if !submit_resp.status().is_success() {
        let body = submit_resp.text().await.unwrap_or_default();
        error!("ComfyUI rejected workflow: {}", body);
        return HttpResponse::BadRequest().json(json!({
            "error": "ComfyUI rejected workflow",
            "details": body
        }));
    }

    let submit_json: Value = match submit_resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to parse ComfyUI response",
                "details": e.to_string()
            }));
        }
    };

    let prompt_id = match submit_json.get("prompt_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => {
            return HttpResponse::InternalServerError().json(json!({
                "error": "No prompt_id in ComfyUI response",
                "raw": submit_json
            }));
        }
    };

    info!("ComfyUI accepted job {} → prompt_id {}", job_id, prompt_id);

    // Poll /history/{prompt_id} until done (max ~5 min)
    let history_url = format!("{}/history/{}", comfyui_base(), prompt_id);
    let mut output_filename: Option<String> = None;
    let mut output_subfolder: Option<String> = None;

    for attempt in 0..60 {
        sleep(Duration::from_secs(5)).await;

        let history = match client.get(&history_url).send().await {
            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
            Err(e) => {
                warn!("History poll {}/60 failed: {}", attempt + 1, e);
                continue;
            }
        };

        if let Some(job) = history.get(&prompt_id) {
            if let Some(outputs) = job.get("outputs") {
                // Find SaveImage output
                for (_node_id, node_out) in outputs.as_object().unwrap_or(&Default::default()) {
                    if let Some(images) = node_out.get("images").and_then(|v| v.as_array()) {
                        if let Some(first) = images.first() {
                            output_filename = first
                                .get("filename")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            output_subfolder = first
                                .get("subfolder")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            break;
                        }
                    }
                }
                if output_filename.is_some() {
                    break;
                }
            }
        }
    }

    let filename = match output_filename {
        Some(f) => f,
        None => {
            return HttpResponse::GatewayTimeout().json(json!({
                "error": "Timed out waiting for ComfyUI to finish",
                "prompt_id": prompt_id
            }));
        }
    };

    info!("ComfyUI finished: {}", filename);

    // Fetch the PNG bytes from ComfyUI
    let subfolder = output_subfolder.as_deref().unwrap_or("");
    let view_url = format!(
        "{}/view?filename={}&subfolder={}&type=output",
        comfyui_base(),
        urlencoding::encode(&filename),
        urlencoding::encode(subfolder)
    );
    let image_bytes = match client.get(&view_url).send().await {
        Ok(r) => match r.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return HttpResponse::InternalServerError().json(json!({
                    "error": "Failed to fetch image bytes",
                    "details": e.to_string()
                }));
            }
        },
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to GET image from ComfyUI",
                "details": e.to_string()
            }));
        }
    };

    // PUT to Solid pod — path: /solid/{user}/images/{job_id}.png
    let pod_path = format!(
        "/api/solid/pods/{}/{}/{}.png",
        user_npub, body.pod_folder, job_id
    );
    let solid_url = format!(
        "{}{}",
        solid_base().trim_end_matches("/api/solid"),
        &pod_path
    );

    let pod_store_resp = client
        .put(&solid_url)
        .header("Content-Type", "image/png")
        .header(
            "Authorization",
            req.headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
        )
        .body(image_bytes)
        .send()
        .await;

    let pod_image_url = match pod_store_resp {
        Ok(r) if r.status().is_success() || r.status().as_u16() == 201 => {
            info!("Stored image in Solid pod: {}", pod_path);
            Some(pod_path)
        }
        Ok(r) => {
            warn!("Solid pod store returned {}: storing skipped", r.status());
            None
        }
        Err(e) => {
            warn!("Failed to store in Solid pod: {}", e);
            None
        }
    };

    HttpResponse::Ok().json(ImageGenResponse {
        job_id: prompt_id,
        status: "completed".to_string(),
        pod_image_url,
        comfyui_filename: Some(filename),
        width: body.width,
        height: body.height,
        seed,
        error: None,
    })
}

/// POST /api/image-gen/agent-submit
/// Uses the ComfyUI Salad wrapper (:3000) — synchronous, returns base64 images
/// in a single request. No polling needed.
///
/// Used by MCP agents in the agentic-workstation container — no user Nostr session needed.
/// Images are stored in the embedded solid-pod-rs storage under the `user_npub` pod.
pub async fn agent_submit_image_job(
    req: HttpRequest,
    body: web::Json<AgentImageGenRequest>,
) -> HttpResponse {
    // Check agent key
    let provided = req
        .headers()
        .get("x-agent-key")
        .and_then(|v| v.to_str().ok());
    if !agent_key_authorised(provided) {
        return HttpResponse::Unauthorized().json(json!({
            "error": "Invalid or missing X-Agent-Key header"
        }));
    }

    let user_npub = body
        .user_npub
        .clone()
        .unwrap_or_else(|| "agent".to_string());
    let seed: u64 = if body.seed < 0 {
        rand::random()
    } else {
        body.seed as u64
    };
    let job_id = Uuid::new_v4().to_string();
    let filename_prefix = format!("visionclaw/{}/{}", user_npub, job_id);

    let params = ImageGenRequest {
        prompt: body.prompt.clone(),
        negative_prompt: None,
        width: body.width,
        height: body.height,
        steps: body.steps,
        guidance: body.guidance,
        seed: body.seed,
        pod_folder: body.pod_folder.clone(),
    };
    let workflow = build_flux2_workflow(&params, seed, &filename_prefix);

    let client = Client::builder()
        .timeout(Duration::from_secs(360))
        .build()
        .unwrap_or_default();

    // Salad API: synchronous — one POST, get base64 images back directly
    let salad_url = format!("{}/prompt", comfyui_salad());
    info!(
        "[agent] Submitting image job {} for npub {} to ComfyUI Salad API",
        job_id,
        &user_npub[..8.min(user_npub.len())]
    );

    let salad_resp = match client
        .post(&salad_url)
        .json(&json!({ "prompt": workflow }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("[agent] ComfyUI Salad API unreachable: {}", e);
            return HttpResponse::ServiceUnavailable().json(json!({
                "error": "ComfyUI Salad API unreachable", "details": e.to_string()
            }));
        }
    };

    if !salad_resp.status().is_success() {
        let err_body = salad_resp.text().await.unwrap_or_default();
        return HttpResponse::BadRequest().json(json!({
            "error": "ComfyUI rejected workflow", "details": err_body
        }));
    }

    // Salad response: { "id": "...", "images": ["base64..."], "filenames": ["..."], "stats": {...} }
    let salad_json: Value = match salad_resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to parse Salad response", "details": e.to_string()
            }))
        }
    };

    let prompt_id = salad_json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(&job_id)
        .to_string();

    // Extract first base64 image
    let b64_image = match salad_json
        .get("images")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
    {
        Some(b64) => b64,
        None => {
            return HttpResponse::InternalServerError().json(json!({
                "error": "No images in Salad response",
                "raw": salad_json
            }))
        }
    };

    let comfyui_filename = salad_json
        .get("filenames")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    info!(
        "[agent] ComfyUI Salad returned image ({} base64 chars), file: {:?}",
        b64_image.len(),
        comfyui_filename
    );

    // Decode base64 → PNG bytes
    use base64::Engine;
    let image_bytes = match base64::engine::general_purpose::STANDARD.decode(b64_image) {
        Ok(bytes) => bytes,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to decode base64 image", "details": e.to_string()
            }))
        }
    };

    // Store in embedded solid-pod-rs
    let pod_image_url = try_store_in_pod(&image_bytes, &user_npub, &body.pod_folder, &job_id).await;

    HttpResponse::Ok().json(ImageGenResponse {
        job_id: prompt_id,
        status: "completed".to_string(),
        pod_image_url,
        comfyui_filename,
        width: body.width,
        height: body.height,
        seed,
        error: None,
    })
}

/// Store PNG bytes in the embedded solid-pod-rs storage.
/// Returns the pod-relative URL on success, None on any failure.
///
/// When `solid-pod-embed` is enabled, this writes directly to the in-process
/// Solid storage backend. No HTTP round-trip or NIP-98 signing needed.
#[cfg(feature = "solid-pod-embed")]
async fn try_store_in_pod(
    image_bytes: &[u8],
    user_npub: &str,
    pod_folder: &str,
    job_id: &str,
) -> Option<String> {
    let storage = match crate::handlers::solid_proxy_handler::get_global_storage() {
        Some(s) => s,
        None => {
            warn!("[agent] solid-pod-rs storage not initialised — skipping pod storage");
            return None;
        }
    };

    let resource_path = format!("/{}/{}/{}.png", user_npub, pod_folder, job_id);

    // Ensure the user's pod container exists before writing
    let container_path = format!("/{}/{}/", user_npub, pod_folder);
    if !storage.exists(&container_path).await.unwrap_or(false) {
        if let Err(e) = storage.create_container(&container_path).await {
            warn!(
                "[agent] Failed to create pod container {}: {}",
                container_path, e
            );
        }
    }

    match storage
        .put(
            &resource_path,
            bytes::Bytes::from(image_bytes.to_vec()),
            "image/png",
        )
        .await
    {
        Ok(_) => {
            info!("[agent] Stored image in pod: {}", resource_path);
            Some(format!("/solid{}", resource_path))
        }
        Err(e) => {
            warn!("[agent] Pod PUT failed: {}", e);
            None
        }
    }
}

#[cfg(not(feature = "solid-pod-embed"))]
async fn try_store_in_pod(
    _image_bytes: &[u8],
    _user_npub: &str,
    _pod_folder: &str,
    _job_id: &str,
) -> Option<String> {
    warn!("[agent] solid-pod-embed feature not enabled, cannot store image");
    None
}

/// GET /api/image-gen/status/{job_id}
pub async fn get_job_status(path: web::Path<String>) -> HttpResponse {
    let job_id = path.into_inner();
    let client = Client::new();
    let history_url = format!("{}/history/{}", comfyui_base(), job_id);

    match client.get(&history_url).send().await {
        Ok(r) => {
            let status = r.status();
            let body: Value = r.json().await.unwrap_or_default();
            if body.get(&job_id).is_some() {
                HttpResponse::Ok().json(JobStatusResponse {
                    job_id: job_id.clone(),
                    status: "completed".to_string(),
                    outputs: body.get(&job_id).cloned(),
                })
            } else {
                HttpResponse::Ok().json(JobStatusResponse {
                    job_id,
                    status: if status.is_success() {
                        "pending".to_string()
                    } else {
                        "unknown".to_string()
                    },
                    outputs: None,
                })
            }
        }
        Err(e) => HttpResponse::ServiceUnavailable().json(json!({
            "error": "ComfyUI unreachable",
            "details": e.to_string()
        })),
    }
}

/// GET /api/image-gen/health
pub async fn health() -> HttpResponse {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    match client
        .get(format!("{}/system_stats", comfyui_base()))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => {
            let stats: Value = r.json().await.unwrap_or_default();
            HttpResponse::Ok().json(json!({
                "status": "ok",
                "comfyui": "reachable",
                "vram_free": stats.pointer("/devices/0/vram_free"),
                "vram_total": stats.pointer("/devices/0/vram_total")
            }))
        }
        Ok(r) => HttpResponse::Ok().json(json!({
            "status": "degraded",
            "comfyui": format!("HTTP {}", r.status())
        })),
        Err(e) => HttpResponse::Ok().json(json!({
            "status": "degraded",
            "comfyui": "unreachable",
            "error": e.to_string()
        })),
    }
}

// ─── Route registration ───────────────────────────────────────────────────────

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/image-gen")
            .route("/health", web::get().to(health))
            .route("/submit", web::post().to(submit_image_job))
            .route("/agent-submit", web::post().to(agent_submit_image_job))
            .route("/status/{job_id}", web::get().to(get_job_status)),
    );
}

#[cfg(test)]
mod agent_key_tests {
    use super::{check_agent_key, constant_time_eq};

    #[test]
    fn unset_key_fails_closed() {
        // ADR-2093: the pre-fix code substituted "changeme-agent-key" here, so an
        // unconfigured deployment accepted a publicly-known literal.
        assert!(!check_agent_key(None, Some("changeme-agent-key")));
        assert!(!check_agent_key(None, Some("anything")));
        assert!(!check_agent_key(None, None));
    }

    #[test]
    fn empty_key_fails_closed() {
        assert!(!check_agent_key(Some(""), Some("")));
        assert!(!check_agent_key(Some(""), Some("x")));
    }

    #[test]
    fn missing_header_fails_closed() {
        assert!(!check_agent_key(Some("real-key"), None));
    }

    #[test]
    fn exact_match_authorises_and_mismatch_does_not() {
        assert!(check_agent_key(Some("real-key"), Some("real-key")));
        assert!(!check_agent_key(Some("real-key"), Some("real-ke")));
        assert!(!check_agent_key(Some("real-key"), Some("real-keyy")));
        assert!(!check_agent_key(Some("real-key"), Some("REAL-KEY")));
    }

    #[test]
    fn constant_time_eq_matches_equality_semantics() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
