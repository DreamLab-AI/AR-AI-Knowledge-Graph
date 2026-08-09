//! GPU Physics Broadcast Optimization
//!
//! Reduces network bandwidth through:
//! - Adaptive broadcast frequency (broadcast rate below the physics tick rate)
//! - Spatial partitioning (visibility culling)
//!
//! The broadcast is FULL-SNAPSHOT ONLY — every broadcast sends complete target
//! positions and clients tween to them. Delta encoding is prohibited by design
//! (docs/KNOWN_ISSUES.md BROADCAST-001, PRD-007 §3, ADR-061); there is no
//! delta/diff path here. Indices returned by the optimizer are visibility-culled,
//! never delta-filtered.

use glam::Vec3;
use log::{debug, info};
use std::time::{Duration, Instant};

/// Configuration for broadcast optimization
#[derive(Debug, Clone)]
pub struct BroadcastConfig {
    /// Target broadcast rate in Hz (below the physics tick rate)
    pub target_fps: u32,

    /// Enable spatial visibility culling
    pub enable_spatial_culling: bool,

    /// Camera frustum bounds for culling (min, max)
    pub camera_bounds: Option<(Vec3, Vec3)>,
}

impl Default for BroadcastConfig {
    fn default() -> Self {
        Self {
            target_fps: 25, // 25fps broadcast, 60fps physics
            enable_spatial_culling: false,
            camera_bounds: None,
        }
    }
}

/// Rate limiter that gates broadcasts to the target frequency.
///
/// This purely controls broadcast *timing* — it decides on which frames a
/// full snapshot should be emitted. It does not track or diff positions;
/// the broadcast is always a full snapshot (BROADCAST-001).
pub struct BroadcastRateLimiter {
    last_broadcast_time: Instant,
    broadcast_interval: Duration,
    frames_since_broadcast: u32,
}

impl BroadcastRateLimiter {
    pub fn new(config: &BroadcastConfig) -> Self {
        let broadcast_interval = Duration::from_micros((1_000_000 / config.target_fps) as u64);

        Self {
            last_broadcast_time: Instant::now(),
            broadcast_interval,
            frames_since_broadcast: 0,
        }
    }

    /// Check if we should broadcast this frame
    pub fn should_broadcast(&mut self) -> bool {
        self.frames_since_broadcast += 1;
        let elapsed = self.last_broadcast_time.elapsed();

        if elapsed >= self.broadcast_interval {
            self.last_broadcast_time = Instant::now();
            self.frames_since_broadcast = 0;
            true
        } else {
            false
        }
    }

    /// Get rate-limiter statistics
    pub fn get_stats(&self, total_nodes: usize, sent_nodes: usize) -> CompressionStats {
        let reduction_percent = if total_nodes > 0 {
            ((total_nodes - sent_nodes) as f32 / total_nodes as f32) * 100.0
        } else {
            0.0
        };

        CompressionStats {
            total_nodes,
            sent_nodes,
            reduction_percent,
            frames_since_broadcast: self.frames_since_broadcast,
        }
    }
}

/// Statistics for broadcast rate-limiting / culling performance
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub total_nodes: usize,
    pub sent_nodes: usize,
    pub reduction_percent: f32,
    pub frames_since_broadcast: u32,
}

/// Spatial partitioning for visibility culling
pub struct SpatialCuller {
    enabled: bool,
    camera_bounds: Option<(Vec3, Vec3)>,
}

impl SpatialCuller {
    pub fn new(config: &BroadcastConfig) -> Self {
        Self {
            enabled: config.enable_spatial_culling,
            camera_bounds: config.camera_bounds,
        }
    }

    /// Update camera frustum bounds
    pub fn update_camera_bounds(&mut self, min: Vec3, max: Vec3) {
        self.camera_bounds = Some((min, max));
    }

    /// Filter positions to only visible nodes
    pub fn filter_visible(&self, positions: &[Vec3], node_ids: &[u32]) -> Vec<usize> {
        if !self.enabled {
            // Return all indices if culling disabled
            return (0..node_ids.len()).collect();
        }

        let Some((min, max)) = self.camera_bounds else {
            // No bounds set, return all
            return (0..node_ids.len()).collect();
        };

        let mut visible_indices = Vec::new();

        for (idx, pos) in positions.iter().enumerate() {
            // Simple AABB test
            if pos.x >= min.x
                && pos.x <= max.x
                && pos.y >= min.y
                && pos.y <= max.y
                && pos.z >= min.z
                && pos.z <= max.z
            {
                visible_indices.push(idx);
            }
        }

        visible_indices
    }
}

/// Main broadcast optimizer combining all techniques
pub struct BroadcastOptimizer {
    config: BroadcastConfig,
    rate_limiter: BroadcastRateLimiter,
    spatial_culler: SpatialCuller,
    total_frames_processed: u64,
    total_nodes_sent: u64,
    total_nodes_processed: u64,
}

impl BroadcastOptimizer {
    pub fn new(config: BroadcastConfig) -> Self {
        let rate_limiter = BroadcastRateLimiter::new(&config);
        let spatial_culler = SpatialCuller::new(&config);

        Self {
            config,
            rate_limiter,
            spatial_culler,
            total_frames_processed: 0,
            total_nodes_sent: 0,
            total_nodes_processed: 0,
        }
    }

