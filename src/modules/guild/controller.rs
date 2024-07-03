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
use mime;
use serde::Deserialize;
use tracing::error;
use validator::Validate;

use super::{
    CreateGuildFormTemplate, EditGuildFormTemplate, GuildDraft, GuildFormDTO,
    GuildOverviewTemplate, GuildTemplate, GuildsListTemplate,
};

pub async fn get_guilds_list(user: Authenticated) -> impl Responder {
    GuildsListTemplate { user: user.clone() }
}

pub async fn get_create_guild_form(user: Authenticated) -> impl Responder {
    CreateGuildFormTemplate {
        user: user.clone(),
        guild: GuildDraft::default(),
        member_search_term: String::new(),
        matched_members: Vec::new(),
        errors: HashMap::default(),
        is_valid: false,
        should_swap_oob: false,
    }
}

#[derive(Deserialize)]
pub struct OptionalGuildIdParam {
    pub guild_id: Option<String>,
}

pub async fn post_guild_form_draft(
    parameters: web::Path<OptionalGuildIdParam>,
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

    if let Some(guild_id) = &parameters.guild_id {
        return EditGuildFormTemplate {
            user: user.clone(),
            guild: GuildDraft {
                name: form.name.clone(),
                members: Vec::new(),
                id: Some(guild_id.to_owned()),
            },
            member_search_term: form.member_search_term.clone(),
            matched_members: members,
            is_valid: errors.is_empty(),
            errors,
            should_swap_oob: true,
        }
        .to_response();
    }

    CreateGuildFormTemplate {
        user: user.clone(),
        guild: GuildDraft {
            name: form.name.clone(),
            members: Vec::new(),
            id: parameters.guild_id.clone(),
        },
        member_search_term: form.member_search_term.clone(),
        matched_members: members,
        is_valid: errors.is_empty(),
        errors,
        should_swap_oob: true,
    }
    .to_response()
}

