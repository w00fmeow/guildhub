use crate::{
    libs::{
        axum::Form, gitlab_api::gitlab_api::Member, htmx::Location,
        validator::validator_errors_to_hashmap,
    },
    modules::{
        app::{
            app::App, user_extractor::Authenticated, AppError,
            Event as AppEvent, HxTriggerEvent, ToastLevel,
        },
        guild::GuildEvent,
        topic::types::{TopicEvent, TopicsListItemTemplate},
    },
};
use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    http::HeaderValue,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse,
    },
};
use axum::{
    http::{header, HeaderMap, StatusCode},
    response::Sse,
};
use futures::Stream;
use mime;
use serde::Deserialize;
use std::{
    collections::HashMap, convert::Infallible, sync::Arc, time::Duration,
};
use tokio::{select, time::sleep};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tracing::{debug, error, info};
use validator::Validate;

use super::{
    CreateGuildFormTemplate, EditGuildFormTemplate, GuildDraft, GuildFormDTO,
    GuildIdParameter, GuildListItemsTemplate, GuildOverviewTemplate,
    GuildTemplate, GuildsListTemplate,
};

pub async fn get_guilds_page(
    Authenticated(user): Authenticated,
) -> impl IntoResponse {
    GuildsListTemplate { user }
}

pub async fn get_guilds_list(
    Authenticated(user): Authenticated,
    State(app): State<Arc<App>>,
) -> Result<impl IntoResponse, AppError> {
    let guilds = app.guilds_service.get_guilds(user.id).await?;

    Ok(GuildListItemsTemplate { guilds }.into_response())
}

pub async fn get_create_guild_form(
    Authenticated(user): Authenticated,
) -> impl IntoResponse {
    CreateGuildFormTemplate {
        user,
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
    Path(parameters): Path<OptionalGuildIdParam>,
    Authenticated(user): Authenticated,
    State(app): State<Arc<App>>,
    Form(form): Form<GuildFormDTO>,
) -> impl IntoResponse {
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
            user,
            guild: GuildDraft {
                name: form.name.clone(),
                members: Vec::new(),
                id: Some(guild_id.to_owned()),
            },
            member_search_term: form.member_search_term,
            matched_members: members,
            is_valid: errors.is_empty(),
            errors,
            should_swap_oob: true,
        }
        .into_response();
    }

    CreateGuildFormTemplate {
        user,
        guild: GuildDraft {
            name: form.name.clone(),
            members: Vec::new(),
            id: parameters.guild_id.clone(),
        },
        member_search_term: form.member_search_term,
        matched_members: members,
        is_valid: errors.is_empty(),
        errors,
        should_swap_oob: true,
    }
    .into_response()
}

pub async fn create_guild(
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
    Form(form): Form<GuildFormDTO>,
) -> Result<impl IntoResponse, AppError> {
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

            return Ok(body.into_response());
        }
        Ok(()) => {}
    };

    let created_guild =
        app.guilds_service.create_new_guild(form, user).await?;

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Guild was created".to_string(),
    };

    let event = HeaderValue::from_str(&serde_json::to_string(&event)?)?;

    let location = Location {
        path: format!("/guilds/{}", created_guild.id),
        target: "#content".to_string(),
        select: "#content".to_string(),
        swap: "outerHTML".to_string(),
    };

    let location = HeaderValue::from_str(&serde_json::to_string(&location)?)?;

    let mut headers = HeaderMap::new();

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime::TEXT_HTML.to_string())?,
    );
    headers.insert("HX-Location", location);
    headers.insert("HX-Trigger", event);

    Ok(headers.into_response())
}

pub async fn remove_member() -> impl IntoResponse {
    return StatusCode::OK;
}

#[derive(Deserialize)]
pub struct InsertMemberParameters {
    pub member_id: usize,
    pub guild_id: Option<String>,
}

