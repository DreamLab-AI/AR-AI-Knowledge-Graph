use crate::config::AppFullSettings; // Use AppFullSettings, ConfigPerplexitySettings removed
use crate::models::metadata::Metadata;
use crate::services::file_service::ProcessedFile;
use chrono::Utc;
use log::{error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

const MARKDOWN_DIR: &str = "data/markdown";

#[derive(Debug, Serialize, Deserialize)]
struct PerplexityResponse {
    content: String,
    link: String,
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    query: String,
    conversation_id: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
    top_p: f32,
    presence_penalty: f32,
    frequency_penalty: f32,
}

pub struct PerplexityService {
    client: Client,
    settings: Arc<RwLock<AppFullSettings>>, // Changed to AppFullSettings
}

impl PerplexityService {
    pub async fn new(settings: Arc<RwLock<AppFullSettings>>) -> Result<Self, Box<dyn StdError + Send + Sync>> { // Changed signature
        let timeout_duration = {
            let settings_read = settings.read().await;
            // Safely access optional perplexity settings, provide default timeout
            settings_read.perplexity.as_ref().and_then(|p| p.timeout).unwrap_or(30) // Default 30s
        };

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_duration)) // Use extracted timeout
            .build()?;

        Ok(Self { 
            client,
            settings: Arc::clone(&settings)
        })
    }

    pub async fn query(&self, query: &str, conversation_id: &str) -> Result<String, Box<dyn StdError + Send + Sync>> {
        let settings_read = self.settings.read().await;
        
        // Get perplexity settings or return error if not configured
        let perplexity_config = match settings_read.perplexity.as_ref() {
            Some(p) => p,
            None => return Err("Perplexity settings not configured".into()),
        };

        // Safely get required fields or return error
        let api_url = perplexity_config.api_url.as_deref().ok_or("Perplexity API URL not configured")?;
        let api_key = perplexity_config.api_key.as_deref().ok_or("Perplexity API Key not configured")?;
        let model = perplexity_config.model.as_deref().ok_or("Perplexity model not configured")?;

        info!("Sending query to Perplexity API: {}", api_url);

        // Use defaults for optional parameters if not set in config
        let request = QueryRequest {
            query: query.to_string(),
            conversation_id: conversation_id.to_string(),
            model: model.to_string(),
            max_tokens: perplexity_config.max_tokens.unwrap_or(4096),
            temperature: perplexity_config.temperature.unwrap_or(0.5),
            top_p: perplexity_config.top_p.unwrap_or(0.9),
            presence_penalty: perplexity_config.presence_penalty.unwrap_or(0.0),
            frequency_penalty: perplexity_config.frequency_penalty.unwrap_or(0.0),
        };

        let response = self.client
            .post(api_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("Perplexity API error: Status: {}, Error: {}", status, error_text);
            return Err(format!("Perplexity API error: {}", error_text).into());
        }

        let perplexity_response: PerplexityResponse = response.json().await?;
        Ok(perplexity_response.content)
    }

    pub async fn process_file(&self, file_name: &str) -> Result<ProcessedFile, Box<dyn StdError + Send + Sync>> {
        let file_path = format!("{}/{}", MARKDOWN_DIR, file_name);
        if !Path::new(&file_path).exists() {
            return Err(format!("File not found: {}", file_name).into());
        }

        let content = fs::read_to_string(&file_path)?;
        let settings_read = self.settings.read().await;

        // Get perplexity settings or return error if not configured
        let perplexity_config = match settings_read.perplexity.as_ref() {
            Some(p) => p,
            None => return Err("Perplexity settings not configured".into()),
        };
        
        // Safely get required fields or return error
        let api_url = perplexity_config.api_url.as_deref().ok_or("Perplexity API URL not configured")?;
        let api_key = perplexity_config.api_key.as_deref().ok_or("Perplexity API Key not configured")?;

        info!("Sending request to Perplexity API: {}", api_url);

        // Assuming the API takes the raw content as JSON string body? If not, adjust .json(&content)
        let response = self.client
            .post(api_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&content)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("Perplexity API error: Status: {}, Error: {}", status, error_text);
            return Err(format!("Perplexity API error: {}", error_text).into());
        }

        let perplexity_response: PerplexityResponse = response.json().await?;
        
        // Create metadata for processed file
        let metadata = Metadata {
            file_name: file_name.to_string(),
            file_size: perplexity_response.content.len(),
            node_id: "0".to_string(), // Will be assigned properly later
            node_size: 10.0, // Default size
            hyperlink_count: 0,
            sha1: String::new(),
            last_modified: Utc::now(),
            last_content_change: Some(Utc::now()), // Perplexity processed content is new
            last_commit: None, // Not from GitHub
            change_count: Some(1), // First generation
            file_blob_sha: None, // Not from GitHub
            perplexity_link: perplexity_response.link,
            last_perplexity_process: Some(Utc::now()),
            topic_counts: HashMap::new(),
        };

        Ok(ProcessedFile {
            file_name: file_name.to_string(),
            content: perplexity_response.content,
            is_public: true,
            metadata,
        })
    }
}
