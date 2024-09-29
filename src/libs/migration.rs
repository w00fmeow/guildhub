use async_trait::async_trait;

use anyhow::Result;
use std::sync::Arc;

use super::mongo::MongoDatabase;

#[async_trait]
pub trait Migration {
    fn name(&self) -> String;

    async fn run(&self, database: Arc<MongoDatabase>) -> Result<()>;
}
