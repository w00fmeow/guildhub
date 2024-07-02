use std::{collections::HashMap, sync::Arc};

use crate::{
    libs::{
        actix::Form, gitlab_api::gitlab_api::Member, htmx::Location,
        validator::validator_errors_to_hashmap,
    },
    modules::app::{app::App, user_extractor::Authenticated, HxTriggerEvent, ToastLevel},
};
use actix_web::{http::header::ContentType, web, HttpResponse, Responder};
use askama_actix::TemplateToResponse;
use futures::future::join_all;
use mime;
use serde::Deserialize;
use tracing::error;
use validator::Validate;

use super::{CreateGuildFormTemplate, Guild, GuildFormDTO, GuildsListTemplate};

pub async fn get_guilds_list(user: Authenticated) -> impl Responder {
    GuildsListTemplate { user: user.clone() }
}

pub async fn get_create_guild_form(user: Authenticated) -> impl Responder {
    CreateGuildFormTemplate {
        user: user.clone(),
        guild: Guild::default(),
        member_search_term: String::new(),
        matched_members: Vec::new(),
        errors: HashMap::default(),
        is_valid: false,
        should_swap_oob: false,
    }
}

pub async fn post_create_guild_form_draft(
    user: Authenticated,
    app: web::Data<Arc<App>>,
    form: Form<GuildFormDTO>,
) -> impl Responder {
    let mut members = Vec::new();

    if !form.member_search_term.is_empty() {
        members = app
            .gitlab_service
            .get_all_cached_members()
            .await
            .into_iter()
            .filter(|member| {
                if form.member_ids.contains(&member.id) {
                    return false;
                }

                let user_name_match = member
                    .username
                    .to_lowercase()
                    .contains(form.member_search_term.to_lowercase().trim());

                return user_name_match;
            })
            .collect();
    }
    let errors = validator_errors_to_hashmap(form.validate().err());

    CreateGuildFormTemplate {
        user: user.clone(),
        guild: Guild {
            name: form.name.clone(),
            members: Vec::new(),
        },
        member_search_term: form.member_search_term.clone(),
        matched_members: members,
        is_valid: errors.is_empty(),
        errors,
        should_swap_oob: true,
    }
}

pub async fn create_guild(user: Authenticated, form: Form<GuildFormDTO>) -> impl Responder {
    match form.validate() {
        Err(errors) => {
            let body = CreateGuildFormTemplate {
                user: user.clone(),
                guild: Guild {
                    name: form.name.clone(),
                    members: Vec::new(),
                },
                member_search_term: form.member_search_term.clone(),
                matched_members: Vec::new(),
                is_valid: false,
                errors: validator_errors_to_hashmap(Some(errors)),
                should_swap_oob: true,
            };

            return HttpResponse::Ok()
                .insert_header(ContentType(mime::TEXT_HTML))
                .body(body.to_string());
        }
        Ok(()) => {}
    };

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Guild was created".to_string(),
    };

    let event = match serde_json::to_string(&event) {
        Ok(result) => result,
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let location = Location {
        path: "/guilds".to_string(),
        target: "#content".to_string(),
        swap: "outerHTML".to_string(),
    };

    let location = match serde_json::to_string(&location) {
        Ok(result) => result,
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let body = GuildsListTemplate {
        user: user.to_owned(),
    };

    return HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .insert_header(("HX-Location", location))
        .insert_header(("HX-Trigger", event))
        .body(body.to_string());
}

pub async fn remove_member() -> impl Responder {
    return HttpResponse::Ok();
}

#[derive(Deserialize)]
pub struct MemberIdParam {
    pub id: usize,
}

pub async fn insert_new_member(
    user: Authenticated,
    parameters: web::Path<MemberIdParam>,
    app: web::Data<Arc<App>>,
    form: Form<GuildFormDTO>,
) -> impl Responder {
    let member_to_insert = match app.gitlab_service.get_cached_member(&parameters.id).await {
        Some(member) => member,
        None => return HttpResponse::NotFound().finish(),
    };

    let existing_members = form
        .member_ids
        .iter()
        .map(|id| app.gitlab_service.get_cached_member(id));

    let existing_members = join_all(existing_members).await;

    let mut existing_members: Vec<Member> = existing_members
        .into_iter()
        .filter(|member| member.is_some())
        .map(|member| member.unwrap())
        .collect();

    if !form.member_ids.contains(&parameters.id) {
        existing_members.insert(0, member_to_insert)
    }

    let errors = validator_errors_to_hashmap(form.validate().err());

    CreateGuildFormTemplate {
        user: user.to_owned(),
        guild: Guild {
            name: form.name.clone(),
            members: existing_members,
        },
        member_search_term: String::new(),
        matched_members: Vec::new(),
        is_valid: errors.is_empty(),
        errors,
        should_swap_oob: false,
    }
    .to_response()
}
