use std::{str::FromStr, sync::Arc};

use anyhow::{bail, Result};
use bson::oid::ObjectId;
use chrono::Utc;
use tracing::error;

use crate::{libs::gitlab_api::gitlab_api::Member, modules::gitlab::GitlabService};

use super::{Guild, GuildFormDTO, GuildsRepository, UpdateGuildPayload};

pub struct GuildsService {
    repository: Arc<GuildsRepository>,
    gitlab_service: Arc<GitlabService>,
}

impl GuildsService {
    pub fn new(repository: Arc<GuildsRepository>, gitlab_service: Arc<GitlabService>) -> Self {
        Self {
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
            created_by_user: created_by_user.clone(),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        };

        let insert_result = self
            .repository
            .insert_guild_document(guild.clone().try_into()?)
            .await?;

        let created_id = insert_result
            .inserted_id
            .as_object_id()
            .ok_or_else(|| anyhow::anyhow!("Failed to convert object id of created document"))?;

        let created_guild = match self
            .get_guild(created_by_user, &created_id.to_hex())
            .await?
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

        Ok(created_guild)
    }

    pub async fn get_guild(&self, user: Member, guild_id: &str) -> Result<Option<Guild>> {
        let guild_id = ObjectId::from_str(guild_id)?;

        let guild_document = match self.repository.get_guild(guild_id, Some(user.id)).await? {
            None => return Ok(None),
            Some(document) => document,
        };

        let members = self
            .gitlab_service
            .get_cached_members_by_ids(&guild_document.member_ids)
            .await;

        Ok(Some(Guild {
            id: guild_document._id.to_hex(),
            name: guild_document.name,
            members,
            created_by_user: user,
            updated_at: guild_document.updated_at.to_chrono(),
            created_at: guild_document.created_at.to_chrono(),
        }))
    }

    pub async fn delete_guild(&self, user_id: usize, guild_id: &str) -> Result<()> {
        let guild_id = ObjectId::from_str(guild_id)?;

        let result = self
            .repository
            .delete_guild(guild_id, Some(user_id))
            .await?;

        if result.deleted_count == 0 {
            bail!("Failed to delete guild")
        }

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
            .update_guild(ObjectId::from_str(&guild_id)?, updated_by_user.id, payload)
            .await?;

        if update_result.modified_count == 0 {
            bail!("Failed to update guild")
        }

        let updated_guild = match self.get_guild(updated_by_user, &guild_id).await? {
            Some(guild) => guild,
            None => {
                error!("Failed to find updated document by id {}", guild_id);

                bail!("Guild document was not found");
            }
        };

        Ok(updated_guild)
    }
}
