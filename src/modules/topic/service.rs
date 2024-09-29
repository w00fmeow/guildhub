use std::{collections::HashMap, str::FromStr, sync::Arc};

use anyhow::{bail, Result};
use bson::{oid::ObjectId, DateTime};
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tracing::error;

use crate::{
    libs::gitlab_api::gitlab_api::Member,
    modules::{gitlab::GitlabService, guild::Guild},
};

use super::{
    repository::TopicsRepository,
    types::{
        PaginationParameters, Topic, TopicEvent, TopicFormDTO,
        TopicPersonalized, TopicStatus, VoteTopicResult,
    },
    PartialTopicDocument, TopicDocument,
};

pub struct TopicsService {
    pub events_channel: (Sender<TopicEvent>, Receiver<TopicEvent>),
    gitlab_service: Arc<GitlabService>,
    repository: Arc<TopicsRepository>,
}

impl TopicsService {
    pub fn new(
        gitlab_service: Arc<GitlabService>,
        repository: Arc<TopicsRepository>,
    ) -> Self {
        Self {
            events_channel: channel::<TopicEvent>(50),
            gitlab_service,
            repository,
        }
    }

    pub async fn get_topics_count_by_guild_ids(
        &self,
        guild_ids: Vec<ObjectId>,
    ) -> Result<HashMap<String, usize>> {
        let aggregation_results =
            self.repository.get_topics_count_by_guild_ids(guild_ids).await?;

        let mut result_map = HashMap::new();

        for aggregation_result in aggregation_results {
            result_map.insert(
                aggregation_result.guild_id.to_hex(),
                aggregation_result.count,
            );
        }

        Ok(result_map)
    }

    pub async fn get_topics_by_guild_id(
        &self,
        user_id: usize,
        guild_id: &str,
        pagination: PaginationParameters,
        guild: Guild,
        status: TopicStatus,
    ) -> Result<Vec<TopicPersonalized>> {
        let guild_id = ObjectId::from_str(guild_id)?;

        let documents = self
            .repository
            .get_guild_topics(
                pagination,
                PartialTopicDocument {
                    guild_id: Some(guild_id),
                    status: Some(status),
                    text: None,
                    will_be_presented_by_the_creator: None,
                    updated_at: None,
                    upvoted_by_users_ids: None,
                },
            )
            .await?;

        let mut result = Vec::with_capacity(documents.len());

        let mut all_members_ids: Vec<usize> = Vec::new();

        for document in documents.iter() {
            if !all_members_ids.contains(&document.created_by_user_id) {
                all_members_ids.push(document.created_by_user_id)
            }

            for member_id in document.upvoted_by_users_ids.iter() {
                if !all_members_ids.contains(&member_id) {
                    all_members_ids.push(member_id.to_owned())
                }
            }
        }

        let all_members = self
            .gitlab_service
            .get_cached_members_by_ids(&all_members_ids)
            .await;

        let is_current_user_created_guild =
            guild.created_by_user.id == user_id;

        for document in documents {
            let document_id = document._id.to_hex();

            let created_by_user = all_members
                .iter()
                .find(|member| member.id == document.created_by_user_id);

            if created_by_user.is_none() {
                bail!(format!(
                    "Failed to fetch user {} who created topic {}",
                    &document.created_by_user_id, &document_id
                ))
            };

            let is_upvoted_by_current_user =
                document.upvoted_by_users_ids.contains(&user_id);
            let is_created_by_current_user =
                document.created_by_user_id == user_id;

            let created_by_user = created_by_user.unwrap().clone();

            let upvoted_by_users: Result<Vec<Member>> = document
                .upvoted_by_users_ids
                .iter()
                .map(|user_id| {
                    let user = all_members
                        .iter()
                        .find(|member| &member.id == user_id);

                    if user.is_none() {
                        bail!(format!(
                            "Failed to fetch user {} who upvoted topic {}",
                            &user_id, &document_id
                        ))
                    };

                    return Ok(user.unwrap().clone());
                })
                .collect();

            let topic = TopicPersonalized {
                id: document._id.to_hex(),
                guild_id: document.guild_id.to_hex(),
                text: document.text,
                is_status_archived: document.status == TopicStatus::Archived,
                status: document.status,
                will_be_presented_by_the_creator: document
                    .will_be_presented_by_the_creator,
                created_by_user,
                can_change_status: is_current_user_created_guild,
                can_delete: is_current_user_created_guild
                    || is_created_by_current_user,
                can_edit: is_current_user_created_guild
                    || is_created_by_current_user,
                is_upvoted_by_current_user,
                upvoted_by_users: upvoted_by_users?,
                updated_at: document.updated_at.to_chrono(),
                created_at: document.created_at.to_chrono(),
            };
            result.push(topic)
        }

        Ok(result)
    }

