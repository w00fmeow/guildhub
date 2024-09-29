use std::sync::Arc;

use crate::{
    libs::{mongo::MongoDatabase, utils::omit_values},
    modules::topic::types::{PaginationParameters, TopicStatus},
};
use anyhow::{Context, Result};
use bson::{doc, oid::ObjectId, Document};
use futures::TryStreamExt;
use mongodb::{
    options::{AggregateOptions, IndexOptions},
    results::{DeleteResult, InsertOneResult, UpdateResult},
    Collection, IndexModel,
};
use serde_json::Value;

use super::{
    PartialTopicDocument, TopicDocument, TopicDocumentId,
    TopicsCountAggregationResult,
};

pub struct TopicsRepository {
    database: Arc<MongoDatabase>,
    collection_name: String,
}

impl TopicsRepository {
    pub async fn new(database: Arc<MongoDatabase>) -> Self {
        let repo = TopicsRepository {
            database,
            collection_name: String::from("topics"),
        };

        let _ = repo.set_indexes().await;

        repo
    }

    pub async fn set_indexes(&self) -> Result<()> {
        let indexes = vec![
            ("guild_id", doc! {"guild_id":1}),
            ("created_by_user_id", doc! {"created_by_user_id":1}),
            ("status", doc! {"status":1}),
        ]
        .into_iter()
        .map(|(index_name, doc)| {
            let options =
                IndexOptions::builder().name(index_name.to_string()).build();

            IndexModel::builder().keys(doc).options(options).build()
        })
        .collect();

        self.database
            .create_indexes::<TopicDocument>(&self.collection_name, indexes)
            .await
    }

    pub async fn insert_topic_document(
        &self,
        document: TopicDocument,
    ) -> Result<InsertOneResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let result = collection
            .insert_one(document, None)
            .await
            .with_context(|| "Failed to insert topic document")?;

