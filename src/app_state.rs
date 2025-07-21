use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use actix::prelude::*;
use actix_web::web;
use log::info;

use crate::actors::{GraphServiceActor, SettingsActor, MetadataActor, ClientManagerActor, GPUComputeActor, ProtectedSettingsActor};
use crate::config::AppFullSettings; // Renamed for clarity, ClientFacingSettings removed
use tokio::time::Duration;
use crate::config::feature_access::FeatureAccess;
use crate::models::metadata::MetadataStore;
use crate::models::protected_settings::{ProtectedSettings, ApiKeys, NostrUser};
use crate::services::github::{GitHubClient, ContentAPI};
use crate::services::perplexity_service::PerplexityService;
use crate::services::speech_service::SpeechService;
use crate::services::ragflow_service::RAGFlowService;
use crate::services::nostr_service::NostrService;
use crate::services::swarm_client::SwarmClient;

#[derive(Clone)]
pub struct AppState {
    pub graph_service_addr: Addr<GraphServiceActor>,
    pub gpu_compute_addr: Option<Addr<GPUComputeActor>>, // Changed to actor address
    pub settings_addr: Addr<SettingsActor>,
    pub protected_settings_addr: Addr<ProtectedSettingsActor>,
    pub metadata_addr: Addr<MetadataActor>,
    pub client_manager_addr: Addr<ClientManagerActor>,
    pub github_client: Arc<GitHubClient>,
    pub content_api: Arc<ContentAPI>,
    pub perplexity_service: Option<Arc<PerplexityService>>,
    pub ragflow_service: Option<Arc<RAGFlowService>>,
    pub speech_service: Option<Arc<SpeechService>>,
    pub nostr_service: Option<web::Data<NostrService>>,
    pub feature_access: web::Data<FeatureAccess>,
    pub ragflow_session_id: String,
    pub active_connections: Arc<AtomicUsize>,
    pub swarm_client: Arc<SwarmClient>,
}

impl AppState {
    pub async fn new(
        settings: AppFullSettings,
        github_client: Arc<GitHubClient>,
        content_api: Arc<ContentAPI>,
        perplexity_service: Option<Arc<PerplexityService>>,
        ragflow_service: Option<Arc<RAGFlowService>>,
        speech_service: Option<Arc<SpeechService>>,
        ragflow_session_id: String,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("[AppState::new] Initializing actor system");
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Start actors
        info!("[AppState::new] Starting ClientManagerActor");
        let client_manager_addr = ClientManagerActor::new().start();
        
        info!("[AppState::new] Starting SettingsActor");
        let settings_addr = SettingsActor::new(settings).start();
        
        info!("[AppState::new] Starting MetadataActor");
        let metadata_addr = MetadataActor::new(MetadataStore::new()).start();
        
        info!("[AppState::new] Starting GPUComputeActor");
        let gpu_compute_addr = Some(GPUComputeActor::new().start());
        
        info!("[AppState::new] Starting GraphServiceActor");
        let graph_service_addr = GraphServiceActor::new(
            client_manager_addr.clone(),
            gpu_compute_addr.clone()
        ).start();
        
        info!("[AppState::new] Starting ProtectedSettingsActor");
        let protected_settings_addr = ProtectedSettingsActor::new(ProtectedSettings::default()).start();
        
        info!("[AppState::new] Initializing SwarmClient");
        let swarm_client = Arc::new(SwarmClient::new());
        
        info!("[AppState::new] Actor system initialization complete");
        
        Ok(Self {
            graph_service_addr,
            gpu_compute_addr,
            settings_addr,
            protected_settings_addr,
            metadata_addr,
            client_manager_addr,
            github_client,
            content_api,
            perplexity_service,
            ragflow_service,
            speech_service,
            nostr_service: None,
            feature_access: web::Data::new(FeatureAccess::from_env()),
            ragflow_session_id,
            active_connections: Arc::new(AtomicUsize::new(0)),
            swarm_client,
        })
    }

    pub fn increment_connections(&self) -> usize {
        self.active_connections.fetch_add(1, Ordering::SeqCst)
    }

    pub fn decrement_connections(&self) -> usize {
        self.active_connections.fetch_sub(1, Ordering::SeqCst)
    }

    pub async fn get_api_keys(&self, pubkey: &str) -> ApiKeys {
        use crate::actors::protected_settings_actor::GetApiKeys;
        self.protected_settings_addr.send(GetApiKeys {
            pubkey: pubkey.to_string(),
        }).await.unwrap_or_else(|_| ApiKeys::default())
    }

    pub async fn get_nostr_user(&self, pubkey: &str) -> Option<NostrUser> {
        if let Some(nostr_service) = &self.nostr_service {
            nostr_service.get_user(pubkey).await
        } else {
            None
        }
    }

    pub async fn validate_nostr_session(&self, pubkey: &str, token: &str) -> bool {
        if let Some(nostr_service) = &self.nostr_service {
            nostr_service.validate_session(pubkey, token).await
        } else {
            false
        }
    }

    pub async fn update_nostr_user_api_keys(&self, pubkey: &str, api_keys: ApiKeys) -> Result<NostrUser, String> {
        if let Some(nostr_service) = &self.nostr_service {
            nostr_service.update_user_api_keys(pubkey, api_keys)
                .await
                .map_err(|e| e.to_string())
        } else {
            Err("Nostr service not initialized".to_string())
        }
    }

    pub fn set_nostr_service(&mut self, service: NostrService) {
        self.nostr_service = Some(web::Data::new(service));
    }

    pub fn is_power_user(&self, pubkey: &str) -> bool {
        self.feature_access.is_power_user(pubkey)
    }

    pub fn can_sync_settings(&self, pubkey: &str) -> bool {
        self.feature_access.can_sync_settings(pubkey)
    }

    pub fn has_feature_access(&self, pubkey: &str, feature: &str) -> bool {
        self.feature_access.has_feature_access(pubkey, feature)
    }

    pub fn get_available_features(&self, pubkey: &str) -> Vec<String> {
        self.feature_access.get_available_features(pubkey)
    }
    
    pub fn get_client_manager_addr(&self) -> &Addr<ClientManagerActor> {
        &self.client_manager_addr
    }

    pub fn get_graph_service_addr(&self) -> &Addr<GraphServiceActor> {
        &self.graph_service_addr
    }

    pub fn get_settings_addr(&self) -> &Addr<SettingsActor> {
        &self.settings_addr
    }

    pub fn get_metadata_addr(&self) -> &Addr<MetadataActor> {
        &self.metadata_addr
    }
}
