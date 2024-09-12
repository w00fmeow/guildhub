use bson::oid::ObjectId;
use mongodb::bson::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TopicDocument {
    pub _id: ObjectId,
    pub guild_id: ObjectId,
    pub text: String,
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
