//! GPU Physics Pipeline Integration Tests
//!
//! Tests for the PTX loading/validation subsystem that can run without GPU hardware.
//! Orchestrator state machine tests live in src/actors/physics_orchestrator_actor.rs
//! as #[cfg(test)] unit tests with direct field access.
//!
//! Run with: cargo test --test gpu_physics_pipeline_test

// ============================================================================
// 1. PTX ISA Downgrade Tests (pure function, no GPU needed)
// ============================================================================

#[test]
fn ptx_isa_downgrade_91_to_90() {
    // CUDA toolkit 13.1 emits ISA 9.1, driver 13.0 only supports 9.0.
    // downgrade_ptx_isa_if_needed must rewrite ".version 9.1" -> ".version 9.0".
    let ptx = ".version 9.1\n.target sm_75\n.entry foo() {}".to_string();
    let result = visionclaw_server::utils::ptx::downgrade_ptx_isa_if_needed(ptx);
    assert!(
        result.contains(".version 9.0"),
        "Expected ISA 9.1 downgraded to 9.0, got: {}",
        result
    );
    assert!(
        !result.contains(".version 9.1"),
        "Original .version 9.1 should have been replaced"
    );
}

#[test]
fn ptx_isa_downgrade_92_to_90() {
    // CUDA toolkit 13.2 would emit ISA 9.2, also needs downgrade.
    let ptx = ".version 9.2\n.target sm_80\n.entry bar() {}".to_string();
    let result = visionclaw_server::utils::ptx::downgrade_ptx_isa_if_needed(ptx);
    assert!(
        result.contains(".version 9.0"),
        "Expected ISA 9.2 downgraded to 9.0, got: {}",
        result
    );
}

#[test]
fn ptx_isa_passthrough_when_already_90() {
    // PTX already at the maximum supported version -- no rewriting needed.
    let ptx = ".version 9.0\n.target sm_75\n.entry baz() {}".to_string();
    let result = visionclaw_server::utils::ptx::downgrade_ptx_isa_if_needed(ptx.clone());
    assert_eq!(result, ptx, "PTX at 9.0 should pass through unchanged");
}

#[test]
fn ptx_isa_passthrough_when_below_max() {
    // PTX at 7.4 (old toolkit) -- well below the 9.0 cap.
    let ptx = ".version 7.4\n.target sm_52\n.entry old_kernel() {}".to_string();
    let result = visionclaw_server::utils::ptx::downgrade_ptx_isa_if_needed(ptx.clone());
    assert_eq!(result, ptx, "PTX at 7.4 should pass through unchanged");
}

// ============================================================================
// 2. PTX Validation Tests
// ============================================================================

#[test]
fn ptx_validate_rejects_missing_version() {
    let ptx = ".target sm_75\n.entry foo() {}";
    let err = visionclaw_server::utils::ptx::validate_ptx(ptx).unwrap_err();
    assert!(
        err.contains(".version"),
        "Error should mention .version: {}",
        err
    );
}

#[test]
fn ptx_validate_rejects_missing_target() {
    let ptx = ".version 9.0\n.entry foo() {}";
    let err = visionclaw_server::utils::ptx::validate_ptx(ptx).unwrap_err();
    assert!(
        err.contains(".target"),
        "Error should mention .target: {}",
        err
    );
}

#[test]
fn ptx_validate_rejects_missing_entry() {
    let ptx = ".version 9.0\n.target sm_75\n// no kernel entry point";
    let err = visionclaw_server::utils::ptx::validate_ptx(ptx).unwrap_err();
    assert!(
        err.contains(".entry"),
        "Error should mention .entry: {}",
        err
    );
}

#[test]
fn ptx_validate_accepts_well_formed() {
    let ptx = ".version 9.0\n.target sm_75\n.entry my_kernel() { ret; }";
    assert!(
        visionclaw_server::utils::ptx::validate_ptx(ptx).is_ok(),
        "Well-formed PTX should pass validation"
    );
}

// ============================================================================
// 3. PTX Module Metadata Consistency
// ============================================================================

#[test]
fn ptx_module_source_and_env_var_consistency() {
    // Every PTX module must have a .cu source and a matching _PTX_PATH env var.
    // Catches copy-paste errors when adding new CUDA kernel modules.
    use visionclaw_server::utils::ptx::PTXModule;

    let modules = PTXModule::all_modules();
    assert!(
        modules.len() >= 10,
        "Expected >= 10 PTX modules, got {}",
        modules.len()
    );

    for module in &modules {
        let src = module.source_file();
        assert!(
            src.ends_with(".cu"),
            "{:?} source should be .cu: {}",
            module,
            src
        );

        let env = module.env_var();
        assert!(
            env.ends_with("_PTX_PATH"),
            "{:?} env var should end _PTX_PATH: {}",
            module,
            env
        );

        // Stem of .cu file should appear uppercase in env var name
        let stem = src.replace(".cu", "").to_uppercase();
        assert!(
            env.contains(&stem),
            "Env var '{}' for {:?} should contain stem '{}' from '{}'",
            env,
            module,
            stem,
            src
        );
    }
}
