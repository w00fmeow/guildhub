use super::{app::App, user_extractor::MaybeAuthenticated, AppError};
use crate::libs::health_checker::Dependency;

use axum::http::{header::SET_COOKIE, HeaderValue};

use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::Cookie;
use cookie::{time::OffsetDateTime, CookieBuilder};
use oauth2::{AuthorizationCode, CsrfToken};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, warn};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct HealthPayload {
    pub dependencies: Vec<Dependency>,
}

pub async fn health(State(app): State<Arc<App>>) -> impl IntoResponse {
    let health_payload = HealthPayload {
        dependencies: app
            .dependencies
            .iter()
            .map(|dependency| dependency.to_dependency())
            .collect(),
    };

    let response = if app.is_healthy() {
        (StatusCode::OK, Json(health_payload))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(health_payload))
    };

    return response;
}

pub async fn index(
    MaybeAuthenticated(user): MaybeAuthenticated,
) -> impl IntoResponse {
    if user.is_some() {
        return Redirect::to("/guilds");
    }

    Redirect::to("/login")
}

pub async fn logout(
    State(app): State<Arc<App>>,
) -> Result<impl IntoResponse, AppError> {
    let cookie = build_auth_cookie("").expires(OffsetDateTime::now_utc());

    let mut response =
        LoginTemplate { gitlab_oath_url: app.gitlab_service.get_oath_url() }
            .into_response();

    let cookie = HeaderValue::from_str(&cookie.to_string())?;

    response.headers_mut().insert(SET_COOKIE, cookie);

    return Ok(response);
}

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    pub gitlab_oath_url: String,
}

pub async fn login(
    State(app): State<Arc<App>>,
    MaybeAuthenticated(user): MaybeAuthenticated,
) -> impl IntoResponse {
    if user.is_some() {
        return Redirect::temporary("/guilds").into_response();
    }

    LoginTemplate { gitlab_oath_url: app.gitlab_service.get_oath_url() }
        .into_response()
}

#[derive(Template)]
#[template(path = "pages/not_found.html")]
struct NotFoundTemplate;

pub async fn not_found() -> impl IntoResponse {
    NotFoundTemplate {}
}

#[derive(Template)]
#[template(path = "pages/internal_server_error.html")]
struct InternalError;

#[derive(Deserialize)]
pub struct GitlabAuthRequest {
    code: String,
    state: String,
}

pub fn build_auth_cookie(token: &str) -> CookieBuilder {
    Cookie::build(("token", token)).path("/").secure(true).http_only(true)
}

pub async fn gitlab_auth(
    State(app): State<Arc<App>>,
    Query(params): Query<GitlabAuthRequest>,
) -> Result<impl IntoResponse, AppError> {
    let code = AuthorizationCode::new(params.code.clone());
    let _state = CsrfToken::new(params.state.clone());

    match app
        .gitlab_service
        .gitlab_api
        .authorize_user_by_access_code(code)
        .await
    {
        Ok(user) => {
            if let Some(user) =
                app.gitlab_service.get_cached_member(&user.id).await
            {
                if let Ok(token) = app.auth_service.create_token(user.id) {
                    let cookie = build_auth_cookie(&token);

                    let mut response =
                        Redirect::temporary("/guilds").into_response();

                    response.headers_mut().insert(
                        SET_COOKIE,
                        HeaderValue::from_str(&cookie.to_string())?,
                    );

                    return Ok(response);
                }
            } else {
                warn!("User {} tried to login but he is missing from cache, which indicates that he does not belong to group", &user.id)
            }
        }
        Err(err) => {
            error!("{:?}", err);
        }
    }

    Ok(Redirect::temporary("/login").into_response())
}
