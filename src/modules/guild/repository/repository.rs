use std::sync::Arc;

use crate::libs::mongo::MongoDatabase;
use anyhow::{Context, Result};
use bson::{doc, oid::ObjectId};
use futures_util::TryStreamExt;
use mongodb::{
    options::{FindOptions, IndexOptions},
    results::{DeleteResult, InsertOneResult, UpdateResult},
    Collection, IndexModel,
};

use super::{model::GuildDocument, UpdateGuildPayload};

pub struct GuildsRepository {
    database: Arc<MongoDatabase>,
    collection_name: String,
}

impl GuildsRepository {
    pub async fn new(database: Arc<MongoDatabase>) -> Self {
        let repo = GuildsRepository {
            database,
            collection_name: String::from("guilds"),
        };

        let _ = repo.set_indexes().await;

        repo
    }

    pub async fn set_indexes(&self) -> Result<()> {
        let indexes =
            vec![("created_by_user_id", doc! {"created_by_user_id":1})]
                .into_iter()
                .map(|(index_name, doc)| {
                    let options = IndexOptions::builder()
                        .name(index_name.to_string())
                        .build();

                    IndexModel::builder().keys(doc).options(options).build()
                })
                .collect();

        self.database
            .create_indexes::<GuildDocument>(&self.collection_name, indexes)
            .await
    }

    pub async fn insert_guild_document(
        &self,
        document: GuildDocument,
    ) -> Result<InsertOneResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<GuildDocument> =
            database.collection(&self.collection_name);

        let result = collection
            .insert_one(document, None)
            .await
            .with_context(|| "Failed to insert guild document")?;

        Ok(result)
    }

    pub async fn get_guilds(
        &self,
        current_user_id: usize,
    ) -> Result<Vec<GuildDocument>> {
        let database = self.database.get_database_client()?;

        let collection: Collection<GuildDocument> =
            database.collection(&self.collection_name);

        let find_options = FindOptions::builder()
            .sort(doc! {
                "created_at": -1
            })
            .build();

        let documents = collection
            .find(
                doc! {
                    "$or": [
                        {
                            "created_by_user_id": current_user_id as u32,
                        },
                        {
                            "member_ids": current_user_id as u32,
                        }
                    ]
                },
                Some(find_options),
            )
            .await?
            .try_collect()
            .await?;

        Ok(documents)
    }

    pub async fn get_guild(
        &self,
        id: ObjectId,
        user_id: Option<usize>,
    ) -> Result<Option<GuildDocument>> {
        let database = self.database.get_database_client()?;

        let collection: Collection<GuildDocument> =
            database.collection(&self.collection_name);

        let query = match user_id {
            Some(user_id) => doc! {

                "_id": id,
                "$or": [
                    {
                        "created_by_user_id": user_id as u32,
                    },
                    {
                        "member_ids": user_id as u32
                    },
                ]
            },
            None => doc! {
                "_id": id
            },
        };

        let document = collection.find_one(query, None).await?;

        Ok(document)
    }

    pub async fn delete_guild(
        &self,
        id: ObjectId,
        user_id: Option<usize>,
    ) -> Result<DeleteResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<GuildDocument> =
            database.collection(&self.collection_name);

        let query = match user_id {
            Some(user_id) => doc! {

                "_id": id,
                "$or": [
                    {
                        "created_by_user_id": user_id as u32,
                    },
                    {
                        "member_ids": user_id as u32
                    },
                ]
            },
            None => doc! {
                "_id": id
            },
        };

        let result = collection.delete_one(query, None).await?;

        Ok(result)
    }

    pub async fn update_guild(
        &self,
        id: ObjectId,
        user_id: usize,
        payload: UpdateGuildPayload,
    ) -> Result<UpdateResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<GuildDocument> =
            database.collection(&self.collection_name);

        let query = doc! {

            "_id": id,
            "created_by_user_id": user_id as u32,
        };

        let payload = doc! {
            "$set": bson::to_bson(&payload)?
        };

        let result = collection.update_one(query, payload, None).await?;

        Ok(result)
    }
}
