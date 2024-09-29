use async_trait::async_trait;
use bson::doc;
use mongodb::Collection;
use std::sync::Arc;
use tracing::info;

use anyhow::Result;

use crate::{
    libs::{migration::Migration, mongo::MongoDatabase},
    modules::topic::{types::TopicStatus, TopicDocument},
};

pub struct AddTopicStatusMigration {}

#[async_trait]
impl Migration for AddTopicStatusMigration {
    fn name(&self) -> String {
        "Add topic status property".to_string()
    }

    async fn run(&self, db: Arc<MongoDatabase>) -> Result<()> {
        let database = db.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection("topics");

        let result = collection
            .update_many(
                doc! {
                    "status": {"$exists": false}
                },
                doc! {
                    "$set" : {
                        "status": TopicStatus::Created.to_string()
                    }
                },
                None,
            )
            .await?;

        info!(
            "Added status property to {} topic documents ",
            result.modified_count
        );

        Ok(())
    }
}
