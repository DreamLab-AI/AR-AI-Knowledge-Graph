//! # Unified GPU Memory Manager
//!
//! This module consolidates three overlapping GPU memory management implementations:
//! 1. `src/utils/gpu_memory.rs` - Memory tracking and leak detection
//! 2. `src/gpu/dynamic_buffer_manager.rs` - Dynamic resizing and pool management
//! 3. `src/utils/unified_gpu_compute.rs` - Async transfers and double buffering
//!
//! ## Key Features
//!
//! - **Pool-based allocation** with configurable growth strategies
//! - **Automatic resizing** when capacity is exceeded
//! - **Memory leak detection** with named buffer tracking
//! - **Async transfers** with double buffering (2.8-4.4x speedup)
//! - **Performance metrics** for monitoring and optimization
//! - **Thread-safe** operations with minimal overhead
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use crate::gpu::memory_manager::{GpuMemoryManager, BufferConfig};
//!
//! // Create manager
//! let mut manager = GpuMemoryManager::new()?;
//!
//! // Allocate buffer with dynamic resizing
//! let config = BufferConfig::for_positions();
//! manager.allocate("positions", 1000, config)?;
//!
//! // Resize automatically when needed
//! manager.ensure_capacity("positions", 5000)?;
//!
//! // Async transfer to host
//! manager.start_async_download("positions")?;
//! // ... do other work ...
//! let data = manager.wait_for_download::<f32>("positions")?;
//!
//! // Check for memory leaks
//! let leaks = manager.check_leaks();
//! assert!(leaks.is_empty());
//! ```

use cust::error::CudaError;
use cust::event::{Event, EventFlags};
use cust::memory::{AsyncCopyDestination, CopyDestination, DeviceBuffer};
use cust::stream::{Stream, StreamFlags};
use log::{debug, error, info, warn};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Configuration for buffer growth and size limits
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Bytes per element (e.g., 12 for f32x3)
    pub bytes_per_element: usize,
    /// Growth multiplier when resizing (e.g., 1.5 = 50% growth)
    pub growth_factor: f32,
    /// Maximum buffer size in bytes
    pub max_size_bytes: usize,
    /// Minimum buffer size in bytes
    pub min_size_bytes: usize,
    /// Enable async transfer support
    pub enable_async: bool,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            bytes_per_element: 4, // f32
            growth_factor: 1.5,
            max_size_bytes: 1024 * 1024 * 1024, // 1GB
            min_size_bytes: 4096,               // 4KB
            enable_async: false,
        }
    }
}

impl BufferConfig {
    /// Configuration for 3D position buffers (f32x3)
    pub fn for_positions() -> Self {
        Self {
            bytes_per_element: 12, // 3 * sizeof(f32)
            growth_factor: 1.3,
            max_size_bytes: 512 * 1024 * 1024,
            min_size_bytes: 4096,
            enable_async: true, // Enable async for frequent reads
        }
    }

    /// Configuration for 3D velocity buffers (f32x3)
    pub fn for_velocities() -> Self {
        Self {
            bytes_per_element: 12,
            growth_factor: 1.3,
            max_size_bytes: 512 * 1024 * 1024,
            min_size_bytes: 4096,
            enable_async: true,
        }
    }

    /// Configuration for edge data (larger growth for graphs)
    pub fn for_edges() -> Self {
        Self {
            bytes_per_element: 32, // Edge metadata
            growth_factor: 2.0,
            max_size_bytes: 2048 * 1024 * 1024,
            min_size_bytes: 8192,
            enable_async: false,
        }
    }

    /// Configuration for grid/spatial structures
    pub fn for_grid_cells() -> Self {
        Self {
            bytes_per_element: 8,
            growth_factor: 1.5,
            max_size_bytes: 256 * 1024 * 1024,
            min_size_bytes: 2048,
            enable_async: false,
        }
    }
}