        Ok(result)
    }

    pub async fn get_guild_topics(
        &self,
        PaginationParameters { skip, limit }: PaginationParameters,
        filter_by: PartialTopicDocument,
    ) -> Result<Vec<TopicDocument>> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let mut pipeline = self.get_topics_aggregation_pipeline(filter_by)?;

        pipeline.push(doc! {
            "$skip": skip as u32
        });

        pipeline.push(doc! {
            "$limit": limit as u32
        });

        let options: AggregateOptions =
            AggregateOptions::builder().allow_disk_use(true).build();

        let mut cursor = collection
            .aggregate(pipeline, options)
            .await
            .context("Failed to fetch topics by guild id")?;

        let mut results = Vec::new();

        while let Some(result_doc) = cursor.try_next().await? {
            let shop_aggregation_result: TopicDocument =
                bson::from_bson(bson::Bson::Document(result_doc))
                    .context("Failed to serialize topic documents")?;

            results.push(shop_aggregation_result);
        }

        Ok(results)
    }

    pub async fn get_topic(
        &self,
        id: ObjectId,
    ) -> Result<Option<TopicDocument>> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let query = doc! {
            "_id": id,
        };

        let document = collection.find_one(query, None).await?;

        Ok(document)
    }

    pub async fn delete_topic(
        &self,
        id: ObjectId,
        user_id: Option<usize>,
    ) -> Result<DeleteResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let query = match user_id {
            Some(user_id) => doc! {

                "_id": id,
                "created_by_user_id": user_id as u32,
            },
            None => doc! {
                "_id": id
            },
        };

        let result = collection.delete_one(query, None).await?;

        Ok(result)
    }

    pub async fn update_topic(
        &self,
        id: ObjectId,
        payload: PartialTopicDocument,
    ) -> Result<UpdateResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let query = doc! {
            "_id": id,
        };

        let payload = omit_values(payload, serde_json::Value::Null)?;

        let payload = doc! {
            "$set": bson::to_bson(&payload)?
        };

        let result = collection.update_one(query, payload, None).await?;

        Ok(result)
    }

    pub async fn get_topics_count_by_guild_ids(
        &self,
        guild_ids: Vec<ObjectId>,
    ) -> Result<Vec<TopicsCountAggregationResult>> {
        if guild_ids.is_empty() {
            return Ok(Vec::new());
        }

        let ids_count = guild_ids.len();

        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let pipeline = vec![
            doc! {
            "$match": {
                "guild_id" : {
                    "$in": guild_ids,
                },
            },
            },
            doc! {
                "$group": {
                    "_id": "$guild_id",
                    "count": { "$sum": 1 }
                }
            },
            doc! {
                "$project":  {
                   "guild_id": "$_id",
                   "count": "$count"
               }
            },
        ];

        let options: AggregateOptions =
            AggregateOptions::builder().allow_disk_use(true).build();

        let mut cursor = collection.aggregate(pipeline, options).await?;

        let mut results = Vec::with_capacity(ids_count);

        while let Some(result_doc) = cursor.try_next().await? {
            let shop_aggregation_result: TopicsCountAggregationResult =
                bson::from_bson(bson::Bson::Document(result_doc))?;

            results.push(shop_aggregation_result);
        }

        Ok(results)
    }

    pub async fn upvote_topic(
        &self,
        id: ObjectId,
        user_id: usize,
    ) -> Result<UpdateResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let query = doc! {
            "_id": id,
        };

        let payload = doc! {
            "$addToSet": doc!{
                "upvoted_by_users_ids": user_id as u32
            }
        };

        let result = collection.update_one(query, payload, None).await?;

        Ok(result)
    }

    pub async fn remove_user_vote_by_guild_id(
        &self,
        guild_id: &ObjectId,
        user_id: usize,
    ) -> Result<UpdateResult> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let query = doc! {
            "guild_id": guild_id,
            "status": TopicStatus::Created.to_string()
        };

        let payload = doc! {
            "$pull": doc!{
                "upvoted_by_users_ids": user_id as u32
            }
        };

        let result = collection.update_many(query, payload, None).await?;

        Ok(result)
    }

    pub async fn get_upvoted_topic_by_guild_id(
        &self,
        guild_id: &ObjectId,
        user_id: usize,
    ) -> Result<Option<TopicDocument>> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let query = doc! {
            "guild_id": guild_id,
            "status": TopicStatus::Created.to_string(),
            "upvoted_by_users_ids" : user_id as u32
        };

        let document = collection.find_one(query, None).await?;

        Ok(document)
    }

    fn get_topics_aggregation_pipeline(
        &self,
        partial_topic: PartialTopicDocument,
    ) -> Result<Vec<Document>> {
        Ok(vec![
            doc! {
                "$match": bson::to_bson(&omit_values(partial_topic, Value::Null)?)?,
            },
            doc! {
                "$addFields": {
                    "upvotes_count": {"$size": "$upvoted_by_users_ids"},
                }
            },
            doc! {
            "$sort": {
                "upvotes_count" : -1,
                "updated_at": -1
                },
            },
        ])
    }

    pub async fn get_topic_ids_sorted(
        &self,
        guild_id: &ObjectId,
    ) -> Result<Vec<String>> {
        let database = self.database.get_database_client()?;

        let collection: Collection<TopicDocument> =
            database.collection(&self.collection_name);

        let mut pipeline =
            self.get_topics_aggregation_pipeline(PartialTopicDocument {
                guild_id: Some(*guild_id),
                text: None,
                status: Some(TopicStatus::Created),
                will_be_presented_by_the_creator: None,
                updated_at: None,
                upvoted_by_users_ids: None,
            })?;

        pipeline.push(doc! {
                "$project": {
                    "_id": "$_id"
                }
        });

        let options: AggregateOptions =
            AggregateOptions::builder().allow_disk_use(true).build();

        let mut cursor = collection.aggregate(pipeline, options).await?;

        let mut results = Vec::new();

        while let Some(result_doc) = cursor.try_next().await? {
            let topic: TopicDocumentId =
                bson::from_bson(bson::Bson::Document(result_doc))?;

            results.push(topic._id.to_hex());
        }

        Ok(results)
    }
}