pub async fn insert_new_member(
    Authenticated(user): Authenticated,
    Path(parameters): Path<InsertMemberParameters>,
    State(app): State<Arc<App>>,
    Form(form): Form<GuildFormDTO>,
) -> Result<impl IntoResponse, AppError> {
    let member_to_insert = match app
        .gitlab_service
        .get_cached_member(&parameters.member_id)
        .await
    {
        Some(member) => member,
        None => return Err(anyhow!("Failed to find user").into()),
    };

    let mut existing_members: Vec<Member> =
        app.gitlab_service.get_cached_members_by_ids(&form.member_ids).await;

    if !form.member_ids.contains(&parameters.member_id) {
        existing_members.insert(0, member_to_insert)
    }

    let errors = validator_errors_to_hashmap(form.validate().err());

    if let Some(guild_id) = &parameters.guild_id {
        return Ok(EditGuildFormTemplate {
            user,
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
        .into_response());
    }

    Ok(CreateGuildFormTemplate {
        user,
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
    .into_response())
}

pub async fn get_guild(
    Authenticated(user): Authenticated,
    Path(parameters): Path<GuildIdParameter>,
) -> impl IntoResponse {
    GuildTemplate { user, guild_id: parameters.guild_id.to_owned() }
}

pub async fn get_guild_overview(
    Authenticated(user): Authenticated,
    Path(parameters): Path<GuildIdParameter>,
    State(app): State<Arc<App>>,
) -> Result<impl IntoResponse, AppError> {
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
                select: "#content".to_string(),
                swap: "outerHTML".to_string(),
            };

            let location =
                HeaderValue::from_str(&serde_json::to_string(&location)?)?;

            let event = HxTriggerEvent::ShowToast {
                level: ToastLevel::Warning,
                message: "Failed to fetch guild".to_string(),
            };

            let event =
                HeaderValue::from_str(&serde_json::to_string(&event)?)?;

            let mut headers = HeaderMap::new();

            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(&mime::TEXT_HTML.to_string())?,
            );
            headers.insert("HX-Location", location);
            headers.insert("HX-Trigger", event);

            return Ok(headers.into_response());
        }
    };

    Ok(GuildOverviewTemplate {
        can_edit: user.id == guild.created_by_user.id,
        user,
        guild_id: parameters.guild_id,
        guild,
    }
    .into_response())
}

pub async fn delete_guild(
    Authenticated(user): Authenticated,
    Path(parameters): Path<GuildIdParameter>,
    State(app): State<Arc<App>>,
) -> Result<impl IntoResponse, AppError> {
    app.guilds_service.delete_guild(user.id, &parameters.guild_id).await?;

    let location = Location {
        path: "/guilds".to_string(),
        target: "#content".to_string(),
        select: "#content".to_string(),
        swap: "outerHTML".to_string(),
    };

    let location = HeaderValue::from_str(&serde_json::to_string(&location)?)?;

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Guild was deleted successfully".to_string(),
    };

    let event: HeaderValue =
        HeaderValue::from_str(&serde_json::to_string(&event)?)?;

    let mut headers = HeaderMap::new();

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime::TEXT_HTML.to_string())?,
    );
    headers.insert("HX-Location", location);
    headers.insert("HX-Trigger", event);

    return Ok(headers);
}

pub async fn get_edit_guild_form(
    Authenticated(user): Authenticated,
    Path(parameters): Path<GuildIdParameter>,
    State(app): State<Arc<App>>,
) -> Result<impl IntoResponse, AppError> {
    let guild = match app
        .guilds_service
        .get_guild(user.clone(), &parameters.guild_id)
        .await?
    {
        Some(guild) => guild,
        None => {
            let location = Location {
                path: "/guilds".to_string(),
                target: "#content".to_string(),
                select: "#content".to_string(),
                swap: "outerHTML".to_string(),
            };

            let location =
                HeaderValue::from_str(&serde_json::to_string(&location)?)?;

            let event = HxTriggerEvent::ShowToast {
                level: ToastLevel::Warning,
                message: "Guild was not found".to_string(),
            };

            let event: HeaderValue =
                HeaderValue::from_str(&serde_json::to_string(&event)?)?;

            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(&mime::TEXT_HTML.to_string())?,
            );
            headers.insert("HX-Location", location);
            headers.insert("HX-Trigger", event);

            return Ok(headers.into_response());
        }
    };

    Ok(EditGuildFormTemplate {
        user,
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
    .into_response())
}

