//! SIMD-accelerated force computations for CPU fallback paths
//!
//! Provides AVX2 (256-bit, 8 floats), SSE4.1 (128-bit, 4 floats), and scalar
//! implementations with runtime feature detection via `is_x86_feature_detected!`.
//! Non-x86 targets fall through to scalar code automatically.
//!
//! # Architecture
//!
//! Each public function performs runtime dispatch:
//! 1. AVX2 path  -- processes 8 elements per iteration
//! 2. SSE4.1 path -- processes 4 elements per iteration
//! 3. Scalar path -- processes 1 element per iteration (always available)
//!
//! The tail elements that don't fill a full SIMD lane are handled by the scalar
//! remainder loop in every implementation.

// ---------------------------------------------------------------------------
// Runtime detection helpers
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
#[inline]
fn has_avx2() -> bool {
    is_x86_feature_detected!("avx2")
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn has_sse41() -> bool {
    is_x86_feature_detected!("sse4.1")
}

// ===========================================================================
// 1. Pairwise distance computation
// ===========================================================================

/// Compute pairwise Euclidean distances between two sets of 3D points.
///
/// `distances[i] = sqrt((pos_x[i]-other_x[i])^2 + (pos_y[i]-other_y[i])^2 + (pos_z[i]-other_z[i])^2)`
///
/// All slices must have the same length.
pub fn compute_distances_simd(
    pos_x: &[f32],
    pos_y: &[f32],
    pos_z: &[f32],
    other_x: &[f32],
    other_y: &[f32],
    other_z: &[f32],
    distances: &mut [f32],
) {
    let n = pos_x
        .len()
        .min(pos_y.len())
        .min(pos_z.len())
        .min(other_x.len())
        .min(other_y.len())
        .min(other_z.len())
        .min(distances.len());

    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            // SAFETY: feature check guarantees AVX2+FMA are available on this CPU.
            unsafe {
                compute_distances_avx2(
                    pos_x, pos_y, pos_z, other_x, other_y, other_z, distances, n,
                );
            }
            return;
        }
        if has_sse41() {
            unsafe {
                compute_distances_sse41(
                    pos_x, pos_y, pos_z, other_x, other_y, other_z, distances, n,
                );
            }
            return;
        }
    }

    compute_distances_scalar(pos_x, pos_y, pos_z, other_x, other_y, other_z, distances, n);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn compute_distances_avx2(
    pos_x: &[f32],
    pos_y: &[f32],
    pos_z: &[f32],
    other_x: &[f32],
    other_y: &[f32],
    other_z: &[f32],
    distances: &mut [f32],
    n: usize,
) {
    use std::arch::x86_64::*;

    let chunks = n / 8;
    for c in 0..chunks {
        let i = c * 8;

        let px = _mm256_loadu_ps(pos_x.as_ptr().add(i));
        let py = _mm256_loadu_ps(pos_y.as_ptr().add(i));
        let pz = _mm256_loadu_ps(pos_z.as_ptr().add(i));

        let ox = _mm256_loadu_ps(other_x.as_ptr().add(i));
        let oy = _mm256_loadu_ps(other_y.as_ptr().add(i));
        let oz = _mm256_loadu_ps(other_z.as_ptr().add(i));

        let dx = _mm256_sub_ps(px, ox);
        let dy = _mm256_sub_ps(py, oy);
        let dz = _mm256_sub_ps(pz, oz);

        // dx*dx + dy*dy + dz*dz using FMA
        let mut sq = _mm256_mul_ps(dx, dx);
        sq = _mm256_fmadd_ps(dy, dy, sq);
        sq = _mm256_fmadd_ps(dz, dz, sq);

        let dist = _mm256_sqrt_ps(sq);
        _mm256_storeu_ps(distances.as_mut_ptr().add(i), dist);
    }

    // Scalar remainder
    let tail_start = chunks * 8;
    for i in tail_start..n {
        let dx = pos_x[i] - other_x[i];
        let dy = pos_y[i] - other_y[i];
        let dz = pos_z[i] - other_z[i];
        distances[i] = (dx * dx + dy * dy + dz * dz).sqrt();
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn compute_distances_sse41(
    pos_x: &[f32],
    pos_y: &[f32],
    pos_z: &[f32],
    other_x: &[f32],
    other_y: &[f32],
    other_z: &[f32],
    distances: &mut [f32],
    n: usize,
) {
    use std::arch::x86_64::*;

    let chunks = n / 4;
    for c in 0..chunks {
        let i = c * 4;

        let px = _mm_loadu_ps(pos_x.as_ptr().add(i));
        let py = _mm_loadu_ps(pos_y.as_ptr().add(i));
        let pz = _mm_loadu_ps(pos_z.as_ptr().add(i));

        let ox = _mm_loadu_ps(other_x.as_ptr().add(i));
        let oy = _mm_loadu_ps(other_y.as_ptr().add(i));
        let oz = _mm_loadu_ps(other_z.as_ptr().add(i));

        let dx = _mm_sub_ps(px, ox);
        let dy = _mm_sub_ps(py, oy);
        let dz = _mm_sub_ps(pz, oz);

        let dx2 = _mm_mul_ps(dx, dx);
        let dy2 = _mm_mul_ps(dy, dy);
        let dz2 = _mm_mul_ps(dz, dz);

        let sq = _mm_add_ps(_mm_add_ps(dx2, dy2), dz2);
        let dist = _mm_sqrt_ps(sq);
        _mm_storeu_ps(distances.as_mut_ptr().add(i), dist);
    }

    let tail_start = chunks * 4;
    for i in tail_start..n {
        let dx = pos_x[i] - other_x[i];
        let dy = pos_y[i] - other_y[i];
        let dz = pos_z[i] - other_z[i];
        distances[i] = (dx * dx + dy * dy + dz * dz).sqrt();
    }
}

fn compute_distances_scalar(
    pos_x: &[f32],
    pos_y: &[f32],
    pos_z: &[f32],
    other_x: &[f32],
    other_y: &[f32],
    other_z: &[f32],
    distances: &mut [f32],
    n: usize,
) {
    for i in 0..n {
        let dx = pos_x[i] - other_x[i];
        let dy = pos_y[i] - other_y[i];
        let dz = pos_z[i] - other_z[i];
        distances[i] = (dx * dx + dy * dy + dz * dz).sqrt();
    }
}

// ===========================================================================
// 2. Force accumulation
// ===========================================================================

/// Accumulate force vectors scaled by magnitude: `forces[i] += delta[i] * magnitudes[i]`
///
/// All x/y/z and magnitude slices must have the same length.
pub fn accumulate_forces_simd(
    forces_x: &mut [f32],
    forces_y: &mut [f32],
    forces_z: &mut [f32],
    delta_x: &[f32],
    delta_y: &[f32],
    delta_z: &[f32],
    magnitudes: &[f32],
) {
    let n = forces_x
        .len()
        .min(forces_y.len())
        .min(forces_z.len())
        .min(delta_x.len())
        .min(delta_y.len())
        .min(delta_z.len())
        .min(magnitudes.len());

    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            unsafe {
                accumulate_forces_avx2(
                    forces_x, forces_y, forces_z, delta_x, delta_y, delta_z, magnitudes, n,
                );
            }
            return;
        }
        if has_sse41() {
            unsafe {
                accumulate_forces_sse41(
                    forces_x, forces_y, forces_z, delta_x, delta_y, delta_z, magnitudes, n,
                );
            }
            return;
        }
    }

    accumulate_forces_scalar(
        forces_x, forces_y, forces_z, delta_x, delta_y, delta_z, magnitudes, n,
    );
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn accumulate_forces_avx2(
    forces_x: &mut [f32],
    forces_y: &mut [f32],
    forces_z: &mut [f32],
    delta_x: &[f32],
    delta_y: &[f32],
    delta_z: &[f32],
    magnitudes: &[f32],
    n: usize,
) {
    use std::arch::x86_64::*;

    let chunks = n / 8;
    for c in 0..chunks {
        let i = c * 8;

        let mag = _mm256_loadu_ps(magnitudes.as_ptr().add(i));

        let fx = _mm256_loadu_ps(forces_x.as_ptr().add(i));
        let fy = _mm256_loadu_ps(forces_y.as_ptr().add(i));
        let fz = _mm256_loadu_ps(forces_z.as_ptr().add(i));

        let dx = _mm256_loadu_ps(delta_x.as_ptr().add(i));
        let dy = _mm256_loadu_ps(delta_y.as_ptr().add(i));
        let dz = _mm256_loadu_ps(delta_z.as_ptr().add(i));

        // forces += delta * magnitude  (FMA: a*b+c)
        let new_fx = _mm256_fmadd_ps(dx, mag, fx);
        let new_fy = _mm256_fmadd_ps(dy, mag, fy);
        let new_fz = _mm256_fmadd_ps(dz, mag, fz);

        _mm256_storeu_ps(forces_x.as_mut_ptr().add(i), new_fx);
        _mm256_storeu_ps(forces_y.as_mut_ptr().add(i), new_fy);
        _mm256_storeu_ps(forces_z.as_mut_ptr().add(i), new_fz);
    }

    let tail = chunks * 8;
    for i in tail..n {
        forces_x[i] += delta_x[i] * magnitudes[i];
        forces_y[i] += delta_y[i] * magnitudes[i];
        forces_z[i] += delta_z[i] * magnitudes[i];
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn accumulate_forces_sse41(
    forces_x: &mut [f32],
    forces_y: &mut [f32],
    forces_z: &mut [f32],
    delta_x: &[f32],
    delta_y: &[f32],
    delta_z: &[f32],
    magnitudes: &[f32],
    n: usize,
) {
    use std::arch::x86_64::*;

    let chunks = n / 4;
    for c in 0..chunks {
        let i = c * 4;

        let mag = _mm_loadu_ps(magnitudes.as_ptr().add(i));

        let fx = _mm_loadu_ps(forces_x.as_ptr().add(i));
        let fy = _mm_loadu_ps(forces_y.as_ptr().add(i));
        let fz = _mm_loadu_ps(forces_z.as_ptr().add(i));

        let dx = _mm_loadu_ps(delta_x.as_ptr().add(i));
        let dy = _mm_loadu_ps(delta_y.as_ptr().add(i));
        let dz = _mm_loadu_ps(delta_z.as_ptr().add(i));

        let new_fx = _mm_add_ps(fx, _mm_mul_ps(dx, mag));
        let new_fy = _mm_add_ps(fy, _mm_mul_ps(dy, mag));
        let new_fz = _mm_add_ps(fz, _mm_mul_ps(dz, mag));

        _mm_storeu_ps(forces_x.as_mut_ptr().add(i), new_fx);
        _mm_storeu_ps(forces_y.as_mut_ptr().add(i), new_fy);
        _mm_storeu_ps(forces_z.as_mut_ptr().add(i), new_fz);
    }

    let tail = chunks * 4;
    for i in tail..n {
        forces_x[i] += delta_x[i] * magnitudes[i];
        forces_y[i] += delta_y[i] * magnitudes[i];
        forces_z[i] += delta_z[i] * magnitudes[i];
    }
}

fn accumulate_forces_scalar(
    forces_x: &mut [f32],
    forces_y: &mut [f32],
    forces_z: &mut [f32],
    delta_x: &[f32],
    delta_y: &[f32],
    delta_z: &[f32],
    magnitudes: &[f32],
    n: usize,
) {
    for i in 0..n {
        forces_x[i] += delta_x[i] * magnitudes[i];
        forces_y[i] += delta_y[i] * magnitudes[i];
        forces_z[i] += delta_z[i] * magnitudes[i];
    }
}

// ===========================================================================
// 3. Position integration (Verlet / Euler with damping)
// ===========================================================================

/// Integrate positions using semi-implicit Euler:
///   vel[i] = (vel[i] + (force[i] / mass[i]) * dt) * damping
///   pos[i] += vel[i] * dt
///
/// All component slices must have the same length.
pub fn integrate_positions_simd(
    pos_x: &mut [f32],
    pos_y: &mut [f32],
    pos_z: &mut [f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    force_x: &[f32],
    force_y: &[f32],
    force_z: &[f32],
    mass: &[f32],
    dt: f32,
    damping: f32,
) {
    let n = pos_x
        .len()
        .min(pos_y.len())
        .min(pos_z.len())
        .min(vel_x.len())
        .min(vel_y.len())
        .min(vel_z.len())
        .min(force_x.len())
        .min(force_y.len())
        .min(force_z.len())
        .min(mass.len());

    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            unsafe {
                integrate_positions_avx2(
                    pos_x, pos_y, pos_z, vel_x, vel_y, vel_z, force_x, force_y, force_z, mass, dt,
                    damping, n,
                );
            }
            return;
        }
        if has_sse41() {
            unsafe {
                integrate_positions_sse41(
                    pos_x, pos_y, pos_z, vel_x, vel_y, vel_z, force_x, force_y, force_z, mass, dt,
                    damping, n,
                );
            }
            return;
        }
    }

    integrate_positions_scalar(
        pos_x, pos_y, pos_z, vel_x, vel_y, vel_z, force_x, force_y, force_z, mass, dt, damping, n,
    );
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn integrate_positions_avx2(
    pos_x: &mut [f32],
    pos_y: &mut [f32],
    pos_z: &mut [f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    force_x: &[f32],
    force_y: &[f32],
    force_z: &[f32],
    mass: &[f32],
    dt: f32,
    damping: f32,
    n: usize,
) {
    use std::arch::x86_64::*;

    let v_dt = _mm256_set1_ps(dt);
    let v_damp = _mm256_set1_ps(damping);
    let v_one = _mm256_set1_ps(1.0);

    let chunks = n / 8;
    for c in 0..chunks {
        let i = c * 8;

        let m = _mm256_loadu_ps(mass.as_ptr().add(i));
        // inv_mass = 1.0 / mass (avoid div-by-zero: mass should always be > 0)
        let inv_mass = _mm256_div_ps(v_one, m);

        // Load forces
        let fx = _mm256_loadu_ps(force_x.as_ptr().add(i));
        let fy = _mm256_loadu_ps(force_y.as_ptr().add(i));
        let fz = _mm256_loadu_ps(force_z.as_ptr().add(i));

        // accel = force * inv_mass
        let ax = _mm256_mul_ps(fx, inv_mass);
        let ay = _mm256_mul_ps(fy, inv_mass);
        let az = _mm256_mul_ps(fz, inv_mass);

        // Load velocities
        let mut vx = _mm256_loadu_ps(vel_x.as_ptr().add(i));
        let mut vy = _mm256_loadu_ps(vel_y.as_ptr().add(i));
        let mut vz = _mm256_loadu_ps(vel_z.as_ptr().add(i));

        // vel = (vel + accel * dt) * damping
        vx = _mm256_mul_ps(_mm256_fmadd_ps(ax, v_dt, vx), v_damp);
        vy = _mm256_mul_ps(_mm256_fmadd_ps(ay, v_dt, vy), v_damp);
        vz = _mm256_mul_ps(_mm256_fmadd_ps(az, v_dt, vz), v_damp);

        _mm256_storeu_ps(vel_x.as_mut_ptr().add(i), vx);
        _mm256_storeu_ps(vel_y.as_mut_ptr().add(i), vy);
        _mm256_storeu_ps(vel_z.as_mut_ptr().add(i), vz);

        // pos += vel * dt
        let px = _mm256_loadu_ps(pos_x.as_ptr().add(i));
        let py = _mm256_loadu_ps(pos_y.as_ptr().add(i));
        let pz = _mm256_loadu_ps(pos_z.as_ptr().add(i));

        _mm256_storeu_ps(pos_x.as_mut_ptr().add(i), _mm256_fmadd_ps(vx, v_dt, px));
        _mm256_storeu_ps(pos_y.as_mut_ptr().add(i), _mm256_fmadd_ps(vy, v_dt, py));
        _mm256_storeu_ps(pos_z.as_mut_ptr().add(i), _mm256_fmadd_ps(vz, v_dt, pz));
    }

    let tail = chunks * 8;
    for i in tail..n {
        let inv_m = 1.0 / mass[i];
        vel_x[i] = (vel_x[i] + force_x[i] * inv_m * dt) * damping;
        vel_y[i] = (vel_y[i] + force_y[i] * inv_m * dt) * damping;
        vel_z[i] = (vel_z[i] + force_z[i] * inv_m * dt) * damping;
        pos_x[i] += vel_x[i] * dt;
        pos_y[i] += vel_y[i] * dt;
        pos_z[i] += vel_z[i] * dt;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn integrate_positions_sse41(
    pos_x: &mut [f32],
    pos_y: &mut [f32],
    pos_z: &mut [f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    force_x: &[f32],
    force_y: &[f32],
    force_z: &[f32],
    mass: &[f32],
    dt: f32,
    damping: f32,
    n: usize,
) {
    use std::arch::x86_64::*;

    let v_dt = _mm_set1_ps(dt);
    let v_damp = _mm_set1_ps(damping);
    let v_one = _mm_set1_ps(1.0);

    let chunks = n / 4;
    for c in 0..chunks {
        let i = c * 4;

        let m = _mm_loadu_ps(mass.as_ptr().add(i));
        let inv_mass = _mm_div_ps(v_one, m);

        let fx = _mm_loadu_ps(force_x.as_ptr().add(i));
        let fy = _mm_loadu_ps(force_y.as_ptr().add(i));
        let fz = _mm_loadu_ps(force_z.as_ptr().add(i));

        let ax = _mm_mul_ps(fx, inv_mass);
        let ay = _mm_mul_ps(fy, inv_mass);
        let az = _mm_mul_ps(fz, inv_mass);

        let mut vx = _mm_loadu_ps(vel_x.as_ptr().add(i));
        let mut vy = _mm_loadu_ps(vel_y.as_ptr().add(i));
        let mut vz = _mm_loadu_ps(vel_z.as_ptr().add(i));

        // vel = (vel + accel * dt) * damping
        vx = _mm_mul_ps(_mm_add_ps(vx, _mm_mul_ps(ax, v_dt)), v_damp);
        vy = _mm_mul_ps(_mm_add_ps(vy, _mm_mul_ps(ay, v_dt)), v_damp);
        vz = _mm_mul_ps(_mm_add_ps(vz, _mm_mul_ps(az, v_dt)), v_damp);

        _mm_storeu_ps(vel_x.as_mut_ptr().add(i), vx);
        _mm_storeu_ps(vel_y.as_mut_ptr().add(i), vy);
        _mm_storeu_ps(vel_z.as_mut_ptr().add(i), vz);

        let px = _mm_loadu_ps(pos_x.as_ptr().add(i));
        let py = _mm_loadu_ps(pos_y.as_ptr().add(i));
        let pz = _mm_loadu_ps(pos_z.as_ptr().add(i));

        _mm_storeu_ps(
            pos_x.as_mut_ptr().add(i),
            _mm_add_ps(px, _mm_mul_ps(vx, v_dt)),
        );
        _mm_storeu_ps(
            pos_y.as_mut_ptr().add(i),
            _mm_add_ps(py, _mm_mul_ps(vy, v_dt)),
        );
        _mm_storeu_ps(
            pos_z.as_mut_ptr().add(i),
            _mm_add_ps(pz, _mm_mul_ps(vz, v_dt)),
        );
    }

    let tail = chunks * 4;
    for i in tail..n {
        let inv_m = 1.0 / mass[i];
        vel_x[i] = (vel_x[i] + force_x[i] * inv_m * dt) * damping;
        vel_y[i] = (vel_y[i] + force_y[i] * inv_m * dt) * damping;
        vel_z[i] = (vel_z[i] + force_z[i] * inv_m * dt) * damping;
        pos_x[i] += vel_x[i] * dt;
        pos_y[i] += vel_y[i] * dt;
        pos_z[i] += vel_z[i] * dt;
    }
}

fn integrate_positions_scalar(
    pos_x: &mut [f32],
    pos_y: &mut [f32],
    pos_z: &mut [f32],
    vel_x: &mut [f32],
    vel_y: &mut [f32],
    vel_z: &mut [f32],
    force_x: &[f32],
    force_y: &[f32],
    force_z: &[f32],
    mass: &[f32],
    dt: f32,
    damping: f32,
    n: usize,
) {
    for i in 0..n {
        let inv_m = 1.0 / mass[i];
        vel_x[i] = (vel_x[i] + force_x[i] * inv_m * dt) * damping;
        vel_y[i] = (vel_y[i] + force_y[i] * inv_m * dt) * damping;
        vel_z[i] = (vel_z[i] + force_z[i] * inv_m * dt) * damping;
        pos_x[i] += vel_x[i] * dt;
        pos_y[i] += vel_y[i] * dt;
        pos_z[i] += vel_z[i] * dt;
    }
}

// ===========================================================================
// 4. Dot product (for similarity computations)
// ===========================================================================

/// SIMD-accelerated dot product of two f32 slices.
///
/// Returns `sum(a[i] * b[i])` for `i in 0..min(a.len(), b.len())`.
pub fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());

    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            return unsafe { dot_product_avx2(a, b, n) };
        }
        if has_sse41() {
            return unsafe { dot_product_sse41(a, b, n) };
        }
    }

    dot_product_scalar(a, b, n)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn dot_product_avx2(a: &[f32], b: &[f32], n: usize) -> f32 {
    use std::arch::x86_64::*;

    let mut acc = _mm256_setzero_ps();
    let chunks = n / 8;
    for c in 0..chunks {
        let i = c * 8;
        let va = _mm256_loadu_ps(a.as_ptr().add(i));
        let vb = _mm256_loadu_ps(b.as_ptr().add(i));
        acc = _mm256_fmadd_ps(va, vb, acc);
    }

    // Horizontal sum of 8 floats in acc
    // acc = [a0 a1 a2 a3 a4 a5 a6 a7]
    let hi128 = _mm256_extractf128_ps(acc, 1); // [a4 a5 a6 a7]
    let lo128 = _mm256_castps256_ps128(acc); // [a0 a1 a2 a3]
    let sum128 = _mm_add_ps(lo128, hi128); // [a0+a4 a1+a5 a2+a6 a3+a7]
    let shuf = _mm_movehdup_ps(sum128); // [a1+a5 a1+a5 a3+a7 a3+a7]
    let sums = _mm_add_ps(sum128, shuf); // [a0+a1+a4+a5 ... a2+a3+a6+a7 ...]
    let shuf2 = _mm_movehl_ps(sums, sums); // move high 64 to low
    let result = _mm_add_ss(sums, shuf2);
    let mut sum = _mm_cvtss_f32(result);

    let tail = chunks * 8;
    for i in tail..n {
        sum += a[i] * b[i];
    }
    sum
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn dot_product_sse41(a: &[f32], b: &[f32], n: usize) -> f32 {
    use std::arch::x86_64::*;

    let mut acc = _mm_setzero_ps();
    let chunks = n / 4;
    for c in 0..chunks {
        let i = c * 4;
        let va = _mm_loadu_ps(a.as_ptr().add(i));
        let vb = _mm_loadu_ps(b.as_ptr().add(i));
        // SSE4.1 dpps: dot product with mask 0xFF -> all 4 lanes participate,
        // result broadcast to all lanes
        acc = _mm_add_ps(acc, _mm_dp_ps(va, vb, 0xFF));
    }

    // All 4 lanes of acc hold the same partial sum from each dp call
    // but we accumulated, so just extract lane 0
    // Actually, _mm_dp_ps with 0xFF broadcasts to all, and we _mm_add_ps accumulated,
    // so lane 0 has the total.
    let mut sum = _mm_cvtss_f32(acc);

    let tail = chunks * 4;
    for i in tail..n {
        sum += a[i] * b[i];
    }
    sum
}

fn dot_product_scalar(a: &[f32], b: &[f32], n: usize) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..n {
        sum += a[i] * b[i];
    }
    sum
}

// ===========================================================================
// 5. Batch stress computation (for stress majorization hot path)
// ===========================================================================

/// Compute stress contributions for a batch of node pairs.
///
/// For each pair `i`: `stress += weight[i] * (ideal_dist[i] - actual_dist[i])^2`
///
/// Returns the total stress for this batch.
pub fn compute_stress_batch_simd(
    ideal_distances: &[f32],
    actual_distances: &[f32],
    weights: &[f32],
) -> f32 {
    let n = ideal_distances
        .len()
        .min(actual_distances.len())
        .min(weights.len());

    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            return unsafe {
                compute_stress_batch_avx2(ideal_distances, actual_distances, weights, n)
            };
        }
        if has_sse41() {
            return unsafe {
                compute_stress_batch_sse41(ideal_distances, actual_distances, weights, n)
            };
        }
    }

    compute_stress_batch_scalar(ideal_distances, actual_distances, weights, n)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn compute_stress_batch_avx2(
    ideal: &[f32],
    actual: &[f32],
    weights: &[f32],
    n: usize,
) -> f32 {
    use std::arch::x86_64::*;

    let mut acc = _mm256_setzero_ps();
    let chunks = n / 8;
    for c in 0..chunks {
        let i = c * 8;
        let vid = _mm256_loadu_ps(ideal.as_ptr().add(i));
        let vad = _mm256_loadu_ps(actual.as_ptr().add(i));
        let vw = _mm256_loadu_ps(weights.as_ptr().add(i));

        let diff = _mm256_sub_ps(vid, vad);
        let diff2 = _mm256_mul_ps(diff, diff);
        acc = _mm256_fmadd_ps(vw, diff2, acc);
    }

    // Horizontal sum
    let hi128 = _mm256_extractf128_ps(acc, 1);
    let lo128 = _mm256_castps256_ps128(acc);
    let sum128 = _mm_add_ps(lo128, hi128);
    let shuf = _mm_movehdup_ps(sum128);
    let sums = _mm_add_ps(sum128, shuf);
    let shuf2 = _mm_movehl_ps(sums, sums);
    let result = _mm_add_ss(sums, shuf2);
    let mut total = _mm_cvtss_f32(result);

    let tail = chunks * 8;
    for i in tail..n {
        let diff = ideal[i] - actual[i];
        total += weights[i] * diff * diff;
    }
    total
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn compute_stress_batch_sse41(
    ideal: &[f32],
    actual: &[f32],
    weights: &[f32],
    n: usize,
) -> f32 {
    use std::arch::x86_64::*;

    let mut acc = _mm_setzero_ps();
    let chunks = n / 4;
    for c in 0..chunks {
        let i = c * 4;
        let vid = _mm_loadu_ps(ideal.as_ptr().add(i));
        let vad = _mm_loadu_ps(actual.as_ptr().add(i));
        let vw = _mm_loadu_ps(weights.as_ptr().add(i));

        let diff = _mm_sub_ps(vid, vad);
        let diff2 = _mm_mul_ps(diff, diff);
        acc = _mm_add_ps(acc, _mm_mul_ps(vw, diff2));
    }

    // Horizontal sum of 4 floats
    let shuf = _mm_movehdup_ps(acc);
    let sums = _mm_add_ps(acc, shuf);
    let shuf2 = _mm_movehl_ps(sums, sums);
    let result = _mm_add_ss(sums, shuf2);
    let mut total = _mm_cvtss_f32(result);

    let tail = chunks * 4;
    for i in tail..n {
        let diff = ideal[i] - actual[i];
        total += weights[i] * diff * diff;
    }
    total
}

fn compute_stress_batch_scalar(ideal: &[f32], actual: &[f32], weights: &[f32], n: usize) -> f32 {
    let mut total = 0.0f32;
    for i in 0..n {
        let diff = ideal[i] - actual[i];
        total += weights[i] * diff * diff;
    }
    total
}

// ===========================================================================
// 6. Centroid accumulation (for kernel_bridge CPU fallback)
// ===========================================================================

/// SIMD-accelerated centroid finalization: `centroids[i] /= counts[i]` for each type.
///
/// Operates on interleaved (x,y,z) triples stored as separate component arrays.
pub fn finalize_centroids_simd(
    centroid_x: &mut [f32],
    centroid_y: &mut [f32],
    centroid_z: &mut [f32],
    counts: &[f32],
) {
    let n = centroid_x
        .len()
        .min(centroid_y.len())
        .min(centroid_z.len())
        .min(counts.len());

    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            unsafe {
                finalize_centroids_avx2(centroid_x, centroid_y, centroid_z, counts, n);
            }
            return;
        }
    }

    for i in 0..n {
        if counts[i] > 0.0 {
            let inv = 1.0 / counts[i];
            centroid_x[i] *= inv;
            centroid_y[i] *= inv;
            centroid_z[i] *= inv;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn finalize_centroids_avx2(
    centroid_x: &mut [f32],
    centroid_y: &mut [f32],
    centroid_z: &mut [f32],
    counts: &[f32],
    n: usize,
) {
    use std::arch::x86_64::*;

    let v_one = _mm256_set1_ps(1.0);
    let v_zero = _mm256_setzero_ps();

    let chunks = n / 8;
    for c in 0..chunks {
        let i = c * 8;

        let cnt = _mm256_loadu_ps(counts.as_ptr().add(i));
        // mask: counts > 0
        let mask = _mm256_cmp_ps(cnt, v_zero, _CMP_GT_OQ);
        let inv = _mm256_blendv_ps(v_one, _mm256_div_ps(v_one, cnt), mask);

        let cx = _mm256_loadu_ps(centroid_x.as_ptr().add(i));
        let cy = _mm256_loadu_ps(centroid_y.as_ptr().add(i));
        let cz = _mm256_loadu_ps(centroid_z.as_ptr().add(i));

        _mm256_storeu_ps(centroid_x.as_mut_ptr().add(i), _mm256_mul_ps(cx, inv));
        _mm256_storeu_ps(centroid_y.as_mut_ptr().add(i), _mm256_mul_ps(cy, inv));
        _mm256_storeu_ps(centroid_z.as_mut_ptr().add(i), _mm256_mul_ps(cz, inv));
    }

    let tail = chunks * 8;
    for i in tail..n {
        if counts[i] > 0.0 {
            let inv = 1.0 / counts[i];
            centroid_x[i] *= inv;
            centroid_y[i] *= inv;
            centroid_z[i] *= inv;
        }
    }
}

// ===========================================================================
// 7. Diagnostics
// ===========================================================================

/// Returns a string describing which SIMD instruction set will be used at runtime.
pub fn simd_feature_name() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            return "AVX2+FMA (256-bit, 8-wide f32)";
        }
        if has_sse41() {
            return "SSE4.1 (128-bit, 4-wide f32)";
        }
    }
    "Scalar (no SIMD)"
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    #[test]
    fn test_compute_distances_basic() {
        let px = [1.0, 4.0, 0.0];
        let py = [2.0, 5.0, 0.0];
        let pz = [3.0, 6.0, 0.0];
        let ox = [0.0, 0.0, 0.0];
        let oy = [0.0, 0.0, 0.0];
        let oz = [0.0, 0.0, 0.0];
        let mut dist = [0.0f32; 3];

        compute_distances_simd(&px, &py, &pz, &ox, &oy, &oz, &mut dist);

        // sqrt(1+4+9) = sqrt(14) ~ 3.7417
        assert!((dist[0] - 14.0f32.sqrt()).abs() < EPSILON);
        // sqrt(16+25+36) = sqrt(77) ~ 8.7749
        assert!((dist[1] - 77.0f32.sqrt()).abs() < EPSILON);
        assert!((dist[2] - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_compute_distances_simd_large() {
        // Test with enough elements to exercise AVX2 (8+) and SSE (4+) paths
        let n = 19; // 2 full AVX2 chunks + 3 remainder
        let px: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let py: Vec<f32> = (0..n).map(|i| (i * 2) as f32).collect();
        let pz: Vec<f32> = (0..n).map(|i| (i * 3) as f32).collect();
        let ox = vec![0.0f32; n];
        let oy = vec![0.0f32; n];
        let oz = vec![0.0f32; n];
        let mut dist = vec![0.0f32; n];

        compute_distances_simd(&px, &py, &pz, &ox, &oy, &oz, &mut dist);

        for i in 0..n {
            let expected = (px[i] * px[i] + py[i] * py[i] + pz[i] * pz[i]).sqrt();
            assert!(
                (dist[i] - expected).abs() < EPSILON,
                "mismatch at {}: got {} expected {}",
                i,
                dist[i],
                expected,
            );
        }
    }

    #[test]
    fn test_accumulate_forces() {
        let mut fx = [1.0, 2.0, 3.0, 4.0];
        let mut fy = [0.0; 4];
        let mut fz = [0.0; 4];
        let dx = [1.0, 1.0, 1.0, 1.0];
        let dy = [2.0, 2.0, 2.0, 2.0];
        let dz = [3.0, 3.0, 3.0, 3.0];
        let mag = [10.0, 10.0, 10.0, 10.0];

        accumulate_forces_simd(&mut fx, &mut fy, &mut fz, &dx, &dy, &dz, &mag);

        assert!((fx[0] - 11.0).abs() < EPSILON); // 1 + 1*10
        assert!((fy[0] - 20.0).abs() < EPSILON); // 0 + 2*10
        assert!((fz[0] - 30.0).abs() < EPSILON); // 0 + 3*10
    }

    #[test]
    fn test_integrate_positions() {
        let mut px = [0.0f32; 4];
        let mut py = [0.0f32; 4];
        let mut pz = [0.0f32; 4];
        let mut vx = [0.0f32; 4];
        let mut vy = [0.0f32; 4];
        let mut vz = [0.0f32; 4];
        let fx = [10.0f32; 4];
        let fy = [0.0f32; 4];
        let fz = [0.0f32; 4];
        let mass = [1.0f32; 4];

        integrate_positions_simd(
            &mut px, &mut py, &mut pz, &mut vx, &mut vy, &mut vz, &fx, &fy, &fz, &mass, 0.1, 0.99,
        );

        // vel_x = (0 + 10/1 * 0.1) * 0.99 = 0.99
        assert!((vx[0] - 0.99).abs() < EPSILON);
        // pos_x = 0 + 0.99 * 0.1 = 0.099
        assert!((px[0] - 0.099).abs() < EPSILON);
    }

    #[test]
    fn test_dot_product() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let b = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

        let result = dot_product_simd(&a, &b);
        assert!((result - 45.0).abs() < EPSILON); // sum 1..9 = 45
    }

    #[test]
    fn test_dot_product_large() {
        let n = 1024;
        let a: Vec<f32> = (0..n).map(|i| (i + 1) as f32).collect();
        let b = vec![1.0f32; n];

        let result = dot_product_simd(&a, &b);
        let expected = (n as f32) * (n as f32 + 1.0) / 2.0;
        assert!(
            (result - expected).abs() < 1.0,
            "got {} expected {}",
            result,
            expected,
        );
    }

    #[test]
    fn test_stress_batch() {
        let ideal = [5.0, 10.0, 3.0, 8.0];
        let actual = [4.0, 9.0, 2.0, 7.0];
        let weights = [1.0, 2.0, 0.5, 1.0];

        let stress = compute_stress_batch_simd(&ideal, &actual, &weights);
        // (5-4)^2*1 + (10-9)^2*2 + (3-2)^2*0.5 + (8-7)^2*1 = 1 + 2 + 0.5 + 1 = 4.5
        assert!((stress - 4.5).abs() < EPSILON);
    }

    #[test]
    fn test_simd_feature_name() {
        let name = simd_feature_name();
        assert!(!name.is_empty());
        // On x86_64 test machines, should report at least SSE4.1
        #[cfg(target_arch = "x86_64")]
        assert!(name.contains("SSE4.1") || name.contains("AVX2"));
    }

    #[test]
    fn test_finalize_centroids() {
        let mut cx = [10.0, 0.0, 30.0];
        let mut cy = [20.0, 0.0, 60.0];
        let mut cz = [30.0, 0.0, 90.0];
        let counts = [2.0, 0.0, 3.0];

        finalize_centroids_simd(&mut cx, &mut cy, &mut cz, &counts);

        assert!((cx[0] - 5.0).abs() < EPSILON);
        assert!((cy[0] - 10.0).abs() < EPSILON);
        assert!((cz[0] - 15.0).abs() < EPSILON);
        // Zero count should leave values unchanged (multiplied by 1.0)
        assert!((cx[1] - 0.0).abs() < EPSILON);
        assert!((cx[2] - 10.0).abs() < EPSILON);
    }
}
