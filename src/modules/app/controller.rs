use super::app::App;
use crate::libs::health_checker::Dependency;
use actix_web::{
    cookie::{time::OffsetDateTime, Cookie},
    dev::ServiceResponse,
    http::{header, StatusCode},
    middleware::ErrorHandlerResponse,
    web::{self, Redirect},
    HttpResponse, Responder, Result,
};

use askama_actix::Template;
use oauth2::{AuthorizationCode, CsrfToken};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, warn};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct HealthPayload {
    pub dependencies: Vec<Dependency>,
}

pub async fn health(app: web::Data<Arc<App>>) -> HttpResponse {
    let health_payload = HealthPayload {
        dependencies: app
            .dependencies
            .iter()
            .map(|dependency| dependency.to_dependency())
            .collect(),
    };

    let response = if app.is_healthy() {
        HttpResponse::Ok().json(health_payload)
    } else {
        HttpResponse::ServiceUnavailable().json(health_payload)
    };

    return response;
}

pub async fn index() -> impl Responder {
    // todo if authenticated => redirect to list of guilds | middleware?
    Redirect::to("/login")
}

pub async fn logout(app: web::Data<Arc<App>>) -> impl Responder {
    let cookie = Cookie::build("token", "")
        .path("/")
        .expires(OffsetDateTime::now_utc())
        .finish();

    let body = LoginTemplate {
        gitlab_oath_url: app.gitlab_service.get_oath_url(),
    }
    .to_string();

    return HttpResponse::Ok().cookie(cookie).body(body);
}

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    pub gitlab_oath_url: String,
}

pub async fn login(app: web::Data<Arc<App>>) -> impl Responder {
    LoginTemplate {
        gitlab_oath_url: app.gitlab_service.get_oath_url(),
    }
}

#[derive(Template)]
#[template(path = "pages/not_found.html")]
struct NotFoundTemplate;

pub async fn not_found() -> impl Responder {
    NotFoundTemplate {}
}

#[derive(Template)]
#[template(path = "pages/internal_server_error.html")]
struct InternalError;

pub fn add_internal_server_error_to_response<B>(
    res: ServiceResponse<B>,
) -> Result<ErrorHandlerResponse<B>> {
    let (req, res) = res.into_parts();

    let res = res.set_body(InternalError {}.to_string());

    let res = ServiceResponse::new(req, res)
        .map_into_boxed_body()
        .map_into_right_body();

    Ok(ErrorHandlerResponse::Response(res))
}

pub fn redirect_to_login<B>(mut res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    let response = res.response_mut();

    let mut _status = response.status_mut();

    let mut new_status_code = StatusCode::SEE_OTHER;

    _status = &mut new_status_code;

    let cookie = Cookie::build("token", "")
        .path("/")
        .expires(OffsetDateTime::now_utc())
        .finish();

    response
        .headers_mut()
        .insert(header::LOCATION, header::HeaderValue::from_static("/login"));

    let _ = response.add_cookie(&cookie);

    debug!("redirect_to_login is happening");

    Ok(ErrorHandlerResponse::Response(res.map_into_left_body()))
}

#[derive(Deserialize)]
pub struct GitlabAuthRequest {
    code: String,
    state: String,
}

pub async fn gitlab_auth(
    app: web::Data<Arc<App>>,
    params: web::Query<GitlabAuthRequest>,
) -> impl Responder {
    let code = AuthorizationCode::new(params.code.clone());
    let _state = CsrfToken::new(params.state.clone());

    match app
        .gitlab_service
        .gitlab_api
        .authorize_user_by_access_code(code)
        .await
    {
        Ok(user) => {
            if let Some(user) = app.gitlab_service.get_cached_member(&user.id).await {
                if let Ok(token) = app.auth_service.create_token(user.id) {
                    let cookie = Cookie::build("token", token)
                        .path("/")
                        .secure(true)
                        .http_only(true)
                        .finish();

                    return HttpResponse::Found()
                        .insert_header((header::LOCATION, "/guilds"))
                        .cookie(cookie)
                        .finish();
                }
            } else {
                warn!("User {} tried to login but he is missing from cache, which indicates that he does not belong to group", &user.id)
            }
        }
        Err(err) => {
            error!("{:?}", err);
        }
    }

    HttpResponse::Found()
        .insert_header((header::LOCATION, "/login"))
        .finish()
}
