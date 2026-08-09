//! Briefing Service — orchestrates the brief → execute → debrief workflow
//! from the VisionClaw Rust backend through the Management API.
//!
//! This service translates BriefingRequest structs into Management API calls,
//! creating briefs, spawning role-specific agents, and consolidating debriefs.

use crate::services::management_api_client::ManagementApiClient;
use crate::types::user_context::{BriefingRequest, BriefingResponse, RoleTask, UserContext};
use log::info;

pub struct BriefingService {
    api_client: ManagementApiClient,
}

impl BriefingService {
    pub fn new(api_client: ManagementApiClient) -> Self {
        Self { api_client }
    }

    /// Submit a briefing request to the Management API.
    ///
    /// This creates a brief file in the team folder structure, optionally creates
    /// a Beads epic, then spawns role-specific agents to respond.
    pub async fn submit_brief(
        &self,
        request: &BriefingRequest,
        user_context: &UserContext,
    ) -> Result<BriefingResponse, BriefingError> {
        info!(
            "[BriefingService] Submitting brief for user={}, roles={:?}",
            user_context.display_name, request.roles
        );

        // Step 1: Create the brief via Management API
        let brief_result = self
            .api_client
            .create_brief(
                &request.content,
                &request.roles,
                user_context,
                request.version.as_deref(),
                request.brief_type.as_deref(),
                request.slug.as_deref(),
            )
            .await
            .map_err(|e| BriefingError::ApiError(format!("Failed to create brief: {}", e)))?;

        let brief_id = brief_result.brief_id.clone();
        let brief_path = brief_result.brief_path.clone();
        let bead_id = brief_result.bead_id.clone();

        // Step 2: Execute the brief (spawn role agents)
        let role_tasks = self
            .api_client
            .execute_brief(
                &brief_id,
                &brief_path,
                &request.roles,
                user_context,
                bead_id.as_deref(),
            )
            .await
            .map_err(|e| BriefingError::ApiError(format!("Failed to execute brief: {}", e)))?;

        info!(
            "[BriefingService] Brief {} submitted: {} role agents spawned",
            brief_id,
            role_tasks.len()
        );

        Ok(BriefingResponse {
            brief_id,
            brief_path,
            bead_id,
            role_tasks,
        })
    }

    /// Request a debrief consolidation for a completed brief.
    pub async fn request_debrief(
        &self,
        brief_id: &str,
        role_tasks: &[RoleTask],
        user_context: &UserContext,
    ) -> Result<String, BriefingError> {
        info!(
            "[BriefingService] Requesting debrief for brief={}, user={}",
            brief_id, user_context.display_name
        );

        let debrief_path = self
            .api_client
            .create_debrief(brief_id, role_tasks, user_context)
            .await
            .map_err(|e| BriefingError::ApiError(format!("Failed to create debrief: {}", e)))?;

        info!("[BriefingService] Debrief created at {}", debrief_path);

        Ok(debrief_path)
    }
}

#[derive(Debug)]
pub enum BriefingError {
    ApiError(String),
}

impl std::fmt::Display for BriefingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BriefingError::ApiError(msg) => write!(f, "Briefing API error: {}", msg),
        }
    }
}

impl std::error::Error for BriefingError {}