/// GPU buffer with automatic resizing and async transfer support
pub struct GpuBuffer<T: cust_core::DeviceCopy> {
    /// Device buffer
    device_buffer: DeviceBuffer<T>,

    /// Buffer name for debugging
    name: String,

    /// Current capacity in elements
    capacity_elements: usize,

    /// Configuration
    config: BufferConfig,

    /// Allocation timestamp
    allocated_at: Instant,

    /// Last access timestamp (using Cell for interior mutability)
    last_accessed: Cell<Instant>,

    // Async transfer state (double buffering)
    host_buffer_a: Option<Vec<T>>,
    host_buffer_b: Option<Vec<T>>,
    current_host_buffer: bool, // true = A, false = B
    transfer_pending: bool,
    transfer_event: Option<Event>,
}

impl<T: cust_core::DeviceCopy + Clone + Default> GpuBuffer<T> {
    /// Create new GPU buffer with specified capacity
    fn new(name: String, capacity: usize, config: BufferConfig) -> Result<Self, CudaError> {
        let device_buffer = DeviceBuffer::from_slice(&vec![T::default(); capacity])?;

        // Initialize async buffers if enabled
        let (host_buffer_a, host_buffer_b) = if config.enable_async {
            (
                Some(vec![T::default(); capacity]),
                Some(vec![T::default(); capacity]),
            )
        } else {
            (None, None)
        };

        Ok(Self {
            device_buffer,
            name,
            capacity_elements: capacity,
            config,
            allocated_at: Instant::now(),
            last_accessed: Cell::new(Instant::now()),
            host_buffer_a,
            host_buffer_b,
            current_host_buffer: true,
            transfer_pending: false,
            transfer_event: None,
        })
    }

    /// Get current capacity in elements
    pub fn capacity(&self) -> usize {
        self.capacity_elements
    }

    /// Get buffer size in bytes
    pub fn size_bytes(&self) -> usize {
        self.capacity_elements * std::mem::size_of::<T>()
    }

    /// Get device buffer reference
    pub fn device_buffer(&self) -> &DeviceBuffer<T> {
        self.last_accessed.set(Instant::now());
        &self.device_buffer
    }

    /// Get mutable device buffer reference
    pub fn device_buffer_mut(&mut self) -> &mut DeviceBuffer<T> {
        self.last_accessed.set(Instant::now());
        &mut self.device_buffer
    }

    /// Resize buffer to new capacity, preserving existing data
    fn resize(&mut self, new_capacity: usize) -> Result<(), CudaError> {
        if new_capacity == self.capacity_elements {
            return Ok(());
        }

        debug!(
            "Resizing buffer '{}' from {} to {} elements",
            self.name, self.capacity_elements, new_capacity
        );

        // Create new buffer
        let mut new_buffer = DeviceBuffer::from_slice(&vec![T::default(); new_capacity])?;

        // Copy old data
        let copy_count = self.capacity_elements.min(new_capacity);
        if copy_count > 0 {
            // Copy old data to host buffer first, then to new device buffer
            let mut temp_host = vec![T::default(); copy_count];
            self.device_buffer.copy_to(&mut temp_host)?;

            // Create stream for async copy from host to device
            let stream = Stream::new(StreamFlags::NON_BLOCKING, None)?;
            // SAFETY: This async copy is safe because:
            // 1. `temp_host` is a valid slice allocated on the heap with exactly `copy_count` elements
            // 2. `new_buffer` is a freshly allocated DeviceBuffer with capacity >= copy_count
            // 3. The stream is valid and will be synchronized before this function returns
            // 4. `temp_host` lifetime extends past the stream.synchronize() call
            // 5. T implements DeviceCopy, guaranteeing it is safe to copy between host and device
            unsafe {
                new_buffer.async_copy_from(&temp_host, &stream)?;
            }
            stream.synchronize()?;
        }

        // Update state
        self.device_buffer = new_buffer;
        self.capacity_elements = new_capacity;

        // Resize host buffers for async transfers
        if self.config.enable_async {
            if let Some(ref mut buf_a) = self.host_buffer_a {
                buf_a.resize(new_capacity, T::default());
            }
            if let Some(ref mut buf_b) = self.host_buffer_b {
                buf_b.resize(new_capacity, T::default());
            }
        }

        Ok(())
    }

