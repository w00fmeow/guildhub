use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: usize,
    pub iat: usize,
    pub exp: usize,
}

pub struct AuthService {
    expire_in: Duration,
    secret: String,
}

impl AuthService {
    pub fn new(expire_in: Duration, secret: String) -> Self {
        Self { expire_in, secret }
    }

    pub fn create_token(&self, user_id: usize) -> Result<String> {
        let now = Utc::now();
        let iat = now.timestamp() as usize;
        let exp = (now + self.expire_in).timestamp() as usize;
        let claims: TokenClaims = TokenClaims {
            sub: user_id,
            exp,
            iat,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?;

        Ok(token)
    }

    pub fn decode_token<T: Into<String>>(&self, token: T) -> Result<TokenClaims> {
        let decoded = decode::<TokenClaims>(
            &token.into(),
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )?;

        Ok(decoded.claims)
    }

    pub fn refresh_token_if_needed(&self, claims: TokenClaims) -> Result<Option<String>> {
        let issued_at = match DateTime::from_timestamp(claims.iat as i64, 0) {
            Some(issued_at) => issued_at,
            None => return Ok(None),
        };

        let now = Utc::now();
        let diff = now - issued_at;

        if diff.num_hours() > 2 {
            let new_token = self.create_token(claims.sub)?;

            return Ok(Some(new_token));
        }

        Ok(None)
    }
}
