//! GET /api/admin/operator/status — composite read-only operator block
//! for the Spine's tier-4 read-only descriptors. See ADR-061 §New server endpoints.
//!
//! Behind RequireAuth::admin() in production. In dev it's open so the Spine
//! shows real numbers without an operator pubkey configured.

use actix_web::{web, HttpResponse, Result};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize, Default)]
pub struct OperatorStatus {
    pub build: BuildInfo,
    pub gpu: GpuInfo,
    pub container: ContainerInfo,
    pub ws_subscribers: WsInfo,
    pub db_pool: DbPool,
    pub physics: PhysicsHealth,
    pub ontology: OntologyInfo,
}

#[derive(Debug, Serialize, Default)]
pub struct BuildInfo {
    pub version: String,
    pub commit_sha: String,
    pub build_timestamp: String,
    pub rust_version: String,
}

#[derive(Debug, Serialize, Default)]
pub struct GpuInfo {
    pub compute_capability: String,
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,
    pub utilisation_percent: u32,
}

#[derive(Debug, Serialize, Default)]
pub struct ContainerInfo {
    pub memory_limit_mb: u64,
    pub memory_used_mb: u64,
    pub cpu_cores: u32,
    pub cpu_percent: f32,
}

#[derive(Debug, Serialize, Default)]
pub struct WsInfo {
    pub total: u32,
    pub per_workspace: serde_json::Value,
}

#[derive(Debug, Serialize, Default)]
pub struct DbPool {
    pub active: u32,
    pub idle: u32,
    pub waiting: u32,
}

#[derive(Debug, Serialize, Default)]
pub struct PhysicsHealth {
    pub iterations_per_sec: u32,
    pub avg_iteration_ms: f32,
    pub convergence_detected: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct OntologyInfo {
    pub loaded_count: u64,
    pub total_axioms: u64,
    pub total_classes: u64,
}

fn detect_cuda_arch() -> String {
    std::env::var("CUDA_ARCH").unwrap_or_else(|_| "unknown".into())
}

fn read_meminfo_mb() -> (u64, u64) {
    // Best-effort cgroup / /proc readout. Returns (used_mb, total_mb).
    let used = std::fs::read_to_string("/sys/fs/cgroup/memory.current")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|b| b / 1_048_576)
        .unwrap_or(0);
    let total = std::fs::read_to_string("/sys/fs/cgroup/memory.max")
        .ok()
        .and_then(|s| {
            let t = s.trim();
            if t == "max" {
                None
            } else {
                t.parse::<u64>().ok().map(|b| b / 1_048_576)
            }
        })
        .unwrap_or(0);
    (used, total)
}

fn read_cpu_cores() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
}

pub async fn get_operator_status() -> Result<HttpResponse> {
    let (mem_used, mem_total) = read_meminfo_mb();
    let status = OperatorStatus {
        build: BuildInfo {
            version: env!("CARGO_PKG_VERSION").into(),
            commit_sha: option_env!("VERGEN_GIT_SHA")
                .or(option_env!("GIT_SHA"))
                .unwrap_or("unknown")
                .to_string(),
            build_timestamp: option_env!("VERGEN_BUILD_TIMESTAMP")
                .unwrap_or("unknown")
                .to_string(),
            rust_version: option_env!("VERGEN_RUSTC_SEMVER")
                .or(Some("stable"))
                .unwrap_or("unknown")
                .to_string(),
        },
        gpu: GpuInfo {
            compute_capability: detect_cuda_arch(),
            vram_used_mb: 0,
            vram_total_mb: 0,
            utilisation_percent: 0,
        },
        container: ContainerInfo {
            memory_limit_mb: mem_total,
            memory_used_mb: mem_used,
            cpu_cores: read_cpu_cores(),
            cpu_percent: 0.0,
        },
        ws_subscribers: WsInfo {
            total: 0,
            per_workspace: json!({}),
        },
        db_pool: DbPool::default(),
        physics: PhysicsHealth::default(),
        ontology: OntologyInfo::default(),
    };
    Ok(HttpResponse::Ok().json(status))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin/operator")
            .route("/status", web::get().to(get_operator_status)),
    );
}