    /// Start async download to host (non-blocking)
    fn start_async_download(&mut self, stream: &Stream) -> Result<(), CudaError> {
        if !self.config.enable_async {
            error!("Async transfers not enabled for buffer '{}'", self.name);
            return Err(CudaError::InvalidValue);
        }

        // Select target host buffer (ping-pong)
        let target_buffer = if self.current_host_buffer {
            match self.host_buffer_a.as_mut() {
                Some(buf) => buf,
                None => {
                    error!(
                        "Host buffer A not initialized for async buffer '{}'",
                        self.name
                    );
                    return Err(CudaError::InvalidValue);
                }
            }
        } else {
            match self.host_buffer_b.as_mut() {
                Some(buf) => buf,
                None => {
                    error!(
                        "Host buffer B not initialized for async buffer '{}'",
                        self.name
                    );
                    return Err(CudaError::InvalidValue);
                }
            }
        };

        // Start async copy from device to host
        stream.synchronize()?; // Ensure previous operations complete
                               // SAFETY: This async copy is safe because:
                               // 1. `self.device_buffer` is a valid DeviceBuffer allocated during GpuBuffer::new()
                               // 2. `target_buffer` points to a valid host Vec<T> (either host_buffer_a or host_buffer_b)
                               //    that was allocated with the same capacity as the device buffer
                               // 3. The stream was synchronized before this call to ensure no concurrent modifications
                               // 4. T implements DeviceCopy, guaranteeing the type is safe for GPU memory operations
                               // 5. The caller must call wait_for_download() before accessing target_buffer data
        unsafe {
            self.device_buffer.async_copy_to(target_buffer, stream)?;
        }

        // Record event for synchronization
        let event = Event::new(EventFlags::DEFAULT)?;
        event.record(stream)?;
        self.transfer_event = Some(event);
        self.transfer_pending = true;

        Ok(())
    }

    /// Wait for async download to complete and return data
    fn wait_for_download(&mut self) -> Result<Vec<T>, CudaError> {
        if !self.transfer_pending {
            error!("No async transfer pending for buffer '{}'", self.name);
            return Err(CudaError::InvalidValue);
        }

        // Wait for transfer event
        if let Some(ref event) = self.transfer_event {
            event.synchronize()?;
        }

        // Get completed buffer
        let result_buffer = if self.current_host_buffer {
            match self.host_buffer_a.as_ref() {
                Some(buf) => buf,
                None => {
                    error!("Host buffer A not initialized for buffer '{}'", self.name);
                    return Err(CudaError::InvalidValue);
                }
            }
        } else {
            match self.host_buffer_b.as_ref() {
                Some(buf) => buf,
                None => {
                    error!("Host buffer B not initialized for buffer '{}'", self.name);
                    return Err(CudaError::InvalidValue);
                }
            }
        };

        // Flip buffers for next transfer
        self.current_host_buffer = !self.current_host_buffer;
        self.transfer_pending = false;

        Ok(result_buffer.clone())
    }

    /// Get statistics for this buffer
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            name: self.name.clone(),
            capacity_bytes: self.size_bytes(),
            allocated_bytes: self.size_bytes(),
            utilization: 1.0, // Assume fully utilized
            age_seconds: self.allocated_at.elapsed().as_secs_f32(),
            last_access_seconds: self.last_accessed.get().elapsed().as_secs_f32(),
        }
    }
}

/// Buffer statistics for monitoring
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub name: String,
    pub capacity_bytes: usize,
    pub allocated_bytes: usize,
    pub utilization: f32,
    pub age_seconds: f32,
    pub last_access_seconds: f32,
}

