use bson::oid::ObjectId;
use mongodb::bson::DateTime;
use serde::{Deserialize, Serialize};

use crate::modules::topic::types::TopicStatus;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TopicDocument {
    pub _id: ObjectId,
    pub guild_id: ObjectId,
    pub text: String,
    pub status: TopicStatus,
    pub will_be_presented_by_the_creator: bool,
    pub created_by_user_id: usize,
    pub upvoted_by_users_ids: Vec<usize>,
    pub updated_at: DateTime,
    pub created_at: DateTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TopicsCountAggregationResult {
    pub guild_id: ObjectId,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TopicDocumentId {
    pub _id: ObjectId,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PartialTopicDocument {
    pub guild_id: Option<ObjectId>,
    pub text: Option<String>,
    pub status: Option<TopicStatus>,
    pub will_be_presented_by_the_creator: Option<bool>,
    pub updated_at: Option<DateTime>,
    pub upvoted_by_users_ids: Option<Vec<usize>>,
}
