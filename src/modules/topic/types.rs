use std::collections::HashMap;

use crate::{
    libs::{gitlab_api::gitlab_api::Member, serialization},
    modules::app::Event,
};
use askama::Template;
use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::str::FromStr;
use validator::Validate;

use super::TopicDocument;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Topic {
    pub id: String,
    pub guild_id: String,
    pub text: String,
    pub will_be_presented_by_the_creator: bool,
    pub created_by_user_id: usize,
    pub upvoted_by_users_ids: Vec<usize>,
    #[serde(with = "serialization::chrono_date")]
    pub updated_at: DateTime<Utc>,
    #[serde(with = "serialization::chrono_date")]
    pub created_at: DateTime<Utc>,
}

impl TryFrom<Topic> for TopicDocument {
    type Error = bson::oid::Error;

    fn try_from(topic: Topic) -> Result<TopicDocument, Self::Error> {
        Ok(TopicDocument {
            _id: ObjectId::from_str(&topic.id)?,
            guild_id: ObjectId::from_str(&topic.guild_id)?,
            text: topic.text,
            will_be_presented_by_the_creator: topic
                .will_be_presented_by_the_creator,
            upvoted_by_users_ids: topic.upvoted_by_users_ids,
            created_by_user_id: topic.created_by_user_id,
            updated_at: bson::DateTime::from_chrono(topic.updated_at),
            created_at: bson::DateTime::from_chrono(topic.created_at),
        })
    }
}

impl From<TopicDocument> for Topic {
    fn from(document: TopicDocument) -> Topic {
        Topic {
            id: document._id.to_hex(),
            guild_id: document.guild_id.to_hex(),
            text: document.text,
            will_be_presented_by_the_creator: document
                .will_be_presented_by_the_creator,
            upvoted_by_users_ids: document.upvoted_by_users_ids,
            created_by_user_id: document.created_by_user_id,
            updated_at: document.updated_at.to_chrono(),
            created_at: document.created_at.to_chrono(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TopicPersonalized {
    pub id: String,
    pub guild_id: String,
    pub text: String,
    pub will_be_presented_by_the_creator: bool,
    pub is_created_by_current_user: bool,
    pub is_upvoted_by_current_user: bool,
    pub created_by_user: Member,
    pub upvoted_by_users: Vec<Member>,
    #[serde(with = "serialization::chrono_date")]
    pub updated_at: DateTime<Utc>,
    #[serde(with = "serialization::chrono_date")]
    pub created_at: DateTime<Utc>,
}

impl From<TopicPersonalized> for Topic {
    fn from(topic: TopicPersonalized) -> Topic {
        Topic {
            id: topic.id,
            guild_id: topic.guild_id,
            text: topic.text,
            upvoted_by_users_ids: topic
                .upvoted_by_users
                .into_iter()
                .map(|member| member.id)
                .collect(),
            will_be_presented_by_the_creator: topic
                .will_be_presented_by_the_creator,
            created_by_user_id: topic.created_by_user.id,
            updated_at: topic.updated_at,
            created_at: topic.created_at,
        }
    }
}

static TEXT_LENGTH_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(\S.{0,198}\S)\s*$").unwrap());

#[serde_as]
#[derive(Deserialize, Debug, Validate)]
pub struct TopicFormDTO {
    #[validate(regex(path = *TEXT_LENGTH_PATTERN, message = "Length must be between 2 and 200 characters"))]
    pub text: String,
    #[serde(rename = "i-will-present")]
    pub will_be_presented_by_the_creator: Option<bool>,
}

pub struct TopicDraft {
    pub id: Option<String>,
    pub guild_id: String,
    pub text: String,
    pub will_be_presented_by_the_creator: bool,
}

#[derive(Template)]
#[template(path = "pages/topic/create-topic.html")]
pub struct CreateTopicTemplate {
    pub user: Member,
    pub topic: TopicDraft,
    pub errors: HashMap<String, String>,
    pub is_valid: bool,
    pub should_swap_oob: bool,
}

impl CreateTopicTemplate {
    pub fn get_field_error_message<'a>(&'a self, field: &str) -> &'a str {
        self.errors.get(field).map(|s| s.as_str()).unwrap_or("")
    }
}

#[derive(Template)]
#[template(path = "pages/topic/edit-topic.html")]
pub struct EditTopicTemplate {
    pub user: Member,
    pub topic: TopicDraft,
    pub errors: HashMap<String, String>,
    pub is_valid: bool,
    pub should_swap_oob: bool,
}

impl EditTopicTemplate {
    pub fn get_field_error_message<'a>(&'a self, field: &str) -> &'a str {
        self.errors.get(field).map(|s| s.as_str()).unwrap_or("")
    }
}

#[derive(Template)]
#[template(path = "components/topic/topics-list.html")]
pub struct TopicsListTemplate {
    pub guild_id: String,
    pub current_page: usize,
    pub has_more_topics: bool,
    pub topics: Vec<TopicPersonalized>,
}

#[derive(Template)]
#[template(path = "components/topic/topic-list-item.html")]
pub struct TopicsListItemTemplate {
    pub topic: TopicPersonalized,
}

pub struct VoteTopicResult {
    pub previously_voted: Option<TopicPersonalized>,
    pub topic: TopicPersonalized,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum TopicEvent {
    Create(Topic),
    Update(Topic),
    Delete(Topic),
    OrderChange(Vec<String>),
}

impl Into<Event> for TopicEvent {
    fn into(self) -> Event {
        Event::Topic(self)
    }
}

pub struct PaginationParameters {
    pub limit: usize,
    pub skip: usize,
}