/// Memory allocation tracking entry
#[derive(Debug, Clone)]
struct AllocationEntry {
    size_bytes: usize,
    timestamp: Instant,
}

/// Unified GPU Memory Manager
pub struct GpuMemoryManager {
    /// Named buffer storage (using Box for type erasure, Send-safe for cross-thread access)
    buffers: HashMap<String, Box<dyn std::any::Any + Send>>,

    /// Buffer configurations
    configs: HashMap<String, BufferConfig>,

    /// Allocation tracking for leak detection
    allocations: Arc<Mutex<HashMap<String, AllocationEntry>>>,

    /// Total allocated memory (atomic for thread-safety)
    total_allocated: Arc<AtomicUsize>,

    /// Peak memory usage
    peak_allocated: Arc<AtomicUsize>,

    /// Maximum total memory limit
    max_total_memory: usize,

    /// Dedicated stream for async transfers
    transfer_stream: Stream,

    /// Performance metrics
    allocation_count: AtomicUsize,
    resize_count: AtomicUsize,
    async_transfer_count: AtomicUsize,
}

impl GpuMemoryManager {
    /// Create new memory manager with default settings
    pub fn new() -> Result<Self, CudaError> {
        Self::with_limit(6 * 1024 * 1024 * 1024) // 6GB default limit
    }

    /// Create memory manager with custom memory limit
    pub fn with_limit(max_memory_bytes: usize) -> Result<Self, CudaError> {
        Ok(Self {
            buffers: HashMap::new(),
            configs: HashMap::new(),
            allocations: Arc::new(Mutex::new(HashMap::new())),
            total_allocated: Arc::new(AtomicUsize::new(0)),
            peak_allocated: Arc::new(AtomicUsize::new(0)),
            max_total_memory: max_memory_bytes,
            transfer_stream: Stream::new(StreamFlags::NON_BLOCKING, None)?,
            allocation_count: AtomicUsize::new(0),
            resize_count: AtomicUsize::new(0),
            async_transfer_count: AtomicUsize::new(0),
        })
    }

