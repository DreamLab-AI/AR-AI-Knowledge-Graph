//! Metadata Actor to replace Arc<RwLock<MetadataStore>>
//!
//! # This actor owns no metadata source (ADR-2097)
//!
//! [`MetadataStore`] is a plain `HashMap<String, Metadata>`, and this actor is
//! constructed with an empty one (`app_state.rs`). It is a *cache cell*, not a
//! loader: the file on disk (`metadata.json`) is owned by `FileService`, and
//! the file/graph handlers push each rebuilt store in through
//! [`UpdateMetadata`]. There is exactly one write path.
//!
//! A `RefreshMetadata` message used to sit alongside it, handled by a
//! `refresh_metadata` that logged one line and returned `Ok(())`. Nothing ever
//! sent it, and nothing could implement it honestly — reloading here would give
//! `metadata.json` a second owner and let the two copies disagree. It was
//! deleted rather than filled in.

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

#[cfg(test)]
mod tests {
    use super::*;
    use visionclaw_domain::models::metadata::Metadata;

    fn store_with(names: &[&str]) -> MetadataStore {
        names
            .iter()
            .map(|name| ((*name).to_string(), Metadata::default()))
            .collect()
    }

    /// `UpdateMetadata` is the sole write path (ADR-2097): it replaces the cell
    /// wholesale, which is what the handlers that own `metadata.json` expect.
    #[test]
    fn update_replaces_the_whole_store() {
        let mut actor = MetadataActor::new(store_with(&["a.md", "b.md"]));
        assert_eq!(actor.get_file_count(), 2);

        actor.update_metadata(store_with(&["c.md"]));
        assert_eq!(actor.get_file_count(), 1);
        assert!(actor.get_metadata().contains_key("c.md"));
        assert!(
            !actor.get_metadata().contains_key("a.md"),
            "update is a replacement, not a merge"
        );
    }

    /// An empty update genuinely empties the cell — the actor never falls back
    /// to a source of its own, because it has none.
    #[test]
    fn update_to_empty_leaves_no_residue() {
        let mut actor = MetadataActor::new(store_with(&["a.md"]));
        actor.update_metadata(MetadataStore::new());
        assert_eq!(actor.get_file_count(), 0);
    }
}
