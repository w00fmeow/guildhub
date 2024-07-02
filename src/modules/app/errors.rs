use actix_web::{error, http::StatusCode, HttpResponse};
use derive_more::Display;
use std::error::Error;

#[derive(Debug, Display)]
pub enum AppError {
    #[display(fmt = "An internal error occurred. Please try again later.")]
    InternalError,
    #[display(fmt = "Validation error")]
    ValidationError(String),
}

impl error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let message = match self {
            AppError::InternalError => "An internal error occurred. Please try again later.",
            AppError::ValidationError(message) => message,
        };

        HttpResponse::build(self.status_code())
            .content_type("application/json")
            .body(format_error_response(message))
    }

    fn status_code(&self) -> StatusCode {
        match *self {
            AppError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
        }
    }
}

impl Error for AppError {}

pub fn format_error_response(message: &str) -> String {
    format!(r#"{{"error":"{}"}}"#, message)
}
