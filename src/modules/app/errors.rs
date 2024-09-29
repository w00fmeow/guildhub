use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::error;

pub struct AppError(pub anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!("{:?}", self.0);

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            // todo: html template
            "Something went wrong",
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
