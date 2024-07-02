use std::collections::HashMap;

use askama::Template;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use validator::Validate;

static NAME_LENGTH_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(\S.{0,198}\S)\s*$").unwrap());

use crate::libs::gitlab_api::gitlab_api::Member;

#[derive(Default)]
pub struct Guild {
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
    pub guild: Guild,
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
#[template(path = "pages/guild/guilds.html")]
pub struct GuildsListTemplate {
    pub user: Member,
    // TODO array of lists for current user
}
