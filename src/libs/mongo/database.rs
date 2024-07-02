use anyhow::Result;
use async_trait::async_trait;
use bson::doc;
use mongodb::{Client, ClientSession, Collection, Database, IndexModel};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::configuration::MongoConfiguration;
use crate::libs::health_checker::HealthChecker;
use crate::modules::app::errors::AppError;

pub struct MongoDatabase {
    pub client: Client,
    pub configuration: Arc<MongoConfiguration>,
    is_healthy: AtomicBool,
}

impl MongoDatabase {
    pub fn new(configuration: Arc<MongoConfiguration>) -> Self {
        Self {
            client: Client::with_options(configuration.client_options.clone())
                .expect("Failed to create MongoDB client"),
            configuration,
            is_healthy: AtomicBool::new(false),
        }
    }

    pub fn get_database_client(&self) -> Result<Database> {
        match self.client.default_database() {
            Some(db) => Ok(db),
            None => Err(AppError::InternalError.into()),
        }
    }

    pub async fn start_session(&self) -> Result<ClientSession> {
        Ok(self.client.start_session(None).await?)
    }

    pub async fn create_indexes<T>(
        &self,
        collection_name: &str,
        indexes: Vec<IndexModel>,
    ) -> Result<()> {
        let database = self.get_database_client()?;

        let existing_collections = database.list_collection_names(None).await?;

        if !existing_collections.contains(&collection_name.to_string()) {
            database.create_collection(collection_name, None).await?;
        }

        let collection: Collection<T> = database.collection(collection_name);

        let existing_indexes = collection.list_index_names().await?;

        let new_index_names: Vec<String> = indexes
            .iter()
            .map(|index| index.clone().options.unwrap().name.unwrap())
            .collect();

        let indexes_to_remove = existing_indexes.iter().filter(|existing_index| {
            !new_index_names.contains(&existing_index) && existing_index.as_str() != "_id_"
        });

        for index_to_remove in indexes_to_remove {
            match collection.drop_index(index_to_remove, None).await {
                Ok(_) => {
                    info!(
                        "Removed db index: {} on '{}' collection",
                        &index_to_remove, collection_name
                    );
                }
                Err(error) => {
                    warn!(
                        "Failed to drop db index {} on '{}' collection: {}",
                        &index_to_remove, collection_name, error
                    );
                }
            }
        }

        for index in indexes {
            let index_name = index.clone().options.unwrap().name.unwrap();

            if existing_indexes.contains(&index_name) {
                debug!("Skipping creation of {index_name} index as it already exists in database");
                continue;
            }
            match collection.create_index(index, None).await {
                Ok(_) => {
                    info!(
                        "Created db index: {} on '{}' collection",
                        &index_name, collection_name
                    );
                }
                Err(error) => {
                    warn!(
                        "Failed to create db index {} on '{}' collection: {}",
                        &index_name, collection_name, error
                    );
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl HealthChecker for MongoDatabase {
    fn get_service_name(&self) -> String {
        "Mongo DB".to_string()
    }

    fn get_health(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    fn persist_new_health_status(&self, is_healthy: bool) {
        self.is_healthy.store(is_healthy, Ordering::Relaxed);
    }

    async fn check_is_healthy(&self) -> Result<()> {
        let database = self.get_database_client()?;

        database.run_command(doc! { "ping": 1 }, None).await?;

        Ok(())
    }
}