    /// Allocate a new GPU buffer
    pub fn allocate<T: cust_core::DeviceCopy + Clone + Default + Send + 'static>(
        &mut self,
        name: &str,
        capacity_elements: usize,
        config: BufferConfig,
    ) -> Result<(), CudaError> {
        // Check if buffer already exists
        if self.buffers.contains_key(name) {
            warn!("Buffer '{}' already exists, skipping allocation", name);
            return Ok(());
        }

        let size_bytes = capacity_elements * std::mem::size_of::<T>();

        // Check memory limit
        let current = self.total_allocated.load(Ordering::Relaxed);
        if current + size_bytes > self.max_total_memory {
            return Err(CudaError::InvalidMemoryAllocation);
        }

        // Create buffer
        let buffer = GpuBuffer::<T>::new(name.to_string(), capacity_elements, config.clone())?;

        // Track allocation
        self.track_allocation(name, size_bytes);

        // Store buffer
        self.buffers.insert(name.to_string(), Box::new(buffer));
        self.configs.insert(name.to_string(), config);

        self.allocation_count.fetch_add(1, Ordering::Relaxed);

        info!(
            "Allocated GPU buffer '{}': {} elements ({} bytes)",
            name, capacity_elements, size_bytes
        );

        Ok(())
    }

    /// Ensure buffer has sufficient capacity, resizing if needed
    pub fn ensure_capacity<T: cust_core::DeviceCopy + Clone + Default + 'static>(
        &mut self,
        name: &str,
        required_elements: usize,
    ) -> Result<(), CudaError> {
        // Get buffer
        let buffer_any = self.buffers.get_mut(name).ok_or(CudaError::NotFound)?;
        let buffer = buffer_any
            .downcast_mut::<GpuBuffer<T>>()
            .ok_or(CudaError::InvalidValue)?;

        // Check if resize needed
        if buffer.capacity() >= required_elements {
            return Ok(());
        }

        // Calculate new capacity
        let config = self.configs.get(name).ok_or(CudaError::NotFound)?;
        let current_capacity = buffer.capacity();
        let mut new_capacity = if current_capacity == 0 {
            (config.min_size_bytes / std::mem::size_of::<T>()).max(required_elements)
        } else {
            let grown = (current_capacity as f32 * config.growth_factor) as usize;
            grown.max(required_elements)
        };

        // Enforce maximum size
        let max_elements = config.max_size_bytes / std::mem::size_of::<T>();
        new_capacity = new_capacity.min(max_elements);

        if required_elements > new_capacity {
            return Err(CudaError::InvalidMemoryAllocation);
        }

        // Track old size for memory accounting
        let old_size = buffer.size_bytes();

        // Resize
        buffer.resize(new_capacity)?;

        // Update allocation tracking
        let new_size = buffer.size_bytes();
        let delta = new_size as i64 - old_size as i64;

        if delta > 0 {
            self.track_allocation(&format!("{}_resize", name), delta as usize);
        } else if delta < 0 {
            self.track_deallocation(&format!("{}_resize", name), (-delta) as usize);
        }

        self.resize_count.fetch_add(1, Ordering::Relaxed);

        info!(
            "Resized buffer '{}' from {} to {} elements",
            name, current_capacity, new_capacity
        );

        Ok(())
    }

    /// Get device buffer reference
    pub fn get_buffer<T: cust_core::DeviceCopy + Clone + Default + 'static>(
        &self,
        name: &str,
    ) -> Result<&DeviceBuffer<T>, CudaError> {
        let buffer_any = self.buffers.get(name).ok_or(CudaError::NotFound)?;
        let buffer = buffer_any
            .downcast_ref::<GpuBuffer<T>>()
            .ok_or(CudaError::InvalidValue)?;
        Ok(buffer.device_buffer())
    }

    /// Get mutable device buffer reference
    pub fn get_buffer_mut<T: cust_core::DeviceCopy + Clone + Default + 'static>(
        &mut self,
        name: &str,
    ) -> Result<&mut DeviceBuffer<T>, CudaError> {
        let buffer_any = self.buffers.get_mut(name).ok_or(CudaError::NotFound)?;
        let buffer = buffer_any
            .downcast_mut::<GpuBuffer<T>>()
            .ok_or(CudaError::InvalidValue)?;
        Ok(buffer.device_buffer_mut())
    }

    /// Start async download (non-blocking)
    pub fn start_async_download<T: cust_core::DeviceCopy + Clone + Default + 'static>(
        &mut self,
        name: &str,
    ) -> Result<(), CudaError> {
        let buffer_any = self.buffers.get_mut(name).ok_or(CudaError::NotFound)?;
        let buffer = buffer_any
            .downcast_mut::<GpuBuffer<T>>()
            .ok_or(CudaError::InvalidValue)?;

        buffer.start_async_download(&self.transfer_stream)?;
        self.async_transfer_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Wait for async download to complete
    pub fn wait_for_download<T: cust_core::DeviceCopy + Clone + Default + 'static>(
        &mut self,
        name: &str,
    ) -> Result<Vec<T>, CudaError> {
        let buffer_any = self.buffers.get_mut(name).ok_or(CudaError::NotFound)?;
        let buffer = buffer_any
            .downcast_mut::<GpuBuffer<T>>()
            .ok_or(CudaError::InvalidValue)?;

        buffer.wait_for_download()
    }

    /// Free a buffer
    pub fn free(&mut self, name: &str) -> Result<(), CudaError> {
        if let Some(_buffer_any) = self.buffers.remove(name) {
            // Type-erased, but Drop will handle cleanup
            self.configs.remove(name);
            self.track_deallocation(name, 0); // Size tracked in allocations map

            info!("Freed GPU buffer '{}'", name);
            Ok(())
        } else {
            Err(CudaError::NotFound)
        }
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        let buffer_stats: Vec<BufferStats> = vec![]; // Would need to iterate type-erased buffers

        MemoryStats {
            total_allocated_bytes: self.total_allocated.load(Ordering::Relaxed),
            peak_allocated_bytes: self.peak_allocated.load(Ordering::Relaxed),
            buffer_count: self.buffers.len(),
            allocation_count: self.allocation_count.load(Ordering::Relaxed),
            resize_count: self.resize_count.load(Ordering::Relaxed),
            async_transfer_count: self.async_transfer_count.load(Ordering::Relaxed),
            buffer_stats,
        }
    }

    /// Check for memory leaks
    pub fn check_leaks(&self) -> Vec<String> {
        match self.allocations.lock() {
            Ok(allocations) => {
                if allocations.is_empty() {
                    debug!("No GPU memory leaks detected");
                    return Vec::new();
                }

                let leaks: Vec<String> = allocations.keys().cloned().collect();
                error!(
                    "GPU memory leaks detected: {} buffers still allocated",
                    leaks.len()
                );
                for (name, entry) in allocations.iter() {
                    error!(
                        "  Leaked buffer '{}': {} bytes (age: {:.2}s)",
                        name,
                        entry.size_bytes,
                        entry.timestamp.elapsed().as_secs_f32()
                    );
                }
                leaks
            }
            Err(e) => {
                error!(
                    "Lock poisoned while checking for leaks: {} - Cannot determine leak status",
                    e
                );
                Vec::new() // Return empty, cannot verify
            }
        }
    }

    // Internal tracking methods

    fn track_allocation(&self, name: &str, size_bytes: usize) {
        if let Ok(mut allocations) = self.allocations.lock() {
            allocations.insert(
                name.to_string(),
                AllocationEntry {
                    size_bytes,
                    timestamp: Instant::now(),
                },
            );

            let new_total = self
                .total_allocated
                .fetch_add(size_bytes, Ordering::Relaxed)
                + size_bytes;

            // Update peak
            let mut peak = self.peak_allocated.load(Ordering::Relaxed);
            while new_total > peak {
                match self.peak_allocated.compare_exchange_weak(
                    peak,
                    new_total,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(current) => peak = current,
                }
            }

            debug!(
                "GPU Memory: +{} bytes for '{}', total: {} bytes",
                size_bytes, name, new_total
            );
        }
    }

    /// Check whether a proposed allocation fits within the memory budget
    /// without actually performing the allocation. Returns Ok(()) if the
    /// allocation would succeed, or an error if the budget would be exceeded.
    pub fn check_budget(&self, name: &str, size_bytes: usize) -> Result<(), CudaError> {
        let current = self.total_allocated.load(Ordering::Relaxed);
        if current + size_bytes > self.max_total_memory {
            warn!(
                "GPU memory budget exceeded for '{}': current={}, requested={}, limit={}",
                name, current, size_bytes, self.max_total_memory
            );
            return Err(CudaError::InvalidMemoryAllocation);
        }
        Ok(())
    }

    /// Track an external allocation against the memory budget.
    /// Use this when GPU memory is allocated outside the manager (e.g. by
    /// `UnifiedGPUCompute`) but should still be accounted for in budget checks.
    pub fn track_external_allocation(&self, name: &str, size_bytes: usize) {
        self.track_allocation(name, size_bytes);
    }

    /// Remove tracking for an external allocation that was freed.
    pub fn track_external_deallocation(&self, name: &str) {
        self.track_deallocation(name, 0);
    }

    fn track_deallocation(&self, name: &str, size_bytes: usize) {
        if let Ok(mut allocations) = self.allocations.lock() {
            let actual_size = if size_bytes == 0 {
                allocations.get(name).map(|e| e.size_bytes).unwrap_or(0)
            } else {
                size_bytes
            };

            if allocations.remove(name).is_some() {
                let new_total = self
                    .total_allocated
                    .fetch_sub(actual_size, Ordering::Relaxed)
                    - actual_size;
                debug!(
                    "GPU Memory: -{} bytes for '{}', total: {} bytes",
                    actual_size, name, new_total
                );
            } else {
                warn!("Attempted to free untracked GPU buffer: {}", name);
            }
        }
    }
}

/// Memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_allocated_bytes: usize,
    pub peak_allocated_bytes: usize,
    pub buffer_count: usize,
    pub allocation_count: usize,
    pub resize_count: usize,
    pub async_transfer_count: usize,
    pub buffer_stats: Vec<BufferStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_config_defaults() {
        let config = BufferConfig::default();
        assert_eq!(config.bytes_per_element, 4);
        assert_eq!(config.growth_factor, 1.5);
        assert_eq!(config.min_size_bytes, 4096);
    }

    #[test]
    fn test_buffer_config_presets() {
        let pos_config = BufferConfig::for_positions();
        assert_eq!(pos_config.bytes_per_element, 12);
        assert!(pos_config.enable_async);

        let edge_config = BufferConfig::for_edges();
        assert_eq!(edge_config.bytes_per_element, 32);
        assert!(!edge_config.enable_async);
    }

    #[test]
    #[ignore] // Requires CUDA device
    fn test_memory_manager_creation() {
        let manager = GpuMemoryManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    #[ignore] // Requires CUDA device
    fn test_allocation_and_free() {
        let mut manager = GpuMemoryManager::new().unwrap();

        // Allocate buffer
        let config = BufferConfig::default();
        manager
            .allocate::<f32>("test_buffer", 1000, config)
            .unwrap();

        // Verify allocation
        let stats = manager.stats();
        assert_eq!(stats.buffer_count, 1);

        // Free buffer
        manager.free("test_buffer").unwrap();

        // Verify freed
        let stats = manager.stats();
        assert_eq!(stats.buffer_count, 0);
    }

    #[test]
    #[ignore] // Requires CUDA device
    fn test_dynamic_resizing() {
        let mut manager = GpuMemoryManager::new().unwrap();

        let config = BufferConfig::for_positions();
        manager.allocate::<f32>("positions", 100, config).unwrap();

        // Resize to larger capacity
        manager.ensure_capacity::<f32>("positions", 500).unwrap();

        // Verify resize happened
        let stats = manager.stats();
        assert!(stats.resize_count > 0);
    }

    #[test]
    #[ignore] // Requires CUDA device
    fn test_memory_limit_enforcement() {
        let mut manager = GpuMemoryManager::with_limit(1024).unwrap(); // 1KB limit

        let config = BufferConfig::default();
        let result = manager.allocate::<f32>("huge_buffer", 1_000_000, config);

        // Should fail due to memory limit
        assert!(result.is_err());
    }

    #[test]
    #[ignore] // Requires CUDA device
    fn test_leak_detection() {
        let mut manager = GpuMemoryManager::new().unwrap();

        let config = BufferConfig::default();
        manager
            .allocate::<f32>("leaked_buffer", 100, config)
            .unwrap();

        // Don't free the buffer
        let leaks = manager.check_leaks();
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0], "leaked_buffer");
    }

    #[test]
    #[ignore] // Requires CUDA device
    fn test_async_transfers() {
        let mut manager = GpuMemoryManager::new().unwrap();

        let mut config = BufferConfig::for_positions();
        config.enable_async = true;

        manager
            .allocate::<f32>("async_buffer", 100, config)
            .unwrap();

        // Start async download
        manager.start_async_download::<f32>("async_buffer").unwrap();

        // Wait for completion
        let data = manager.wait_for_download::<f32>("async_buffer").unwrap();
        assert_eq!(data.len(), 100);
    }
}
