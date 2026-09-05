//! GPU stability probes.
//!
//! Port of `tests/integration/gpu_stability_test.py`. The Python suite reached
//! CUDA the long way round — `docker exec mcp-gui-tools python -c "import
//! torch; torch.cuda.is_available()"` — dragging a torch install into a test
//! whose only real question was *is there a working GPU*. `nvml-wrapper` asks
//! the driver directly, in process, with no container and no Python.
//!
//! Every probe skips cleanly when NVML is unavailable (no driver, no GPU, or a
//! container without the device mapped), so this file is safe on CPU-only hosts.

use nvml_wrapper::Nvml;
use serde_json::Value;
use visionclaw_integration_tests::require_server;

/// Initialise NVML, or explain the skip.
///
/// NVML is the driver's own management interface, so a successful `init` plus a
/// non-zero device count is the same assertion `torch.cuda.is_available()` made,
/// minus the runtime.
fn nvml() -> Option<Nvml> {
    match Nvml::init() {
        Ok(nvml) => Some(nvml),
        Err(e) => {
            eprintln!("SKIP: NVML unavailable ({e}) — no GPU visible to this process.");
            None
        }
    }
}

#[test]
fn a_cuda_device_is_present_and_identifiable() {
    let Some(nvml) = nvml() else { return };

    let count = nvml
        .device_count()
        .expect("NVML initialised but the device count failed");
    assert!(count > 0, "NVML initialised but reports zero devices");

    let device = nvml
        .device_by_index(0)
        .expect("device 0 could not be opened");
    let name = device.name().expect("device 0 has no readable name");
    assert!(!name.is_empty(), "device 0 reported an empty name");

    let memory = device
        .memory_info()
        .expect("device 0 has no readable memory info");
    assert!(memory.total > 0, "device 0 reports zero total memory");

    eprintln!(
        "GPU: {name} — {:.1} GiB total, {:.1} GiB free",
        memory.total as f64 / 1024.0 / 1024.0 / 1024.0,
        memory.free as f64 / 1024.0 / 1024.0 / 1024.0
    );
}

#[test]
fn gpu_memory_reporting_is_stable_under_repeated_queries() {
    let Some(nvml) = nvml() else { return };
    let device = match nvml.device_by_index(0) {
        Ok(device) => device,
        Err(e) => {
            eprintln!("SKIP: device 0 could not be opened ({e}).");
            return;
        }
    };

    let baseline = device
        .memory_info()
        .expect("baseline memory read failed")
        .total;

    // Ten reads over five seconds. Total memory is a fixed property of the
    // device: if it moves, the driver state is not what we think it is.
    for i in 0..10 {
        let memory = device
            .memory_info()
            .unwrap_or_else(|e| panic!("memory read {i} of 10 failed: {e}"));
        assert_eq!(
            memory.total, baseline,
            "total memory changed between reads {i} and 0"
        );
        assert!(
            memory.used <= memory.total,
            "used memory exceeds total on read {i}"
        );
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

#[test]
fn concurrent_queries_do_not_destabilise_the_driver() {
    let Some(nvml) = nvml() else { return };
    if nvml.device_count().unwrap_or(0) == 0 {
        eprintln!("SKIP: no devices to query.");
        return;
    }

    // Five threads each opening their own handle — the shape of the Python
    // suite's concurrent `torch.matmul` check, without allocating a gigabyte.
    let outcomes: Vec<bool> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..5)
            .map(|_| {
                scope.spawn(|| {
                    let Ok(nvml) = Nvml::init() else { return false };
                    let Ok(device) = nvml.device_by_index(0) else {
                        return false;
                    };
                    (0..5).all(|_| device.memory_info().is_ok())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(false))
            .collect()
    });

    let succeeded = outcomes.iter().filter(|ok| **ok).count();
    assert_eq!(
        succeeded, 5,
        "only {succeeded} of 5 concurrent NVML readers succeeded"
    );
}

#[tokio::test]
async fn the_health_endpoint_reports_gpu_status() {
    let h = require_server!();

    let Some(response) = h.get("/health").await else {
        eprintln!("SKIP: /health did not answer.");
        return;
    };
    assert!(
        response.status().is_success(),
        "/health answered {}",
        response.status()
    );

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            eprintln!("SKIP: /health returned a non-JSON body ({e}).");
            return;
        }
    };

    // The server may report GPU readiness under any of these keys depending on
    // build features; the assertion is that *something* says so, not which.
    let reported = ["gpu", "gpu_enabled", "gpu_status", "cuda", "gpu_available"]
        .iter()
        .any(|key| body.get(*key).is_some());

    assert!(
        reported,
        "/health carries no GPU status field at all: {body}"
    );
}
