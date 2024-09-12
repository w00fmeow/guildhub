use crate::{
    libs::{htmx::Location, validator::validator_errors_to_hashmap},
    modules::{
        app::{app::App, user_extractor::Authenticated, AppError, HxTriggerEvent, ToastLevel},
        guild::GuildIdParameter,
    },
};
use anyhow::anyhow;
use askama_axum::IntoResponse;
use axum::http::{header, HeaderValue};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Form,
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use validator::Validate;

use super::constants::TOPICS_LIMIT;
use super::types::{
    CreateTopicTemplate, EditTopicTemplate, PaginationParameters, TopicDraft, TopicFormDTO,
    TopicsListItemTemplate, TopicsListTemplate, VoteTopicResult,
};

#[derive(Deserialize)]
pub struct PaginationQueryParameters {
    pub page: usize,
}

pub async fn get_topics_list(
    Authenticated(user): Authenticated,
    State(app): State<Arc<App>>,
    Path(parameters): Path<GuildIdParameter>,
    Query(PaginationQueryParameters { page }): Query<PaginationQueryParameters>,
) -> Result<impl IntoResponse, AppError> {
    let topics = app
        .topics_service
        .get_topics_by_guild_id(
            user.id,
            parameters.guild_id.as_str(),
            PaginationParameters {
                skip: page * TOPICS_LIMIT - TOPICS_LIMIT,
                limit: TOPICS_LIMIT,
            },
        )
        .await?;

    Ok(TopicsListTemplate {
        current_page: page,
        guild_id: parameters.guild_id,
        has_more_topics: topics.len() >= TOPICS_LIMIT,
        topics,
    })
}

pub async fn get_create_topic_form(
    Authenticated(user): Authenticated,
    Path(parameters): Path<GuildIdParameter>,
) -> impl IntoResponse {
    CreateTopicTemplate {
        user,
        topic: TopicDraft {
            id: None,
            guild_id: parameters.guild_id,
            text: String::new(),
            will_be_presented_by_the_creator: true,
        },
        errors: HashMap::default(),
        is_valid: false,
        should_swap_oob: false,
    }
}

#[derive(Deserialize)]
pub struct DraftParameters {
    pub guild_id: String,
    pub topic_id: Option<String>,
}

pub async fn post_topic_form_draft(
    Path(parameters): Path<DraftParameters>,
    Authenticated(user): Authenticated,
    Form(form): Form<TopicFormDTO>,
) -> impl IntoResponse {
    let errors = validator_errors_to_hashmap(form.validate().err());

    if let Some(topic_id) = parameters.topic_id {
        return EditTopicTemplate {
            user,
            topic: TopicDraft {
                id: Some(topic_id),
                guild_id: parameters.guild_id,
                text: form.text,
                will_be_presented_by_the_creator: form
                    .will_be_presented_by_the_creator
                    .is_some_and(|val| val == true),
            },
            is_valid: errors.is_empty(),
            errors,
            should_swap_oob: true,
        }
        .into_response();
    }

    CreateTopicTemplate {
        user,
        topic: TopicDraft {
            id: None,
            guild_id: parameters.guild_id,
            text: form.text,
            will_be_presented_by_the_creator: form
                .will_be_presented_by_the_creator
                .is_some_and(|val| val == true),
        },
        is_valid: errors.is_empty(),
        errors,
        should_swap_oob: true,
    }
    .into_response()
}

pub async fn create_topic(
    Path(parameters): Path<DraftParameters>,
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
    Form(form): Form<TopicFormDTO>,
) -> Result<impl IntoResponse, AppError> {
    match form.validate() {
        Err(errors) => {
            return Ok(CreateTopicTemplate {
                user,
                topic: TopicDraft {
                    id: None,
                    guild_id: parameters.guild_id,
                    text: form.text,
                    will_be_presented_by_the_creator: form
                        .will_be_presented_by_the_creator
                        .is_some_and(|val| val == true),
                },
                is_valid: false,
                errors: validator_errors_to_hashmap(Some(errors)),
                should_swap_oob: true,
            }
            .into_response())
        }
        Ok(()) => {}
    };

    app.topics_service
        .create_topic(form, &parameters.guild_id, user.id)
        .await?;

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Topic was created".to_string(),
    };

    let event = HeaderValue::from_str(&serde_json::to_string(&event)?)?;

    let location = Location {
        path: format!("/guilds/{}", &parameters.guild_id),
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

    return Ok(headers.into_response());
}

