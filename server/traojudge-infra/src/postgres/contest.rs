use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use traojudge_core::{
    domain::model::{
        account::{AccountRole, AuthenticatedSession, RoleName, RoleScope},
        contest::*,
        problem::ProblemReference,
        request::IdempotencyKey,
        user::UserId,
    },
    ports::repository::contest::{ContestUnitOfWork, ContestUnitOfWorkProvider},
};
use uuid::Uuid;

pub struct PgContestUnitOfWorkProvider {
    pool: PgPool,
}
impl PgContestUnitOfWorkProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
pub struct PgContestUnitOfWork {
    tx: Transaction<'static, Postgres>,
    contest: Option<Contest>,
    now: DateTime<Utc>,
}
fn storage(error: sqlx::Error) -> ContestError {
    ContestError::storage(error)
}
fn write_error(error: sqlx::Error) -> ContestError {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        ContestError::Conflict
    } else {
        storage(error)
    }
}

#[derive(sqlx::FromRow)]
struct ContestRow {
    id: Uuid,
    kind: String,
    created_by_user_id: Uuid,
    title: String,
    description: String,
    visibility: String,
    participation_mode: String,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    tie_break: String,
    penalty_minutes: i32,
    revision: String,
    created_at: DateTime<Utc>,
    participation_epoch: Uuid,
    team_salt: Uuid,
}
impl TryFrom<ContestRow> for Contest {
    type Error = ContestError;
    fn try_from(row: ContestRow) -> Result<Self, Self::Error> {
        let invalid = || ContestError::storage(std::io::Error::other("invalid contest value"));
        Ok(Self {
            id: row.id,
            kind: match row.kind.as_str() {
                "regular" => ContestKind::Regular,
                "virtual" => ContestKind::Virtual,
                _ => return Err(invalid()),
            },
            created_by: row.created_by_user_id,
            title: row.title,
            description: row.description,
            visibility: match row.visibility.as_str() {
                "public" => ContestVisibility::Public,
                "private" => ContestVisibility::Private,
                _ => return Err(invalid()),
            },
            participation_mode: match row.participation_mode.as_str() {
                "individual" => ParticipationMode::Individual,
                "team" => ParticipationMode::Team,
                _ => return Err(invalid()),
            },
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            rules: ContestRules {
                tie_break: match row.tie_break.as_str() {
                    "icpc" => TieBreak::Icpc,
                    "atcoder" => TieBreak::Atcoder,
                    _ => return Err(invalid()),
                },
                penalty_minutes: row.penalty_minutes,
            },
            revision: row.revision,
            created_at: row.created_at,
            participation_epoch: row.participation_epoch,
            team_salt: row.team_salt,
        })
    }
}
#[async_trait]
impl ContestUnitOfWorkProvider for PgContestUnitOfWorkProvider {
    type UnitOfWork<'a> = PgContestUnitOfWork;
    async fn begin(
        &self,
        actor: Option<AuthenticatedSession>,
        scope: ContestScope,
    ) -> Result<Self::UnitOfWork<'_>, ContestError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let contest = if let Some((kind, id)) = scope.target {
            Some(sqlx::query_as::<_,ContestRow>("SELECT id,kind,created_by_user_id,title,description,visibility,participation_mode,starts_at,ends_at,tie_break,penalty_minutes,revision,created_at,participation_epoch,team_salt FROM contests WHERE id = $1 AND kind = $2 AND deleted_at IS NULL FOR UPDATE")
                .bind(id).bind(kind.as_str()).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(ContestError::NotFound)?.try_into()?)
        } else {
            None
        };
        let mut users = scope
            .additional_users
            .iter()
            .map(UserId::as_uuid)
            .collect::<Vec<_>>();
        if let Some(actor) = actor {
            users.push(actor.user_id.as_uuid());
        }
        if scope.lock_participants {
            let c: &Contest = contest.as_ref().ok_or(ContestError::NotFound)?;
            let participants:Vec<Uuid>=sqlx::query_scalar("SELECT user_id FROM account_roles WHERE scope_kind = $1 AND scope_id = $2 AND role = $3 UNION SELECT user_id FROM contest_team_memberships WHERE contest_id = $2")
                .bind(c.kind.scope_kind()).bind(c.id).bind(c.kind.participant().as_str()).fetch_all(&mut *tx).await.map_err(storage)?;
            users.extend(participants);
        }
        users.sort_unstable();
        users.dedup();
        let locked: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE id = ANY($1) ORDER BY id FOR UPDATE")
                .bind(&users)
                .fetch_all(&mut *tx)
                .await
                .map_err(storage)?;
        if let Some(actor) = actor {
            if !locked.contains(&actor.user_id.as_uuid()) {
                return Err(ContestError::Unauthenticated);
            }
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM web_sessions WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL AND expires_at > clock_timestamp())")
                .bind(actor.id).bind(actor.user_id.as_uuid()).fetch_one(&mut *tx).await.map_err(storage)?;
            if !valid {
                return Err(ContestError::Unauthenticated);
            }
        }
        if locked.len() != users.len() {
            return Err(ContestError::NotFound);
        }
        let now = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        Ok(PgContestUnitOfWork { tx, contest, now })
    }
}
impl PgContestUnitOfWork {
    fn target(&self) -> Result<(Uuid, ContestKind), ContestError> {
        self.contest
            .as_ref()
            .map(|c| (c.id, c.kind))
            .ok_or(ContestError::NotFound)
    }
}
#[async_trait]
impl ContestUnitOfWork for PgContestUnitOfWork {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
    fn current(&self) -> Result<Contest, ContestError> {
        self.contest.clone().ok_or(ContestError::NotFound)
    }
    async fn has_service_role(
        &mut self,
        user: UserId,
        role: RoleName,
    ) -> Result<bool, ContestError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_roles WHERE user_id = $1 AND role = $2 AND scope_kind = 'service' AND scope_id IS NULL)")
            .bind(user.as_uuid()).bind(role.as_str()).fetch_one(&mut *self.tx).await.map_err(storage)
    }
    async fn assignment(
        &mut self,
        user: UserId,
        role: RoleName,
    ) -> Result<Option<AccountRole>, ContestError> {
        let (id, kind) = self.target()?;
        let time:Option<DateTime<Utc>>=sqlx::query_scalar("SELECT granted_at FROM account_roles WHERE user_id = $1 AND role = $2 AND scope_kind = $3 AND scope_id = $4")
            .bind(user.as_uuid()).bind(role.as_str()).bind(kind.scope_kind()).bind(id).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        Ok(time.map(|granted_at| AccountRole {
            role,
            scope: match kind {
                ContestKind::Regular => RoleScope::Contest(id),
                ContestKind::Virtual => RoleScope::VirtualContest(id),
            },
            user_id: user,
            granted_at,
        }))
    }
    async fn insert_role(
        &mut self,
        user: UserId,
        role: RoleName,
    ) -> Result<AccountRole, ContestError> {
        let (id, kind) = self.target()?;
        let granted_at=sqlx::query_scalar("INSERT INTO account_roles (user_id,role,scope_kind,scope_id) VALUES ($1,$2,$3,$4) RETURNING granted_at")
            .bind(user.as_uuid()).bind(role.as_str()).bind(kind.scope_kind()).bind(id).fetch_one(&mut *self.tx).await.map_err(write_error)?;
        Ok(AccountRole {
            role,
            scope: match kind {
                ContestKind::Regular => RoleScope::Contest(id),
                ContestKind::Virtual => RoleScope::VirtualContest(id),
            },
            user_id: user,
            granted_at,
        })
    }
    async fn delete_role(&mut self, user: UserId, role: RoleName) -> Result<(), ContestError> {
        let (id, kind) = self.target()?;
        sqlx::query("DELETE FROM account_roles WHERE user_id = $1 AND role = $2 AND scope_kind = $3 AND scope_id = $4")
            .bind(user.as_uuid()).bind(role.as_str()).bind(kind.scope_kind()).bind(id).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn is_active(&mut self, kind: ContestKind, id: Uuid) -> Result<bool, ContestError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM contests WHERE id = $1 AND kind = $2 AND deleted_at IS NULL)").bind(id).bind(kind.as_str()).fetch_one(&mut *self.tx).await.map_err(storage)
    }
    async fn insert(&mut self, c: &Contest) -> Result<(), ContestError> {
        sqlx::query("INSERT INTO contests (id,kind,created_by_user_id,title,description,visibility,participation_mode,starts_at,ends_at,tie_break,penalty_minutes,revision,created_at,participation_epoch,team_salt) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)")
            .bind(c.id).bind(c.kind.as_str()).bind(c.created_by).bind(&c.title).bind(&c.description).bind(c.visibility.as_str()).bind(c.participation_mode.as_str()).bind(c.starts_at).bind(c.ends_at).bind(c.rules.tie_break.as_str()).bind(c.rules.penalty_minutes).bind(&c.revision).bind(c.created_at).bind(c.participation_epoch).bind(c.team_salt)
            .execute(&mut *self.tx).await.map_err(write_error)?;
        self.contest = Some(c.clone());
        Ok(())
    }
    async fn save(&mut self, c: &Contest) -> Result<(), ContestError> {
        let (id, _) = self.target()?;
        sqlx::query("UPDATE contests SET description = $2, participation_mode = $3, starts_at = $4, ends_at = $5, tie_break = $6, penalty_minutes = $7, revision = $8, participation_epoch = $9, title = $10, visibility = $11 WHERE id = $1")
            .bind(id).bind(&c.description).bind(c.participation_mode.as_str()).bind(c.starts_at).bind(c.ends_at).bind(c.rules.tie_break.as_str()).bind(c.rules.penalty_minutes).bind(&c.revision).bind(c.participation_epoch).bind(&c.title).bind(c.visibility.as_str())
            .execute(&mut *self.tx).await.map_err(storage)?;
        self.contest = Some(c.clone());
        Ok(())
    }
    async fn delete(&mut self) -> Result<(), ContestError> {
        let (id, _) = self.target()?;
        sqlx::query("UPDATE contests SET deleted_at = clock_timestamp(),revision = gen_random_uuid()::text WHERE id = $1").bind(id).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn list_public(
        &mut self,
        kind: ContestKind,
        page: &ContestPageRequest,
    ) -> Result<Vec<Contest>, ContestError> {
        let rows=sqlx::query_as::<_,ContestRow>("SELECT id,kind,created_by_user_id,title,description,visibility,participation_mode,starts_at,ends_at,tie_break,penalty_minutes,revision,created_at,participation_epoch,team_salt FROM contests WHERE kind = $1 AND visibility = 'public' AND deleted_at IS NULL AND ($2::timestamptz IS NULL OR (created_at,id) > ($2,$3)) ORDER BY created_at,id LIMIT $4")
            .bind(kind.as_str()).bind(page.after.map(|a|a.0)).bind(page.after.map(|a|a.1)).bind(i64::from(page.limit)+1).fetch_all(&mut *self.tx).await.map_err(storage)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
    async fn clear_participation(&mut self) -> Result<(), ContestError> {
        let (id, kind) = self.target()?;
        sqlx::query(
            "DELETE FROM account_roles WHERE scope_kind = $1 AND scope_id = $2 AND role = $3",
        )
        .bind(kind.scope_kind())
        .bind(id)
        .bind(kind.participant().as_str())
        .execute(&mut *self.tx)
        .await
        .map_err(storage)?;
        sqlx::query("DELETE FROM contest_teams WHERE contest_id = $1")
            .bind(id)
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn problem(&mut self, id: Uuid) -> Result<Option<ProblemReference>, ContestError> {
        super::problems::lock_reference(&mut self.tx, id)
            .await
            .map_err(storage)
    }
    async fn problems(&mut self) -> Result<Vec<ContestProblem>, ContestError> {
        let (id, _) = self.target()?;
        let rows:Vec<(Uuid,String,i32)>=sqlx::query_as("SELECT cp.problem_id,cp.label,cp.position FROM contest_problems cp JOIN problems p ON p.id = cp.problem_id WHERE cp.contest_id = $1 AND cp.removed_at IS NULL AND p.deleted_at IS NULL ORDER BY cp.position,cp.problem_id")
            .bind(id).fetch_all(&mut *self.tx).await.map_err(storage)?;
        Ok(rows
            .into_iter()
            .map(|(problem_id, label, order)| ContestProblem {
                problem_id,
                label,
                order,
            })
            .collect())
    }
    async fn max_problem_order(&mut self) -> Result<Option<i32>, ContestError> {
        let (id, _) = self.target()?;
        sqlx::query_scalar("SELECT max(position) FROM contest_problems WHERE contest_id = $1 AND removed_at IS NULL")
            .bind(id).fetch_one(&mut *self.tx).await.map_err(storage)
    }
    async fn insert_problem(&mut self, p: &ContestProblem) -> Result<(), ContestError> {
        let (id, _) = self.target()?;
        sqlx::query("INSERT INTO contest_problems (contest_id,problem_id,label,position) VALUES ($1,$2,$3,$4) ON CONFLICT (contest_id,problem_id) DO UPDATE SET label = EXCLUDED.label,position = EXCLUDED.position,removed_at = NULL WHERE contest_problems.removed_at IS NOT NULL")
            .bind(id).bind(p.problem_id).bind(&p.label).bind(p.order).execute(&mut *self.tx).await.map_err(write_error)?;
        Ok(())
    }
    async fn remove_problem(&mut self, problem: Uuid) -> Result<(), ContestError> {
        let (id, _) = self.target()?;
        sqlx::query("UPDATE contest_problems SET removed_at = clock_timestamp() WHERE contest_id = $1 AND problem_id = $2 AND removed_at IS NULL").bind(id).bind(problem).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn team_id(
        &mut self,
        name: &str,
        password_hash: &str,
    ) -> Result<Option<Uuid>, ContestError> {
        let (id, _) = self.target()?;
        sqlx::query_scalar("SELECT id FROM contest_teams WHERE contest_id = $1 AND name = $2 AND password_hash = $3").bind(id).bind(name).bind(password_hash).fetch_optional(&mut *self.tx).await.map_err(storage)
    }
    async fn create_team(
        &mut self,
        team: Uuid,
        name: &str,
        password_hash: &str,
    ) -> Result<(), ContestError> {
        let (id, _) = self.target()?;
        sqlx::query(
            "INSERT INTO contest_teams (id,contest_id,name,password_hash) VALUES ($1,$2,$3,$4)",
        )
        .bind(team)
        .bind(id)
        .bind(name)
        .bind(password_hash)
        .execute(&mut *self.tx)
        .await
        .map_err(write_error)?;
        Ok(())
    }
    async fn team(&mut self, team: Uuid) -> Result<Option<ContestTeam>, ContestError> {
        let (id, _) = self.target()?;
        let name: Option<String> =
            sqlx::query_scalar("SELECT name FROM contest_teams WHERE contest_id = $1 AND id = $2")
                .bind(id)
                .bind(team)
                .fetch_optional(&mut *self.tx)
                .await
                .map_err(storage)?;
        let Some(name) = name else { return Ok(None) };
        let members:Vec<(Uuid,String)>=sqlx::query_as("SELECT u.id,u.display_name FROM contest_team_memberships m JOIN users u ON u.id = m.user_id WHERE m.contest_id = $1 AND m.team_id = $2 ORDER BY m.joined_at,u.id")
            .bind(id).bind(team).fetch_all(&mut *self.tx).await.map_err(storage)?;
        Ok(Some(ContestTeam {
            id: team,
            name,
            members: members
                .into_iter()
                .map(|(user_id, display_name)| TeamMember {
                    user_id,
                    display_name,
                })
                .collect(),
        }))
    }
    async fn membership(
        &mut self,
        user: UserId,
    ) -> Result<Option<(Uuid, DateTime<Utc>)>, ContestError> {
        let (id, _) = self.target()?;
        sqlx::query_as("SELECT team_id,joined_at FROM contest_team_memberships WHERE contest_id = $1 AND user_id = $2").bind(id).bind(user.as_uuid()).fetch_optional(&mut *self.tx).await.map_err(storage)
    }
    async fn set_membership(
        &mut self,
        user: UserId,
        team: Uuid,
    ) -> Result<DateTime<Utc>, ContestError> {
        let (id, _) = self.target()?;
        sqlx::query_scalar("INSERT INTO contest_team_memberships (contest_id,user_id,team_id,joined_at) VALUES ($1,$2,$3,$4) ON CONFLICT (contest_id,user_id) DO UPDATE SET team_id = EXCLUDED.team_id,joined_at = EXCLUDED.joined_at RETURNING joined_at")
            .bind(id).bind(user.as_uuid()).bind(team).bind(self.now).fetch_one(&mut *self.tx).await.map_err(storage)
    }
    async fn remove_membership(&mut self, user: UserId) -> Result<(), ContestError> {
        let (id, _) = self.target()?;
        sqlx::query("DELETE FROM contest_team_memberships WHERE contest_id = $1 AND user_id = $2")
            .bind(id)
            .bind(user.as_uuid())
            .execute(&mut *self.tx)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn receipt(
        &mut self,
        user: UserId,
        operation: &str,
        scope: Uuid,
        key: &IdempotencyKey,
    ) -> Result<Option<ContestReceipt>, ContestError> {
        let row:Option<(Vec<u8>,Uuid,sqlx::types::Json<ContestReceiptResult>)>=sqlx::query_as("SELECT request_digest,participation_epoch,result FROM contest_request_receipts WHERE user_id = $1 AND operation = $2 AND scope_id = $3 AND key = $4 AND expires_at > $5")
            .bind(user.as_uuid()).bind(operation).bind(scope).bind(key.as_str()).bind(self.now).fetch_optional(&mut *self.tx).await.map_err(storage)?;
        Ok(row.map(|(digest, epoch, result)| ContestReceipt {
            digest,
            epoch,
            result: result.0,
        }))
    }
    async fn save_receipt(
        &mut self,
        user: UserId,
        operation: &str,
        scope: Uuid,
        key: &IdempotencyKey,
        r: &ContestReceipt,
        expires_at: DateTime<Utc>,
    ) -> Result<(), ContestError> {
        sqlx::query("INSERT INTO contest_request_receipts (user_id,operation,scope_id,key,request_digest,participation_epoch,result,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (user_id,operation,scope_id,key) DO UPDATE SET request_digest = EXCLUDED.request_digest,participation_epoch = EXCLUDED.participation_epoch,result = EXCLUDED.result,expires_at = EXCLUDED.expires_at")
            .bind(user.as_uuid()).bind(operation).bind(scope).bind(key.as_str()).bind(&r.digest).bind(r.epoch).bind(sqlx::types::Json(&r.result)).bind(expires_at).execute(&mut *self.tx).await.map_err(storage)?;
        Ok(())
    }
    async fn commit(self) -> Result<(), ContestError> {
        self.tx.commit().await.map_err(storage)
    }
    async fn rollback(self) -> Result<(), ContestError> {
        self.tx.rollback().await.map_err(storage)
    }
}
