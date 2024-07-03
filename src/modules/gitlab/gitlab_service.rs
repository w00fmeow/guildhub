use anyhow::Result;
use futures::future::join_all;
use moka::future::Cache;
use oauth2::{CsrfToken, Scope};
use std::time::Duration;
use tokio::time::sleep;
use tracing::error;

use crate::libs::gitlab_api::{gitlab_api::Member, GitlabApi};

pub struct GitlabService {
    pub gitlab_api: GitlabApi,
    pub group_id: String,
    cache: Cache<usize, Member>,
}

impl GitlabService {
    pub fn new(gitlab_api: GitlabApi, group_id: String, cache_ttl: Duration) -> Self {
        let cache: Cache<usize, Member> = Cache::builder().time_to_live(cache_ttl).build();

        Self {
            gitlab_api,
            group_id,
            cache,
        }
    }

    pub async fn fetch_all_group_members(&self) -> Result<Vec<Member>> {
        self.gitlab_api.get_all_group_members(&self.group_id).await
    }

    pub async fn insert_member_into_cache(&self, member: Member) {
        self.cache.insert(member.id, member).await
    }

    pub async fn refresh_members_cache(&self) -> Result<()> {
        let members = self.fetch_all_group_members().await?;

        let insert_futures = members
            .into_iter()
            .map(|member| self.cache.insert(member.id, member));

        join_all(insert_futures).await;

        Ok(())
    }

    pub async fn refresh_cache_loop(&self, interval: Duration) {
        loop {
            match self.refresh_members_cache().await {
                Ok(()) => {
                    sleep(interval).await;
                }
                Err(err) => {
                    error!("Failed to refresh members cache: {err}. Retrying in 5 sec...");
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    pub async fn get_cached_member(&self, user_id: &usize) -> Option<Member> {
        let member = self.cache.get(user_id).await;

        member
    }

    pub async fn get_all_cached_members(&self) -> Vec<Member> {
        let members: Vec<Member> = self.cache.iter().map(|(_, member)| member).collect();

        members
    }

    pub fn get_oath_url(&self) -> String {
        let (auth_url, _) = self
            .gitlab_api
            .oath
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("read_user".to_string()))
            .url();

        auth_url.to_string()
    }

    pub async fn get_cached_members_by_ids(&self, user_ids: &Vec<usize>) -> Vec<Member> {
        let members = user_ids.iter().map(|id: &usize| self.get_cached_member(id));

        let members = join_all(members).await;

        let members: Vec<Member> = members
            .into_iter()
            .filter(|member| member.is_some())
            .map(|member| member.unwrap())
            .collect();

        return members;
    }
}
