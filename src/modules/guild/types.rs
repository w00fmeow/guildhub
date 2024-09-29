use crate::libs::serialization;
use crate::modules::app::Event;
use crate::modules::topic::types::TopicStatus;
use askama_axum::Template;
use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use std::collections::HashMap;
use std::str::FromStr;
use validator::Validate;

static NAME_LENGTH_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(\S.{0,198}\S)\s*$").unwrap());

use crate::libs::gitlab_api::gitlab_api::Member;

use super::GuildDocument;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Guild {
    pub id: String,
    pub name: String,
    pub members: Vec<Member>,
    pub topics_count: usize,
    pub created_by_user: Member,
    #[serde(with = "serialization::chrono_date")]
    pub updated_at: DateTime<Utc>,
    #[serde(with = "serialization::chrono_date")]
    pub created_at: DateTime<Utc>,
}

impl TryFrom<Guild> for GuildDocument {
    type Error = bson::oid::Error;

    fn try_from(guild: Guild) -> Result<GuildDocument, Self::Error> {
        Ok(GuildDocument {
            _id: ObjectId::from_str(&guild.id)?,
            name: guild.name,
            member_ids: guild
                .members
                .into_iter()
                .map(|member| member.id)
                .collect(),
            created_by_user_id: guild.created_by_user.id,
            updated_at: bson::DateTime::from_chrono(guild.updated_at),
            created_at: bson::DateTime::from_chrono(guild.created_at),
        })
    }
}

#[derive(Default)]
pub struct GuildDraft {
    pub id: Option<String>,
    pub name: String,
    pub members: Vec<Member>,
}

#[serde_as]
#[derive(Deserialize, Debug, Validate)]
pub struct GuildFormDTO {
    #[validate(regex(path = *NAME_LENGTH_PATTERN, message = "Name length must be between 2 and 200 characters"))]
    pub name: String,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[serde(default)]
    pub member_ids: Vec<usize>,
    pub member_search_term: String,
}

#[derive(Template)]
#[template(path = "pages/guild/create-guild.html")]
pub struct CreateGuildFormTemplate {
    pub user: Member,
    pub guild: GuildDraft,
    pub member_search_term: String,
    pub matched_members: Vec<Member>,
    pub errors: HashMap<String, String>,
    pub is_valid: bool,
    pub should_swap_oob: bool,
}

impl CreateGuildFormTemplate {
    pub fn get_field_error_message<'a>(&'a self, field: &str) -> &'a str {
        self.errors.get(field).map(|s| s.as_str()).unwrap_or("")
    }
}

#[derive(Template)]
#[template(path = "pages/guild/edit-guild.html")]
pub struct EditGuildFormTemplate {
    pub user: Member,
    pub guild: GuildDraft,
    pub member_search_term: String,
    pub matched_members: Vec<Member>,
    pub errors: HashMap<String, String>,
    pub is_valid: bool,
    pub should_swap_oob: bool,
}

impl EditGuildFormTemplate {
    pub fn get_field_error_message<'a>(&'a self, field: &str) -> &'a str {
        self.errors.get(field).map(|s| s.as_str()).unwrap_or("")
    }
}

#[derive(Template)]
#[template(path = "pages/guild/guilds.html")]
pub struct GuildsListTemplate {
    pub user: Member,
}

#[derive(Template)]
#[template(path = "pages/guild/guild.html")]
pub struct GuildTemplate {
    pub user: Member,
    pub guild_id: String,
    pub status: TopicStatus,
}

#[derive(Template)]
#[template(path = "components/guild/guild-overview.html")]
pub struct GuildOverviewTemplate {
    pub guild_id: String,
    pub user: Member,
    pub guild: Guild,
    pub can_edit: bool,
}

#[derive(Template)]
#[template(path = "components/guild/guild-list-items.html")]
pub struct GuildListItemsTemplate {
    pub guilds: Vec<Guild>,
}

#[derive(Deserialize)]
pub struct GuildIdParameter {
    pub guild_id: String,
}

#[derive(Deserialize)]
pub struct ArchivedQueryParameter {
    pub archived: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum GuildEvent {
    Create(Guild),
    Update(Guild),
    Delete(String),
}

impl Into<Event> for GuildEvent {
    fn into(self) -> Event {
        Event::Guild(self)
    }
}