pub async fn update_guild(
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
    Path(parameters): Path<GuildIdParameter>,
    Form(form): Form<GuildFormDTO>,
) -> Result<impl IntoResponse, AppError> {
    debug!("{form:?}");

    match form.validate() {
        Err(errors) => {
            return Ok(EditGuildFormTemplate {
                user: user,
                guild: GuildDraft {
                    name: form.name,
                    members: Vec::new(),
                    id: Some(parameters.guild_id),
                },
                member_search_term: form.member_search_term,
                matched_members: Vec::new(),
                is_valid: false,
                errors: validator_errors_to_hashmap(Some(errors)),
                should_swap_oob: true,
            }
            .into_response())
        }
        Ok(()) => {}
    };

    let updated_guild = app
        .guilds_service
        .update_guild(parameters.guild_id, form, user)
        .await?;

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Guild was updated".to_string(),
    };

    let event: HeaderValue =
        HeaderValue::from_str(&serde_json::to_string(&event)?)?;

    let location = Location {
        path: format!("/guilds/{}", updated_guild.id),
        target: "#content".to_string(),
        select: "#content".to_string(),
        swap: "outerHTML".to_string(),
    };

    let location: HeaderValue =
        HeaderValue::from_str(&serde_json::to_string(&location)?)?;

    let mut headers = HeaderMap::new();

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime::TEXT_HTML.to_string())?,
    );
    headers.insert("HX-Location", location);
    headers.insert("HX-Trigger", event);

    return Ok(headers.into_response());
}

pub async fn subscribe_to_events(
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
    Path(parameters): Path<GuildIdParameter>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("User {} subscribed to events", &user.id);

    let mut app_events_receiver = app.events_channel.0.subscribe();

    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(10);

    let stream = ReceiverStream::<Event>::new(rx).map(|evt| Ok(evt));

    tokio::spawn(async move {
        select! {
            _ = async {
                loop {
                    match tx.send(Event::default().data("ping").event("ping").id("ping")).await {
                        Ok(()) => {
                            sleep(Duration::from_secs(5)).await;
                        }
                        Err(err) => {
                            debug!("Dropping stale client {}. Error: {err}", &user.id);
                            break;
                        }
                    }
                };
            }=>{
                info!("User {} disconnected", &user.id);
            }
            _ = async {
               loop {
                    match app_events_receiver.recv().await {
                        Ok(event) => {
                            debug!("new event {event:?}");
                            match event {
                                AppEvent::Guild(GuildEvent::Update(guild)) => {
                                    if guild.id == parameters.guild_id {
                                        let _ = tx.send(Event::default().data(" ").event("guild-updated")).await;
                                    }
                                }
                                AppEvent::Topic(TopicEvent::Update(topic)) => {
                                    if topic.guild_id == parameters.guild_id {
                                        let _ = tx.send(Event::default().data(" ").event(format!("topic-updated-{}", topic.id))).await;
                                    }
                                }
                                AppEvent::Topic(TopicEvent::Delete(topic)) => {
                                    if topic.guild_id == parameters.guild_id {
                                        let _ = tx.send(Event::default().data(" ").event(format!("topic-deleted-{}", topic.id))).await;
                                    }
                                }
                                AppEvent::Topic(TopicEvent::Create(topic)) => {
                                    if topic.guild_id == parameters.guild_id {
                                        match app.topics_service.map_topic_with_user(topic, user.id).await {
                                            Ok(topic) => {
                                                let topic_item = TopicsListItemTemplate { topic };
                                                let _ = tx.send(Event::default().data(topic_item.to_string()).event("topic-created")).await;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                AppEvent::Topic(TopicEvent::OrderChange(ids)) => {
                                    debug!("ids: {ids:?}");

                                     match serde_json::to_string(&ids) {
                                        Ok(data) => {
                                            debug!("data {}", data);

                                            let _ = tx.send(Event::default().data(data).event("topics-order-changed")).await;
                                        }
                                        _ => {}
                                     }

                                }
                                    _ => {}
                            }
                        }
                        Err(err) => {
                            error!("{err}");
                        }
                    }
                }
            }=>{
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
