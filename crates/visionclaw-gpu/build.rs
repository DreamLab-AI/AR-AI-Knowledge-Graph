//! build.rs for visionclaw-gpu — ADR-090 Phase 3
//!
//! Compiles all CUDA kernels to PTX and (where possible) to native object files
//! for linking. All .cu sources now live at `src/cuda_sources/` within this crate.
//!
//! Lifted from the root webxr build.rs; the root build.rs retains a stub that
//! delegates CUDA compilation to this crate via the workspace dep.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Check if GPU feature is enabled
    let gpu_enabled = env::var("CARGO_FEATURE_GPU").is_ok();

    if !gpu_enabled {
        println!("cargo:warning=visionclaw-gpu: GPU feature disabled, skipping CUDA compilation");
        return;
    }

    // All CUDA source files — paths are relative to this crate root
    let cuda_files = [
        "src/cuda_sources/visionclaw_unified.cu",
        "src/cuda_sources/gpu_clustering_kernels.cu",
        "src/cuda_sources/dynamic_grid.cu",
        "src/cuda_sources/gpu_aabb_reduction.cu",
        "src/cuda_sources/gpu_landmark_apsp.cu",
        "src/cuda_sources/sssp_compact.cu",
        "src/cuda_sources/semantic_forces.cu",
        "src/cuda_sources/pagerank.cu",
        "src/cuda_sources/gpu_connected_components.cu",
    ];

    // Rebuild triggers
    for cuda_file in &cuda_files {
        println!("cargo:rerun-if-changed={}", cuda_file);
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CUDA_ARCH");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=DOCKER_ENV");

    let out_dir = env::var("OUT_DIR").unwrap();
    let cuda_path = env::var("CUDA_PATH")
        .or_else(|_| env::var("CUDA_HOME"))
        .unwrap_or_else(|_| "/opt/cuda".to_string());

    // CUDA architecture selection.
    // Docker builds: never auto-detect (build GPU != runtime GPU). Default sm_75 portable baseline.
    let is_docker = env::var("DOCKER_ENV").is_ok();
    let cuda_arch = env::var("CUDA_ARCH").unwrap_or_else(|_| {
        if is_docker {
            println!("cargo:warning=visionclaw-gpu: Docker build — using portable sm_75 (set CUDA_ARCH to override)");
            return "75".to_string();
        }
        if let Ok(output) = Command::new("nvidia-smi")
            .args(["--query-gpu=compute_cap", "--format=csv,noheader", "--id=0"])
            .output()
        {
            if output.status.success() {
                let raw = String::from_utf8_lossy(&output.stdout);
                if let Some(cap) = raw.lines().next() {
                    let arch = cap.trim().replace('.', "");
                    if !arch.is_empty() {
                        println!("cargo:warning=visionclaw-gpu: Auto-detected GPU sm_{}", arch);
                        return arch;
                    }
                }
            }
        }
        "75".to_string()
    });
    println!(
        "cargo:warning=visionclaw-gpu: Building for sm_{}",
        cuda_arch
    );

    // Find a CUDA-compatible host compiler (nvcc supports up to GCC 14).
    // CachyOS ships GCC 16 which is too new.
    let cuda_host_compiler = [
        "/usr/bin/g++-13",
        "/usr/bin/g++-14",
        "/opt/cuda/bin/gcc",
        "/usr/local/bin/g++-13",
    ]
    .iter()
    .find(|p| Path::new(p).exists())
    .map(|s| s.to_string());

    if let Some(ref cc) = cuda_host_compiler {
        println!(
            "cargo:warning=visionclaw-gpu: Using CUDA host compiler: {}",
            cc
        );
    }

    // ── Phase 1: PTX compilation ──────────────────────────────────────────────
    println!(
        "cargo:warning=visionclaw-gpu: Compiling {} CUDA kernels to PTX",
        cuda_files.len()
    );

    for cuda_file in &cuda_files {
        let cuda_src = Path::new(cuda_file);
        let file_name = cuda_src.file_stem().unwrap().to_str().unwrap();
        let ptx_output = PathBuf::from(&out_dir).join(format!("{}.ptx", file_name));

        let mut nvcc_args: Vec<String> = vec![
            "-ptx".into(),
            "-arch".into(),
            format!("sm_{}", cuda_arch),
            "-o".into(),
            ptx_output.to_str().unwrap().into(),
            cuda_src.to_str().unwrap().into(),
            "--use_fast_math".into(),
            "-O3".into(),
            "-std=c++17".into(),
            "--allow-unsupported-compiler".into(),
            "--expt-relaxed-constexpr".into(),
        ];

        if let Some(ref cc) = cuda_host_compiler {
            nvcc_args.push("--compiler-bindir".into());
            nvcc_args.push(cc.clone());
        }

        let nvcc_output = Command::new("nvcc")
            .args(&nvcc_args)
            .output()
            .expect("Failed to execute nvcc — is the CUDA toolkit installed?");

        if !nvcc_output.status.success() {
            let stderr = String::from_utf8_lossy(&nvcc_output.stderr);
            eprintln!("NVCC STDERR: {}", stderr);

            // Fallback: pre-compiled PTX bundled with the crate or from /app image
            let fallback_paths = [
                format!("src/ptx/{}.ptx", file_name),
                format!("/app/src/utils/ptx/{}.ptx", file_name),
                // Legacy path — Docker image may still have them here
                format!("/app/crates/visionclaw-gpu/src/ptx/{}.ptx", file_name),
            ];
            let fallback = fallback_paths.iter().find(|p| Path::new(p).exists());

            if let Some(fb) = fallback {
                println!(
                    "cargo:warning=visionclaw-gpu: NVCC failed for {} — using pre-compiled PTX from {}",
                    file_name, fb
                );
                std::fs::copy(fb, &ptx_output).expect("Failed to copy fallback PTX");
            } else {
                panic!(
                    "CUDA PTX compilation failed for {} (exit {:?}) and no fallback PTX found.\n\
                     Install gcc-13 or gcc-14: pacman -S gcc13",
                    file_name,
                    nvcc_output.status.code()
                );
            }
        }

        // Downgrade PTX ISA to 9.0 for driver compatibility.
        // CUDA toolkit 13.x emits .version 9.x; some host drivers only support 9.0.
        if let Ok(ptx_text) = std::fs::read_to_string(&ptx_output) {
            if let Some(pos) = ptx_text.find(".version 9.") {
                let slice_end = (pos + 13).min(ptx_text.len());
                let version_str = &ptx_text[pos..slice_end];
                if version_str != ".version 9.0" {
                    let fixed =
                        ptx_text[..pos].to_string() + ".version 9.0" + &ptx_text[pos + 12..];
                    std::fs::write(&ptx_output, fixed).expect("Failed to write downgraded PTX");
                    println!(
                        "cargo:warning=visionclaw-gpu: Downgraded {} -> 9.0 for {}",
                        version_str.trim(),
                        file_name
                    );
                }
            }
        }

        match std::fs::metadata(&ptx_output) {
            Ok(meta) if meta.len() > 0 => {
                let env_var = format!("{}_PTX_PATH", file_name.to_uppercase());
                println!("cargo:rustc-env={}={}", env_var, ptx_output.display());
            }
            Ok(_) => panic!("PTX file {} is empty after compilation", file_name),
            Err(e) => panic!("PTX file {} not created: {}", file_name, e),
        }
    }

    println!("cargo:warning=visionclaw-gpu: All PTX compilation done");

    // ── Phase 2: Native linking (FFI symbols) ─────────────────────────────────
    // These four .cu files export host-callable FFI symbols and must be linked
    // as a static library so the webxr binary can call them.
    let link_sources = [
        ("src/cuda_sources/visionclaw_unified.cu", "thrust_wrapper"),
        ("src/cuda_sources/semantic_forces.cu", "semantic_forces"),
        ("src/cuda_sources/pagerank.cu", "pagerank"),
        (
            "src/cuda_sources/gpu_connected_components.cu",
            "gpu_connected_components",
        ),
    ];

    let mut obj_files: Vec<PathBuf> = Vec::new();

    for (src_path, obj_name) in &link_sources {
        let cuda_src = Path::new(src_path);
        let obj_output = PathBuf::from(&out_dir).join(format!("{}.o", obj_name));
        let gencode = format!(
            "-gencode=arch=compute_{0},code=[sm_{0},compute_{0}]",
            cuda_arch
        );

        let mut obj_args: Vec<String> = vec![
            "-c".into(),
            gencode,
            "-o".into(),
            obj_output.to_str().unwrap().into(),
            cuda_src.to_str().unwrap().into(),
            "--use_fast_math".into(),
            "-O3".into(),
            "-Xcompiler".into(),
            "-fPIC".into(),
            "-dc".into(),
            "-std=c++17".into(),
            "--allow-unsupported-compiler".into(),
            "--expt-relaxed-constexpr".into(),
        ];

        if let Some(ref cc) = cuda_host_compiler {
            obj_args.push("--compiler-bindir".into());
            obj_args.push(cc.clone());
        }

        let result = Command::new("nvcc")
            .args(&obj_args)
            .output()
            .expect(&format!("Failed to compile {}", obj_name));

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            println!(
                "cargo:warning=visionclaw-gpu: Native object compilation failed for {}: {}",
                obj_name,
                stderr.lines().last().unwrap_or("unknown error")
            );
            println!("cargo:warning=visionclaw-gpu: Falling back to PTX-only JIT mode");
            obj_files.clear();
            break;
        }
        obj_files.push(obj_output);
    }

    if !obj_files.is_empty() {
        let dlink_output = PathBuf::from(&out_dir).join("cuda_dlink.o");
        let dlink_gencode = format!(
            "-gencode=arch=compute_{0},code=[sm_{0},compute_{0}]",
            cuda_arch
        );
        let mut dlink_args = vec!["-dlink".to_string(), dlink_gencode];
        for obj in &obj_files {
            dlink_args.push(obj.to_str().unwrap().to_string());
        }
        dlink_args.extend(["-o".to_string(), dlink_output.to_str().unwrap().to_string()]);

        let dlink_status = Command::new("nvcc")
            .args(&dlink_args)
            .status()
            .expect("Device link failed");
        if !dlink_status.success() {
            panic!("Device linking step failed");
        }

        let lib_output = PathBuf::from(&out_dir).join("libthrust_wrapper.a");
        let mut ar_args = vec!["rcs".to_string(), lib_output.to_str().unwrap().to_string()];
        for obj in &obj_files {
            ar_args.push(obj.to_str().unwrap().to_string());
        }
        ar_args.push(dlink_output.to_str().unwrap().to_string());

        let ar_status = Command::new("ar")
            .args(&ar_args)
            .status()
            .expect("ar failed");
        if !ar_status.success() {
            panic!("Failed to create libthrust_wrapper.a");
        }

        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=thrust_wrapper");
        println!("cargo:rustc-link-search=native={}/lib64", cuda_path);
        println!("cargo:rustc-link-search=native={}/lib64/stubs", cuda_path);
        println!("cargo:rustc-link-lib=cudart");
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rustc-link-lib=cudadevrt");
        println!("cargo:rustc-link-lib=stdc++");

        println!("cargo:warning=visionclaw-gpu: Native CUDA linking complete");
    } else {
        // PTX-only mode: stub out FFI symbols so the linker is satisfied.
        // The stub lives in the webxr monolith at src/utils/cuda_ffi_stubs.c —
        // reference it from here via an absolute-ish relative path. If this path
        // is wrong the linker will emit an error pointing here.
        let stub_candidates = [
            // Relative to this crate (when building from workspace)
            "../../src/utils/cuda_ffi_stubs.c",
            // Absolute Docker image path
            "/app/src/utils/cuda_ffi_stubs.c",
        ];
        let stub_src = stub_candidates
            .iter()
            .find(|p| Path::new(p).exists())
            .map(Path::new)
            .expect("cuda_ffi_stubs.c not found — cannot provide FFI symbols in PTX-only mode");

        let stub_obj = PathBuf::from(&out_dir).join("cuda_ffi_stubs.o");
        let stub_lib = PathBuf::from(&out_dir).join("libthrust_wrapper.a");
        let cc = cuda_host_compiler.as_deref().unwrap_or("gcc");

        let cc_status = Command::new(cc)
            .args(["-c", "-fPIC", "-o"])
            .arg(&stub_obj)
            .arg(stub_src)
            .status()
            .expect("Failed to compile cuda_ffi_stubs.c");
        if !cc_status.success() {
            panic!("cuda_ffi_stubs.c compilation failed");
        }

        let ar_status = Command::new("ar")
            .args(["rcs"])
            .arg(&stub_lib)
            .arg(&stub_obj)
            .status()
            .expect("ar failed for stub library");
        if !ar_status.success() {
            panic!("Failed to create libthrust_wrapper.a from stubs");
        }

        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=thrust_wrapper");
        println!("cargo:rustc-link-search=native={}/lib64", cuda_path);
        println!("cargo:rustc-link-search=native={}/lib64/stubs", cuda_path);
        println!("cargo:rustc-link-lib=cudart");
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rustc-link-lib=stdc++");

        println!("cargo:warning=visionclaw-gpu: PTX-only mode with FFI stubs");
    }
}