pub async fn create_guild(
    app: web::Data<Arc<App>>,
    user: Authenticated,
    form: Form<GuildFormDTO>,
) -> impl Responder {
    match form.validate() {
        Err(errors) => {
            let body = CreateGuildFormTemplate {
                user: user.clone(),
                guild: GuildDraft {
                    name: form.name.clone(),
                    members: Vec::new(),
                    id: None,
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

    let created_guild = match app.guilds_service.create_new_guild(form.0, user.0).await {
        Ok(guild) => guild,
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
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
        path: format!("/guilds/{}", created_guild.id),
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

    return HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .insert_header(("HX-Location", location))
        .insert_header(("HX-Trigger", event))
        .finish();
}

pub async fn remove_member() -> impl Responder {
    return HttpResponse::Ok();
}

#[derive(Deserialize)]
pub struct InsertMemberParameters {
    pub member_id: usize,
    pub guild_id: Option<String>,
}

pub async fn insert_new_member(
    user: Authenticated,
    parameters: web::Path<InsertMemberParameters>,
    app: web::Data<Arc<App>>,
    form: Form<GuildFormDTO>,
) -> impl Responder {
    let member_to_insert = match app
        .gitlab_service
        .get_cached_member(&parameters.member_id)
        .await
    {
        Some(member) => member,
        None => return HttpResponse::NotFound().finish(),
    };

    let mut existing_members: Vec<Member> = app
        .gitlab_service
        .get_cached_members_by_ids(&form.member_ids)
        .await;

    if !form.member_ids.contains(&parameters.member_id) {
        existing_members.insert(0, member_to_insert)
    }

    let errors = validator_errors_to_hashmap(form.validate().err());

    if let Some(guild_id) = &parameters.guild_id {
        return EditGuildFormTemplate {
            user: user.to_owned(),
            guild: GuildDraft {
                name: form.name.clone(),
                members: existing_members,
                id: Some(guild_id.to_owned()),
            },
            member_search_term: String::new(),
            matched_members: Vec::new(),
            is_valid: errors.is_empty(),
            errors,
            should_swap_oob: false,
        }
        .to_response();
    }

    CreateGuildFormTemplate {
        user: user.to_owned(),
        guild: GuildDraft {
            name: form.name.clone(),
            members: existing_members,
            id: None,
        },
        member_search_term: String::new(),
        matched_members: Vec::new(),
        is_valid: errors.is_empty(),
        errors,
        should_swap_oob: false,
    }
    .to_response()
}

#[derive(Deserialize)]
pub struct GuildIdParam {
    pub guild_id: String,
}

pub async fn get_guild(user: Authenticated, parameters: web::Path<GuildIdParam>) -> impl Responder {
    GuildTemplate {
        user: user.to_owned(),
        guild_id: parameters.guild_id.to_owned(),
    }
}

pub async fn get_guild_overview(
    user: Authenticated,
    parameters: web::Path<GuildIdParam>,
    app: web::Data<Arc<App>>,
) -> impl Responder {
    let guild = match app
        .guilds_service
        .get_guild(user.clone(), &parameters.guild_id)
        .await
    {
        Ok(Some(guild)) => guild,
        _ => {
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

            let event = HxTriggerEvent::ShowToast {
                level: ToastLevel::Warning,
                message: "Failed to fetch guild".to_string(),
            };

            let event = match serde_json::to_string(&event) {
                Ok(result) => result,
                Err(err) => {
                    error!("{err}");
                    return HttpResponse::InternalServerError().finish();
                }
            };

            return HttpResponse::Ok()
                .insert_header(ContentType(mime::TEXT_HTML))
                .insert_header(("HX-Location", location))
                .insert_header(("HX-Trigger", event))
                .finish();
        }
    };

    let can_edit = user.id == guild.created_by_user.id;

    GuildOverviewTemplate {
        user: user.to_owned(),
        guild,
        can_edit,
    }
    .to_response()
}

pub async fn delete_guild(
    user: Authenticated,
    parameters: web::Path<GuildIdParam>,
    app: web::Data<Arc<App>>,
) -> impl Responder {
    match app
        .guilds_service
        .delete_guild(user.id, &parameters.guild_id)
        .await
    {
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
        _ => {}
    }

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

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Guild was deleted successfully".to_string(),
    };

    let event = match serde_json::to_string(&event) {
        Ok(result) => result,
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .insert_header(("HX-Location", location))
        .insert_header(("HX-Trigger", event))
        .finish()
}

pub async fn get_edit_guild_form(
    user: Authenticated,
    parameters: web::Path<GuildIdParam>,
    app: web::Data<Arc<App>>,
) -> impl Responder {
    let guild = match app
        .guilds_service
        .get_guild(user.to_owned(), &parameters.guild_id)
        .await
    {
        Ok(Some(guild)) => guild,
        Ok(None) => {
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

            let event = HxTriggerEvent::ShowToast {
                level: ToastLevel::Warning,
                message: "Guild was not found".to_string(),
            };

            let event = match serde_json::to_string(&event) {
                Ok(result) => result,
                Err(err) => {
                    error!("{err}");
                    return HttpResponse::InternalServerError().finish();
                }
            };

            return HttpResponse::Ok()
                .insert_header(ContentType(mime::TEXT_HTML))
                .insert_header(("HX-Location", location))
                .insert_header(("HX-Trigger", event))
                .finish();
        }
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    EditGuildFormTemplate {
        user: user.clone(),
        guild: GuildDraft {
            id: Some(guild.id),
            name: guild.name,
            members: guild.members,
        },
        member_search_term: String::new(),
        matched_members: Vec::new(),
        errors: HashMap::default(),
        is_valid: false,
        should_swap_oob: false,
    }
    .to_response()
}

pub async fn update_guild(
    app: web::Data<Arc<App>>,
    user: Authenticated,
    parameters: web::Path<GuildIdParam>,
    form: Form<GuildFormDTO>,
) -> impl Responder {
    match form.validate() {
        Err(errors) => {
            let body = EditGuildFormTemplate {
                user: user.clone(),
                guild: GuildDraft {
                    name: form.name.clone(),
                    members: Vec::new(),
                    id: Some(parameters.guild_id.clone()),
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

    let updated_guild = match app
        .guilds_service
        .update_guild(parameters.guild_id.clone(), form.0, user.0)
        .await
    {
        Ok(guild) => guild,
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Guild was updated".to_string(),
    };

    let event = match serde_json::to_string(&event) {
        Ok(result) => result,
        Err(err) => {
            error!("{err}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let location = Location {
        path: format!("/guilds/{}", updated_guild.id),
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

    return HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .insert_header(("HX-Location", location))
        .insert_header(("HX-Trigger", event))
        .finish();
}