#[derive(Deserialize)]
pub struct TopicParameters {
    pub guild_id: String,
    pub topic_id: String,
}

pub async fn upvote_topic(
    Path(parameters): Path<TopicParameters>,
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
) -> Result<impl IntoResponse, AppError> {
    let VoteTopicResult {
        topic,
        previously_voted: _,
    } = app
        .topics_service
        .upvote_topic(parameters.guild_id, parameters.topic_id, user.id)
        .await?;

    Ok(TopicsListItemTemplate { topic })
}

pub async fn remove_vote_from_topic(
    Path(parameters): Path<TopicParameters>,
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
) -> Result<impl IntoResponse, AppError> {
    let topic = app
        .topics_service
        .remove_vote_from_topic(&parameters.guild_id, &parameters.topic_id, user.id)
        .await?;

    Ok(TopicsListItemTemplate { topic })
}

pub async fn get_topic_card(
    State(app): State<Arc<App>>,
    Path(parameters): Path<TopicParameters>,
    Authenticated(user): Authenticated,
) -> Result<impl IntoResponse, AppError> {
    let topic = match app
        .topics_service
        .get_topic(&parameters.topic_id, user.id)
        .await
    {
        Ok(Some(topic)) => topic,
        _ => return Err(anyhow!("Failed to find topic").into()),
    };

    Ok(TopicsListItemTemplate { topic })
}

pub async fn get_edit_topic_form(
    State(app): State<Arc<App>>,
    Path(parameters): Path<TopicParameters>,
    Authenticated(user): Authenticated,
) -> Result<impl IntoResponse, AppError> {
    let topic = match app
        .topics_service
        .get_topic(&parameters.topic_id, user.id)
        .await
    {
        Ok(Some(topic)) => topic,
        _ => return Err(anyhow!("Failed to find topic").into()),
    };

    Ok(EditTopicTemplate {
        user,
        topic: TopicDraft {
            id: Some(topic.id),
            guild_id: topic.guild_id,
            text: topic.text,
            will_be_presented_by_the_creator: topic.will_be_presented_by_the_creator,
        },
        is_valid: false,
        errors: HashMap::new(),
        should_swap_oob: false,
    }
    .into_response())
}

pub async fn update_topic(
    Path(parameters): Path<TopicParameters>,
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
    Form(form): Form<TopicFormDTO>,
) -> Result<impl IntoResponse, AppError> {
    match form.validate() {
        Err(errors) => {
            return Ok(EditTopicTemplate {
                user,
                topic: TopicDraft {
                    id: Some(parameters.topic_id),
                    guild_id: parameters.guild_id,
                    text: form.text,
                    will_be_presented_by_the_creator: form
                        .will_be_presented_by_the_creator
                        .is_some_and(|val| val == true),
                },
                is_valid: false,
                errors: validator_errors_to_hashmap(Some(errors)),
                should_swap_oob: true,
            }
            .into_response());
        }
        Ok(()) => {}
    };

    app.topics_service
        .update_topic(form, &parameters.topic_id, user.id)
        .await?;

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Topic was updated".to_string(),
    };

    let event = HeaderValue::from_str(&serde_json::to_string(&event)?)?;

    let location = Location {
        path: format!("/guilds/{}", &parameters.guild_id),
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

    return Ok(headers.into_response());
}

pub async fn delete_topic(
    Path(parameters): Path<TopicParameters>,
    State(app): State<Arc<App>>,
    Authenticated(user): Authenticated,
) -> Result<impl IntoResponse, AppError> {
    app.topics_service
        .delete_topic(&parameters.topic_id, user.id)
        .await?;

    let event = HxTriggerEvent::ShowToast {
        level: ToastLevel::Info,
        message: "Topic was deleted".to_string(),
    };

    let event = HeaderValue::from_str(&serde_json::to_string(&event)?)?;

    let mut headers = HeaderMap::new();

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime::TEXT_HTML.to_string())?,
    );

    headers.insert("HX-Trigger", event);

    return Ok((StatusCode::OK, headers).into_response());
}
