//! Metadata Actor to replace Arc<RwLock<MetadataStore>>

use actix::prelude::*;
use log::{debug, info};

use crate::actors::messages::*;
use visionclaw_domain::models::metadata::MetadataStore;

pub struct MetadataActor {
    metadata: MetadataStore,
}

impl MetadataActor {
    pub fn new(metadata: MetadataStore) -> Self {
        Self { metadata }
    }

    pub fn get_metadata(&self) -> &MetadataStore {
        &self.metadata
    }

    pub fn update_metadata(&mut self, new_metadata: MetadataStore) {
        self.metadata = new_metadata;
        debug!("Metadata updated with {} files", self.metadata.len());
    }

    pub fn refresh_metadata(&mut self) -> Result<(), String> {
        info!("Metadata refresh requested");

        Ok(())
    }

    pub fn get_file_count(&self) -> usize {
        self.metadata.len()
    }
}

impl Actor for MetadataActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("MetadataActor started with {} files", self.metadata.len());
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("MetadataActor stopped");
    }
}

impl Handler<GetMetadata> for MetadataActor {
    type Result = Result<MetadataStore, String>;

    fn handle(&mut self, _msg: GetMetadata, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.metadata.clone())
    }
}

impl Handler<UpdateMetadata> for MetadataActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: UpdateMetadata, _ctx: &mut Self::Context) -> Self::Result {
        self.update_metadata(msg.metadata);
        Ok(())
    }
}

impl Handler<RefreshMetadata> for MetadataActor {
    type Result = Result<(), String>;

    fn handle(&mut self, _msg: RefreshMetadata, _ctx: &mut Self::Context) -> Self::Result {
        self.refresh_metadata()
    }
}
