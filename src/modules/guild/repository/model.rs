use bson::oid::ObjectId;
use mongodb::bson::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GuildDocument {
    pub _id: ObjectId,
    pub name: String,
    pub created_by_user_id: usize,
    pub member_ids: Vec<usize>,
    pub updated_at: DateTime,
    pub created_at: DateTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UpdateGuildPayload {
    pub name: String,
    pub member_ids: Vec<usize>,
    pub updated_at: DateTime,
}
