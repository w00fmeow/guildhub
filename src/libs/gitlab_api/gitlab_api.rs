use anyhow::Result;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AccessToken, AuthUrl, AuthorizationCode, ClientId, ClientSecret, RedirectUrl, TokenResponse,
    TokenUrl,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct GitlabApi {
    pub domain: String,
    pub private_token: String,
    pub oath: BasicClient,
    http_client: Client,
}

impl GitlabApi {
    pub fn new(
        private_token: String,
        domain: String,
        client_id: String,
        client_secret: String,
        redirect_url: String,
    ) -> Self {
        let oath = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new(format!("https://{}/oauth/authorize", &domain,))
                .expect("To be able to create auth url"),
            Some(
                TokenUrl::new(format!("https://{}/oauth/token", &domain,))
                    .expect("To be able to create token url"),
            ),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url).expect("Invalid redirect URL"));

        Self {
            domain,
            private_token,
            http_client: Client::new(),
            oath,
        }
    }

    fn get_base_api_url(&self) -> String {
        format!("https://{}/api/v4/", self.domain)
    }

    pub async fn get_group_members(
        &self,
        group_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<Vec<Member>> {
        let url = format!(
            "{}/groups/{group_id}/members/all?page={page}&per_page={page_size}&private_token={}",
            self.get_base_api_url(),
            self.private_token
        );

        let members = self
            .http_client
            .get(url)
            .send()
            .await?
            .json::<Vec<Member>>()
            .await?;

        Ok(members)
    }

    pub async fn get_all_group_members(&self, group_id: &str) -> Result<Vec<Member>> {
        let mut members = Vec::new();

        let page_size = 100;

        let mut current_page = 1;

        loop {
            let new_members = self
                .get_group_members(group_id, current_page, page_size)
                .await?;

            current_page += 1;

            let new_members_count = new_members.len();

            members.extend(new_members);

            if new_members_count < page_size {
                break;
            }
        }

        Ok(members)
    }

    pub async fn get_user_by_access_token(&self, access_token: &AccessToken) -> Result<Member> {
        let url = format!(
            "{}/user?access_token={}",
            self.get_base_api_url(),
            access_token.secret()
        );

        let member = self
            .http_client
            .get(url)
            .send()
            .await?
            .json::<Member>()
            .await?;

        Ok(member)
    }

    pub async fn authorize_user_by_access_code(&self, code: AuthorizationCode) -> Result<Member> {
        let token = self
            .oath
            .exchange_code(code)
            .request_async(async_http_client)
            .await?;

        let user = self.get_user_by_access_token(token.access_token()).await?;

        Ok(user)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Member {
    pub id: usize,
    pub username: String,
    pub name: String,
    pub avatar_url: String,
}

impl Member {
    pub fn default() -> Member {
        Member {
            id: 000,
            username: "default".to_string(),
            name: "Default Test".to_string(),
            avatar_url: "/static/images/user-avatar.png".to_string(),
        }
    }
}
