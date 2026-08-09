// src/actors/lifecycle.rs
//! Actor Lifecycle Management
//!
//! Manages the lifecycle of Actix actors including startup, shutdown,
//! health monitoring, and supervision strategies.

use actix::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::actors::physics_orchestrator_actor::PhysicsOrchestratorActor;
use crate::actors::semantic_processor_actor::SemanticProcessorActor;
use crate::models::simulation_params::SimulationParams;

pub struct ActorLifecycleManager {
    physics_actor: Option<Addr<PhysicsOrchestratorActor>>,
    semantic_actor: Option<Addr<SemanticProcessorActor>>,
    health_check_interval: Duration,
}

impl Default for ActorLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorLifecycleManager {
    pub fn new() -> Self {
        Self {
            physics_actor: None,
            semantic_actor: None,
            health_check_interval: Duration::from_secs(30),
        }
    }

    pub async fn initialize(&mut self) -> Result<(), ActorLifecycleError> {
        info!("Initializing actor system");

        self.start_physics_actor().await?;

        self.start_semantic_actor().await?;

        self.start_health_monitoring();

        info!("Actor system initialized successfully");
        Ok(())
    }

    async fn start_physics_actor(&mut self) -> Result<(), ActorLifecycleError> {
        info!("Starting PhysicsOrchestratorActor");

        let simulation_params = SimulationParams::default();
        let actor = PhysicsOrchestratorActor::new(simulation_params, None, None);
        let addr = actor.start();

        self.physics_actor = Some(addr);
        info!("PhysicsOrchestratorActor started successfully");

        Ok(())
    }

    async fn start_semantic_actor(&mut self) -> Result<(), ActorLifecycleError> {
        info!("Starting SemanticProcessorActor");

        let actor = SemanticProcessorActor::new(None);
        let addr = actor.start();

        self.semantic_actor = Some(addr);
        info!("SemanticProcessorActor started successfully");

        Ok(())
    }

    fn start_health_monitoring(&self) {
        let physics_actor = self.physics_actor.clone();
        let semantic_actor = self.semantic_actor.clone();
        let interval = self.health_check_interval;

        actix::spawn(async move {
            let mut timer = actix::clock::interval(interval);

            loop {
                timer.tick().await;

                if let Some(addr) = &physics_actor {
                    if addr.connected() {
                        info!("PhysicsActor health check: OK");
                    } else {
                        warn!("PhysicsActor health check: DISCONNECTED");
                    }
                }

                if let Some(addr) = &semantic_actor {
                    if addr.connected() {
                        info!("SemanticActor health check: OK");
                    } else {
                        warn!("SemanticActor health check: DISCONNECTED");
                    }
                }
            }
        });
    }

    /// Gracefully shut down all managed actors.
    ///
    /// `shutdown_timeout` controls how long to wait for actors to process their
    /// final messages before the addresses are dropped (which triggers Actix to
    /// stop the actors).  Dropping the last `Addr` clone causes Actix to call
    /// `Actor::stopping` → `Actor::stopped` on the target actor, so the drop
    /// itself is the stop signal.  The timeout gives in-flight messages time to
    /// drain before we discard the addresses.
    pub async fn shutdown_with_timeout(
        &mut self,
        shutdown_timeout: Duration,
    ) -> Result<(), ActorLifecycleError> {
        info!(
            "Starting graceful actor shutdown (timeout: {:?})",
            shutdown_timeout
        );

        // Phase 1: Check connectivity and log status for each actor.
        if let Some(ref addr) = self.physics_actor {
            if addr.connected() {
                info!("PhysicsOrchestratorActor is connected — will be stopped");
            } else {
                warn!("PhysicsOrchestratorActor already disconnected");
            }
        }
        if let Some(ref addr) = self.semantic_actor {
            if addr.connected() {
                info!("SemanticProcessorActor is connected — will be stopped");
            } else {
                warn!("SemanticProcessorActor already disconnected");
            }
        }

        // Phase 2: Wait for the timeout to let in-flight messages drain.
        tokio::time::sleep(shutdown_timeout).await;

        // Phase 3: Drop the addresses. When the last Addr clone is dropped,
        // Actix transitions the actor through stopping → stopped.
        if let Some(addr) = self.physics_actor.take() {
            info!("Dropping PhysicsOrchestratorActor address to trigger Actix stop");
            drop(addr);
        }

        if let Some(addr) = self.semantic_actor.take() {
            info!("Dropping SemanticProcessorActor address to trigger Actix stop");
            drop(addr);
        }

        info!("Actor system shutdown complete");
        Ok(())
    }