    /// Process positions and return indices of nodes to broadcast.
    ///
    /// Returns `(should_broadcast, visible_indices)`. This is a full-snapshot
    /// broadcast; `visible_indices` are visibility-culled, never delta-filtered.
    /// When spatial culling is disabled (the default) `visible_indices` contains
    /// every node index, i.e. the complete snapshot.
    pub fn process_frame(
        &mut self,
        positions: &[(Vec3, Vec3)], // (position, velocity)
        node_ids: &[u32],
    ) -> (bool, Vec<usize>) {
        self.total_frames_processed += 1;

        // Rate-limit: only broadcast on frames within the target frequency.
        if !self.rate_limiter.should_broadcast() {
            return (false, Vec::new());
        }

        // Apply spatial culling to determine which nodes are visible. When
        // culling is disabled this returns all indices (full snapshot).
        let visible_indices = if self.spatial_culler.enabled {
            let pos_only: Vec<Vec3> = positions.iter().map(|(p, _)| *p).collect();
            self.spatial_culler.filter_visible(&pos_only, node_ids)
        } else {
            (0..node_ids.len()).collect()
        };

        self.total_nodes_sent += visible_indices.len() as u64;
        self.total_nodes_processed += node_ids.len() as u64;

        (true, visible_indices)
    }

    /// Get overall performance statistics
    pub fn get_performance_stats(&self) -> BroadcastPerformanceStats {
        let avg_reduction = if self.total_nodes_processed > 0 {
            ((self.total_nodes_processed - self.total_nodes_sent) as f64
                / self.total_nodes_processed as f64)
                * 100.0
        } else {
            0.0
        };

        BroadcastPerformanceStats {
            total_frames_processed: self.total_frames_processed,
            total_nodes_sent: self.total_nodes_sent,
            total_nodes_processed: self.total_nodes_processed,
            average_bandwidth_reduction: avg_reduction as f32,
            target_fps: self.config.target_fps,
        }
    }

    /// Update configuration at runtime
    pub fn update_config(&mut self, config: BroadcastConfig) {
        info!("BroadcastOptimizer: Updating configuration");
        info!(
            "  Target FPS: {} -> {}",
            self.config.target_fps, config.target_fps
        );

        self.config = config;
        self.rate_limiter = BroadcastRateLimiter::new(&self.config);
        self.spatial_culler = SpatialCuller::new(&self.config);
    }

    /// Update camera bounds for spatial culling
    pub fn update_camera_bounds(&mut self, min: Vec3, max: Vec3) {
        self.spatial_culler.update_camera_bounds(min, max);
        debug!(
            "BroadcastOptimizer: Camera bounds updated to [{:?}, {:?}]",
            min, max
        );
    }

    /// Reset the broadcast rate-limit timer so the next frame broadcasts
    /// immediately. Call this when simulation parameters change or a new
    /// client connects, so the next full snapshot is emitted without waiting
    /// for the rate-limit interval.
    pub fn reset_broadcast_timer(&mut self) {
        info!("BroadcastOptimizer: Resetting broadcast timer — next frame will broadcast a full snapshot");
        self.rate_limiter = BroadcastRateLimiter::new(&self.config);
    }
}

#[derive(Debug, Clone)]
pub struct BroadcastPerformanceStats {
    pub total_frames_processed: u64,
    pub total_nodes_sent: u64,
    pub total_nodes_processed: u64,
    pub average_bandwidth_reduction: f32,
    pub target_fps: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_culling() {
        let config = BroadcastConfig {
            target_fps: 30,
            enable_spatial_culling: true,
            camera_bounds: Some((Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0))),
        };

        let culler = SpatialCuller::new(&config);

        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),  // Inside
            Vec3::new(15.0, 0.0, 0.0), // Outside
            Vec3::new(5.0, 5.0, 5.0),  // Inside
            Vec3::new(0.0, 20.0, 0.0), // Outside
        ];
        let node_ids = vec![0, 1, 2, 3];

        let visible = culler.filter_visible(&positions, &node_ids);
        assert_eq!(visible.len(), 2, "Only 2 nodes should be visible");
        assert!(visible.contains(&0));
        assert!(visible.contains(&2));
    }

    #[test]
    fn test_broadcast_optimizer_integration() {
        let config = BroadcastConfig {
            target_fps: 60, // High rate for testing
            enable_spatial_culling: false,
            camera_bounds: None,
        };

        let mut optimizer = BroadcastOptimizer::new(config);

        // Simulate multiple frames
        let positions = vec![
            (Vec3::new(0.0, 0.0, 0.0), Vec3::ZERO),
            (Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO),
        ];
        let node_ids = vec![0, 1];

        // First frame after the interval elapses should broadcast the full snapshot.
        std::thread::sleep(Duration::from_millis(20));
        let (should_broadcast, indices) = optimizer.process_frame(&positions, &node_ids);
        assert!(should_broadcast, "Frame after interval should broadcast");
        assert_eq!(indices.len(), 2, "Full snapshot: all node indices returned");

        // A frame taken immediately afterwards is inside the rate-limit interval
        // and must be gated out (no broadcast, no indices).
        let (should_broadcast, indices) = optimizer.process_frame(&positions, &node_ids);
        assert!(
            !should_broadcast,
            "Frame inside interval should be rate-limited"
        );
        assert!(indices.is_empty(), "Rate-limited frame returns no indices");

        // After the interval elapses again the full snapshot is emitted — every
        // node, never a delta-filtered subset.
        std::thread::sleep(Duration::from_millis(20));
        let (should_broadcast, indices) = optimizer.process_frame(&positions, &node_ids);
        assert!(
            should_broadcast,
            "Frame after interval should broadcast again"
        );
        assert_eq!(indices.len(), 2, "Full snapshot always returns all nodes");
    }
}
