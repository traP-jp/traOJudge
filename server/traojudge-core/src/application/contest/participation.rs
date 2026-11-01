use super::{ContestService, permissions::*};
use crate::{
    domain::model::{account::AuthenticatedSession, contest::*, request::IdempotencyKey},
    ports::repository::contest::{ContestUnitOfWork, ContestUnitOfWorkProvider},
};
use uuid::Uuid;

impl<U: ContestUnitOfWorkProvider> ContestService<U> {
    pub async fn join_individual(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        id: Uuid,
        key: IdempotencyKey,
    ) -> Result<Participation, ContestError> {
        let mut tx = self
            .unit_of_work
            .begin(Some(actor), ContestScope::existing(kind, id))
            .await?;
        let c = tx.current()?;
        require_joinable(&mut tx, actor.user_id, &c).await?;
        if c.participation_mode != ParticipationMode::Individual {
            return Err(ContestError::Conflict);
        }
        let digest = digest(&(kind, id))?;
        let existing = tx.assignment(actor.user_id, kind.participant()).await?;
        if let Some(receipt) = tx
            .receipt(actor.user_id, "join-individual", id, &key)
            .await?
        {
            check_receipt(&receipt, &digest, c.participation_epoch)?;
            let ContestReceiptResult::Individual(result) = receipt.result else {
                return Err(ContestError::Conflict);
            };
            if existing.as_ref().map(|role| role.granted_at) != Some(result.joined_at) {
                return Err(ContestError::Conflict);
            }
            tx.commit().await?;
            return Ok(result);
        }
        let assignment = match existing {
            Some(role) => role,
            None => {
                if tx.now() >= c.ends_at {
                    return Err(ContestError::Conflict);
                }
                tx.insert_role(actor.user_id, kind.participant()).await?
            }
        };
        let result = Participation {
            user_id: actor.user_id.as_uuid(),
            joined_at: assignment.granted_at,
        };
        let receipt = ContestReceipt {
            digest,
            epoch: c.participation_epoch,
            result: ContestReceiptResult::Individual(result.clone()),
        };
        tx.save_receipt(
            actor.user_id,
            "join-individual",
            id,
            &key,
            &receipt,
            self.receipt_expires_at(tx.now())?,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn join_team(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        key: IdempotencyKey,
        name: String,
        password: String,
    ) -> Result<TeamParticipation, ContestError> {
        validate_text("name", &name, 1, 80)?;
        validate_text("password", &password, 1, 128)?;
        // 高コストのパスワード処理中には接続・行ロックを保持しない。
        let mut preflight = self
            .unit_of_work
            .begin(
                Some(actor),
                ContestScope::existing(ContestKind::Regular, id),
            )
            .await?;
        let c = preflight.current()?;
        require_joinable(&mut preflight, actor.user_id, &c).await?;
        if c.participation_mode != ParticipationMode::Team {
            return Err(ContestError::Conflict);
        }
        preflight.commit().await?;
        let hash_name = name.clone();
        let password_hash = tokio::task::spawn_blocking(move || {
            hash_team_password(c.team_salt, &hash_name, &password)
        })
        .await
        .map_err(ContestError::storage)??;
        let digest = digest(&(&name, &password_hash))?;
        let mut tx = self
            .unit_of_work
            .begin(
                Some(actor),
                ContestScope::existing(ContestKind::Regular, id),
            )
            .await?;
        let c = tx.current()?;
        require_joinable(&mut tx, actor.user_id, &c).await?;
        if c.participation_mode != ParticipationMode::Team {
            return Err(ContestError::Conflict);
        }
        let team_id = tx.team_id(&name, &password_hash).await?;
        let membership = tx.membership(actor.user_id).await?;
        if let Some(receipt) = tx.receipt(actor.user_id, "join-team", id, &key).await? {
            check_receipt(&receipt, &digest, c.participation_epoch)?;
            let ContestReceiptResult::Team(mut result) = receipt.result else {
                return Err(ContestError::Conflict);
            };
            if membership != Some((result.team.id, result.joined_at)) {
                return Err(ContestError::Conflict);
            }
            result.team = tx
                .team(result.team.id)
                .await?
                .ok_or(ContestError::NotFound)?;
            tx.commit().await?;
            return Ok(result);
        }
        let (target, joined_at, created) =
            if let (Some(target), Some((current, joined_at))) = (team_id, membership) {
                if target == current {
                    (target, joined_at, false)
                } else {
                    if tx.now() >= c.starts_at {
                        return Err(ContestError::Conflict);
                    }
                    (
                        target,
                        tx.set_membership(actor.user_id, target).await?,
                        false,
                    )
                }
            } else {
                if tx.now() >= c.starts_at {
                    return Err(ContestError::Conflict);
                }
                let created = team_id.is_none();
                let target = team_id.unwrap_or_else(Uuid::new_v4);
                if created {
                    tx.create_team(target, &name, &password_hash).await?;
                }
                (
                    target,
                    tx.set_membership(actor.user_id, target).await?,
                    created,
                )
            };
        if tx
            .assignment(actor.user_id, c.kind.participant())
            .await?
            .is_none()
        {
            tx.insert_role(actor.user_id, c.kind.participant()).await?;
        }
        let result = TeamParticipation {
            team: tx.team(target).await?.ok_or(ContestError::NotFound)?,
            joined_at,
            created,
        };
        let receipt = ContestReceipt {
            digest,
            epoch: c.participation_epoch,
            result: ContestReceiptResult::Team(result.clone()),
        };
        tx.save_receipt(
            actor.user_id,
            "join-team",
            id,
            &key,
            &receipt,
            self.receipt_expires_at(tx.now())?,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn team(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
        team_id: Uuid,
    ) -> Result<ContestTeam, ContestError> {
        let mut tx = self
            .unit_of_work
            .begin(
                Some(actor),
                ContestScope::existing(ContestKind::Regular, id),
            )
            .await?;
        let c = tx.current()?;
        if c.participation_mode != ParticipationMode::Team {
            return Err(ContestError::Conflict);
        }
        if !has_role(&mut tx, actor.user_id, c.kind.setter()).await?
            && !has_role(&mut tx, actor.user_id, c.kind.participant()).await?
        {
            return Err(ContestError::Forbidden);
        }
        let result = tx.team(team_id).await?.ok_or(ContestError::NotFound)?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn leave_team(
        &self,
        actor: AuthenticatedSession,
        id: Uuid,
    ) -> Result<(), ContestError> {
        let mut tx = self
            .unit_of_work
            .begin(
                Some(actor),
                ContestScope::existing(ContestKind::Regular, id),
            )
            .await?;
        let c = tx.current()?;
        if !tx
            .has_service_role(
                actor.user_id,
                crate::domain::model::account::RoleName::StandardUser,
            )
            .await?
        {
            return Err(ContestError::Forbidden);
        }
        if c.participation_mode != ParticipationMode::Team || tx.now() >= c.starts_at {
            return Err(ContestError::Conflict);
        }
        tx.remove_membership(actor.user_id).await?;
        tx.delete_role(actor.user_id, c.kind.participant()).await?;
        tx.commit().await
    }
}

fn hash_team_password(salt: Uuid, name: &str, password: &str) -> Result<String, ContestError> {
    use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
    use sha2::{Digest, Sha256};
    // 同一大会・同一名の秘密を同じArgon2値で検索する。大会ごとにランダムなsaltを持つ。
    let mut digest = Sha256::new();
    digest.update(salt.as_bytes());
    digest.update(name.as_bytes());
    let bytes = digest.finalize();
    let salt = SaltString::encode_b64(&bytes[..16]).map_err(ContestError::storage)?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(ContestError::storage)
}