    /// Gracefully shut down all managed actors with a default 5-second timeout.
    pub async fn shutdown(&mut self) -> Result<(), ActorLifecycleError> {
        self.shutdown_with_timeout(Duration::from_secs(5)).await
    }

    pub async fn restart_physics_actor(&mut self) -> Result<(), ActorLifecycleError> {
        warn!("Restarting PhysicsOrchestratorActor");

        if let Some(_addr) = self.physics_actor.take() {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        self.start_physics_actor().await?;

        info!("PhysicsOrchestratorActor restarted successfully");
        Ok(())
    }

    pub async fn restart_semantic_actor(&mut self) -> Result<(), ActorLifecycleError> {
        warn!("Restarting SemanticProcessorActor");

        if let Some(_addr) = self.semantic_actor.take() {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        self.start_semantic_actor().await?;

        info!("SemanticProcessorActor restarted successfully");
        Ok(())
    }

    pub fn get_physics_actor(&self) -> Option<&Addr<PhysicsOrchestratorActor>> {
        self.physics_actor.as_ref()
    }

    pub fn get_semantic_actor(&self) -> Option<&Addr<SemanticProcessorActor>> {
        self.semantic_actor.as_ref()
    }

    pub fn is_healthy(&self) -> bool {
        self.physics_actor.as_ref().map_or(false, |a| a.connected())
            && self
                .semantic_actor
                .as_ref()
                .map_or(false, |a| a.connected())
    }

    pub fn set_health_check_interval(&mut self, interval: Duration) {
        self.health_check_interval = interval;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActorLifecycleError {
    #[error("Actor initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Actor not running")]
    ActorNotRunning,

    #[error("Actor communication error: {0}")]
    CommunicationError(String),

    #[error("Shutdown timeout")]
    ShutdownTimeout,
}

pub struct SupervisionStrategy {
    max_restarts: usize,
    #[allow(dead_code)]
    restart_window: Duration,
}

impl Default for SupervisionStrategy {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            restart_window: Duration::from_secs(60),
        }
    }
}

impl SupervisionStrategy {
    pub fn new(max_restarts: usize, restart_window: Duration) -> Self {
        Self {
            max_restarts,
            restart_window,
        }
    }

    pub async fn handle_failure(
        &self,
        actor_name: &str,
        restart_count: usize,
    ) -> SupervisionDecision {
        if restart_count >= self.max_restarts {
            error!(
                "Actor {} exceeded max restarts ({}), giving up",
                actor_name, self.max_restarts
            );
            SupervisionDecision::Stop
        } else {
            warn!(
                "Actor {} failed, restarting (attempt {}/{})",
                actor_name,
                restart_count + 1,
                self.max_restarts
            );
            SupervisionDecision::Restart
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionDecision {
    Restart,
    Stop,
}

pub static ACTOR_SYSTEM: once_cell::sync::Lazy<Arc<RwLock<ActorLifecycleManager>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(ActorLifecycleManager::new())));

pub async fn initialize_actor_system() -> Result<(), ActorLifecycleError> {
    let mut system = ACTOR_SYSTEM.write().await;
    system.initialize().await
}

pub async fn shutdown_actor_system() -> Result<(), ActorLifecycleError> {
    let mut system = ACTOR_SYSTEM.write().await;
    system.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lifecycle_manager_creation() {
        let manager = ActorLifecycleManager::new();
        assert!(!manager.is_healthy());
    }

    #[tokio::test]
    async fn test_supervision_strategy() {
        let strategy = SupervisionStrategy::default();

        let decision = strategy.handle_failure("test_actor", 0).await;
        assert_eq!(decision, SupervisionDecision::Restart);

        let decision = strategy.handle_failure("test_actor", 3).await;
        assert_eq!(decision, SupervisionDecision::Stop);
    }

    #[test]
    fn test_supervision_strategy_custom() {
        let strategy = SupervisionStrategy::new(5, Duration::from_secs(120));
        assert_eq!(strategy.max_restarts, 5);
        assert_eq!(strategy.restart_window, Duration::from_secs(120));
    }
}