    pub async fn create_topic(
        &self,
        form: TopicFormDTO,
        guild: &Guild,
        user_id: usize,
    ) -> Result<TopicPersonalized> {
        let guild_id = ObjectId::from_str(&guild.id)?;

        let document_to_insert = TopicDocument {
            _id: ObjectId::new(),
            guild_id: guild_id.clone(),
            text: form.text,
            status: TopicStatus::Created,
            will_be_presented_by_the_creator: form
                .will_be_presented_by_the_creator
                .is_some_and(|val| val == true),
            created_by_user_id: user_id,
            upvoted_by_users_ids: vec![],
            updated_at: bson::DateTime::now(),
            created_at: bson::DateTime::now(),
        };

        let insert_result =
            self.repository.insert_topic_document(document_to_insert).await?;

        let created_id =
            insert_result.inserted_id.as_object_id().ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to convert object id of created topic document"
                )
            })?;

        let created_topic = match self
            .get_topic(&created_id.to_hex(), user_id, guild)
            .await?
        {
            Some(topic) => topic,
            None => {
                error!(
                    "Failed to find created document by id {}",
                    insert_result.inserted_id
                );

                bail!("Topic document was not found");
            }
        };

        let _ = self
            .events_channel
            .0
            .send(TopicEvent::Create(created_topic.clone().into()));

        let topic_ids =
            self.repository.get_topic_ids_sorted(&guild_id).await?;

        let _ = self.events_channel.0.send(TopicEvent::OrderChange(topic_ids));

        Ok(created_topic)
    }

    pub async fn get_topic(
        &self,
        id: &str,
        user_id: usize,
        guild: &Guild,
    ) -> Result<Option<TopicPersonalized>> {
        let document =
            match self.repository.get_topic(ObjectId::from_str(id)?).await? {
                Some(document) => document,
                None => return Ok(None),
            };

        let mapped_topic =
            self.map_topic_with_user(guild, document.into(), user_id).await?;

        Ok(Some(mapped_topic))
    }

    pub async fn map_topic_with_user(
        &self,
        guild: &Guild,
        topic: Topic,
        user_id: usize,
    ) -> Result<TopicPersonalized> {
        let mut all_members_ids: Vec<usize> = Vec::new();

        if !all_members_ids.contains(&topic.created_by_user_id) {
            all_members_ids.push(topic.created_by_user_id)
        }

        for member_id in topic.upvoted_by_users_ids.iter() {
            if !all_members_ids.contains(&member_id) {
                all_members_ids.push(member_id.to_owned())
            }
        }
        let all_members = self
            .gitlab_service
            .get_cached_members_by_ids(&all_members_ids)
            .await;

        let created_by_user = all_members
            .iter()
            .find(|member| member.id == topic.created_by_user_id);

        if created_by_user.is_none() {
            bail!(format!(
                "Failed to fetch user {} who created topic {}",
                &topic.created_by_user_id, &topic.id
            ))
        };

        let is_current_user_created_guild =
            guild.created_by_user.id == user_id;

        let is_upvoted_by_current_user =
            topic.upvoted_by_users_ids.contains(&user_id);
        let is_created_by_current_user = topic.created_by_user_id == user_id;

        let created_by_user = created_by_user.unwrap().clone();

        let upvoted_by_users: Result<Vec<Member>> = topic
            .upvoted_by_users_ids
            .iter()
            .map(|user_id| {
                let user =
                    all_members.iter().find(|member| &member.id == user_id);

                if user.is_none() {
                    bail!(format!(
                        "Failed to fetch user {} who upvoted topic {}",
                        &user_id, &topic.id
                    ))
                };

                return Ok(user.unwrap().clone());
            })
            .collect();

        let topic = TopicPersonalized {
            id: topic.id,
            guild_id: topic.guild_id,
            text: topic.text,
            is_status_archived: topic.status == TopicStatus::Archived,
            status: topic.status,
            will_be_presented_by_the_creator: topic
                .will_be_presented_by_the_creator,
            created_by_user,
            can_change_status: is_current_user_created_guild,
            can_delete: is_current_user_created_guild
                || is_created_by_current_user,
            can_edit: is_current_user_created_guild
                || is_created_by_current_user,
            is_upvoted_by_current_user,
            upvoted_by_users: upvoted_by_users?,
            updated_at: topic.updated_at,
            created_at: topic.created_at,
        };

        Ok(topic)
    }

    pub async fn upvote_topic(
        &self,
        guild: &Guild,
        id: String,
        user_id: usize,
    ) -> Result<VoteTopicResult> {
        let guild_id = ObjectId::from_str(&guild.id)?;

        let previously_voted = match self
            .repository
            .get_upvoted_topic_by_guild_id(&guild_id, user_id)
            .await?
        {
            Some(document) => {
                let _ = self
                    .events_channel
                    .0
                    .send(TopicEvent::Update(document.clone().into()));

                Some(
                    self.map_topic_with_user(guild, document.into(), user_id)
                        .await?,
                )
            }
            None => None,
        };

        let _ = self
            .repository
            .remove_user_vote_by_guild_id(&guild_id, user_id)
            .await?;

        let _ = self
            .repository
            .upvote_topic(ObjectId::from_str(&id)?, user_id)
            .await?;

        let topic = self.get_topic(&id, user_id, guild).await?;

        let topic_ids =
            self.repository.get_topic_ids_sorted(&guild_id).await?;

        match topic {
            Some(topic) => {
                let _ = self
                    .events_channel
                    .0
                    .send(TopicEvent::Update(topic.clone().into()));

                let _ = self
                    .events_channel
                    .0
                    .send(TopicEvent::OrderChange(topic_ids));

                Ok(VoteTopicResult { previously_voted, topic })
            }
            None => bail!("Failed to fetch topic"),
        }
    }

    pub async fn remove_vote_from_topic(
        &self,
        guild: &Guild,
        id: &str,
        user_id: usize,
    ) -> Result<TopicPersonalized> {
        let guild_id = ObjectId::from_str(&guild.id)?;
        let _ = self
            .repository
            .remove_user_vote_by_guild_id(&guild_id, user_id)
            .await?;

        let topic = self.get_topic(&id, user_id, guild).await?;

        let topic_ids =
            self.repository.get_topic_ids_sorted(&guild_id).await?;

        match topic {
            Some(topic) => {
                let _ = self
                    .events_channel
                    .0
                    .send(TopicEvent::Update(topic.clone().into()));

                let _ = self
                    .events_channel
                    .0
                    .send(TopicEvent::OrderChange(topic_ids));

                Ok(topic)
            }
            None => bail!("Failed to fetch topic"),
        }
    }

    pub async fn update_topic(
        &self,
        form: TopicFormDTO,
        id: &str,
        user_id: usize,
        guild: &Guild,
    ) -> Result<TopicPersonalized> {
        let result = self
            .repository
            .update_topic(
                ObjectId::from_str(id)?,
                PartialTopicDocument {
                    guild_id: None,
                    text: Some(form.text),
                    will_be_presented_by_the_creator: Some(
                        form.will_be_presented_by_the_creator
                            .is_some_and(|val| val == true),
                    ),
                    updated_at: Some(DateTime::now()),
                    status: None,
                    upvoted_by_users_ids: Some(vec![]),
                },
            )
            .await?;

        if result.modified_count != 1 {
            bail!("Failed to update topic {id}")
        };

        let updated_topic = match self.get_topic(id, user_id, guild).await? {
            Some(topic) => topic,
            None => {
                error!("Failed to find updated document by id {}", id);

                bail!("Updated topic document was not found");
            }
        };

        let _ = self
            .events_channel
            .0
            .send(TopicEvent::Update(updated_topic.clone().into()));

        let topic_ids = self
            .repository
            .get_topic_ids_sorted(&ObjectId::from_str(
                &updated_topic.guild_id,
            )?)
            .await?;

        let _ = self.events_channel.0.send(TopicEvent::OrderChange(topic_ids));

        Ok(updated_topic)
    }

    pub async fn delete_topic(
        &self,
        id: &str,
        user_id: usize,
        guild: &Guild,
    ) -> Result<Topic> {
        let topic = match self.get_topic(id, user_id, guild).await? {
            Some(topic) => topic,
            None => bail!("Failed to find this topic"),
        };

        self.repository
            .delete_topic(ObjectId::from_str(id)?, Some(user_id))
            .await?;

        let _ = self
            .events_channel
            .0
            .send(TopicEvent::Delete(topic.clone().into()));

        Ok(topic.into())
    }

    pub async fn change_topic_status(
        &self,
        id: &str,
        user_id: usize,
        new_status: TopicStatus,
        guild: &Guild,
    ) -> Result<TopicPersonalized> {
        let topic = match self.get_topic(id, user_id, guild).await? {
            Some(topic) => topic,
            None => bail!("Failed to find this topic"),
        };

        if topic.status == new_status {
            return Ok(topic.into());
        }

        let upvoted_by_users_ids = match new_status {
            TopicStatus::Archived => None,
            _ => Some(vec![]),
        };

        let result = self
            .repository
            .update_topic(
                ObjectId::from_str(id)?,
                PartialTopicDocument {
                    guild_id: None,
                    updated_at: Some(DateTime::now()),
                    status: Some(new_status),
                    text: None,
                    will_be_presented_by_the_creator: None,
                    upvoted_by_users_ids,
                },
            )
            .await?;

        if result.modified_count != 1 {
            bail!("Failed to update topic {id}")
        };

        let updated_topic = match self.get_topic(id, user_id, guild).await? {
            Some(topic) => topic,
            None => bail!("Failed to find this topic"),
        };

        let _ = self
            .events_channel
            .0
            .send(TopicEvent::StatusChange(updated_topic.clone().into()));

        if updated_topic.status == TopicStatus::Created {
            let topic_ids = self
                .repository
                .get_topic_ids_sorted(&ObjectId::from_str(
                    &updated_topic.guild_id,
                )?)
                .await?;

            let _ =
                self.events_channel.0.send(TopicEvent::OrderChange(topic_ids));
        }

        Ok(updated_topic)
    }
}
