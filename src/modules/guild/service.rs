use std::{str::FromStr, sync::Arc};

use anyhow::{bail, Result};
use bson::oid::ObjectId;
use chrono::Utc;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tracing::error;

use crate::{
    libs::gitlab_api::gitlab_api::Member,
    modules::{gitlab::GitlabService, topic::TopicsService},
};

use super::{
    Guild, GuildEvent, GuildFormDTO, GuildsRepository, UpdateGuildPayload,
};

pub struct GuildsService {
    pub events_channel: (Sender<GuildEvent>, Receiver<GuildEvent>),
    repository: Arc<GuildsRepository>,
    gitlab_service: Arc<GitlabService>,
    topics_service: Arc<TopicsService>,
}

impl GuildsService {
    pub fn new(
        topics_service: Arc<TopicsService>,
        repository: Arc<GuildsRepository>,
        gitlab_service: Arc<GitlabService>,
    ) -> Self {
        Self {
            events_channel: channel::<GuildEvent>(50),
            topics_service,
            repository,
            gitlab_service,
        }
    }

    pub async fn create_new_guild(
        &self,
        form_dto: GuildFormDTO,
        created_by_user: Member,
    ) -> Result<Guild> {
        let members = self
            .gitlab_service
            .get_cached_members_by_ids(&form_dto.member_ids)
            .await;

        let guild = Guild {
            id: ObjectId::new().to_hex(),
            name: form_dto.name,
            members,
            topics_count: 0,
            created_by_user: created_by_user.clone(),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        let insert_result = self
            .repository
            .insert_guild_document(guild.clone().try_into()?)
            .await?;

        let created_id =
            insert_result.inserted_id.as_object_id().ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to convert object id of created guild document"
                )
            })?;

        let created_guild =
            match self.get_guild(created_by_user, &created_id.to_hex()).await?
            {
                Some(guild) => guild,
                None => {
                    error!(
                        "Failed to find created document by id {}",
                        insert_result.inserted_id
                    );

                    bail!("Guild document was not found");
                }
            };

        let _ = self
            .events_channel
            .0
            .send(GuildEvent::Create(created_guild.clone()));

        Ok(created_guild)
    }

    pub async fn get_guild(
        &self,
        user: Member,
        guild_id: &str,
    ) -> Result<Option<Guild>> {
        let guild_id: ObjectId = ObjectId::from_str(guild_id)?;

        let guild_document =
            match self.repository.get_guild(guild_id, Some(user.id)).await? {
                None => return Ok(None),
                Some(document) => document,
            };

        let members = self
            .gitlab_service
            .get_cached_members_by_ids(&guild_document.member_ids)
            .await;

        let mut topics_count = self
            .topics_service
            .get_topics_count_by_guild_ids(vec![guild_id])
            .await?;

        let topics_count =
            topics_count.remove(&guild_id.to_hex()).unwrap_or(0);

        Ok(Some(Guild {
            id: guild_document._id.to_hex(),
            name: guild_document.name,
            topics_count,
            members,
            created_by_user: user,
            updated_at: guild_document.updated_at.to_chrono(),
            created_at: guild_document.created_at.to_chrono(),
        }))
    }

    pub async fn get_guilds(&self, user_id: usize) -> Result<Vec<Guild>> {
        let documents = self.repository.get_guilds(user_id).await?;

        let document_ids =
            documents.iter().map(|document| document._id.clone()).collect();

        let mut all_members_ids: Vec<usize> = Vec::new();

        for document in documents.iter() {
            if !all_members_ids.contains(&document.created_by_user_id) {
                all_members_ids.push(document.created_by_user_id)
            }

            for member_id in document.member_ids.iter() {
                if !all_members_ids.contains(&member_id) {
                    all_members_ids.push(member_id.to_owned())
                }
            }
        }

        let all_members = self
            .gitlab_service
            .get_cached_members_by_ids(&all_members_ids)
            .await;

        let mut all_topics_count = self
            .topics_service
            .get_topics_count_by_guild_ids(document_ids)
            .await?;

        let guilds: Result<Vec<Guild>> = documents
            .into_iter()
            .map(|document| {
                let members = all_members
                    .iter()
                    .filter(|member| document.member_ids.contains(&member.id))
                    .map(|member| member.clone())
                    .collect();

                let created_by_user = all_members
                    .iter()
                    .find(|member| member.id == document.created_by_user_id);

                if created_by_user.is_none() {
                    bail!(format!(
                        "Failed to fetch user {} who created guild {}",
                        document.created_by_user_id,
                        document._id.to_hex()
                    ))
                }

                let topics_count = all_topics_count
                    .remove(&document._id.to_hex())
                    .unwrap_or(0);

                return Ok(Guild {
                    id: document._id.to_hex(),
                    name: document.name,
                    members,
                    topics_count,
                    created_by_user: created_by_user.unwrap().clone(),
                    updated_at: document.updated_at.to_chrono(),
                    created_at: document.created_at.to_chrono(),
                });
            })
            .collect();

        Ok(guilds?)
    }

    pub async fn delete_guild(
        &self,
        user_id: usize,
        guild_id: &str,
    ) -> Result<()> {
        let result = self
            .repository
            .delete_guild(ObjectId::from_str(guild_id)?, Some(user_id))
            .await?;

        if result.deleted_count == 0 {
            bail!("Failed to delete guild")
        }

        let _ = self
            .events_channel
            .0
            .send(GuildEvent::Delete(guild_id.to_owned()));

        Ok(())
    }

    pub async fn update_guild(
        &self,
        guild_id: String,
        form_dto: GuildFormDTO,
        updated_by_user: Member,
    ) -> Result<Guild> {
        let members = self
            .gitlab_service
            .get_cached_members_by_ids(&form_dto.member_ids)
            .await;

        let payload = UpdateGuildPayload {
            name: form_dto.name,
            member_ids: members.into_iter().map(|member| member.id).collect(),
            updated_at: bson::DateTime::now(),
        };

        let update_result = self
            .repository
            .update_guild(
                ObjectId::from_str(&guild_id)?,
                updated_by_user.id,
                payload,
            )
            .await?;

        if update_result.modified_count == 0 {
            bail!("Failed to update guild")
        }

        let updated_guild = match self
            .get_guild(updated_by_user, &guild_id)
            .await?
        {
            Some(guild) => guild,
            None => {
                error!("Failed to find updated document by id {}", guild_id);

                bail!("Guild document was not found");
            }
        };

        let _ = self
            .events_channel
            .0
            .send(GuildEvent::Update(updated_guild.clone()));

        Ok(updated_guild)
    }
}
