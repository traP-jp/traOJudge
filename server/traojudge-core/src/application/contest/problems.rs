use super::{ContestService, permissions::*};
use crate::{
    domain::model::{
        account::{AuthenticatedSession, RoleName},
        contest::*,
        problem::ProblemVisibility,
    },
    ports::repository::contest::{ContestUnitOfWork, ContestUnitOfWorkProvider},
};
use uuid::Uuid;

impl<U: ContestUnitOfWorkProvider> ContestService<U> {
    pub async fn problems(
        &self,
        actor: Option<AuthenticatedSession>,
        kind: ContestKind,
        id: Uuid,
    ) -> Result<Vec<ContestProblem>, ContestError> {
        let mut tx = self
            .unit_of_work
            .begin(actor, ContestScope::existing(kind, id))
            .await?;
        let c = tx.current()?;
        let staff = if let Some(actor) = actor {
            has_role(&mut tx, actor.user_id, kind.setter()).await?
                || (kind == ContestKind::Regular
                    && has_role(&mut tx, actor.user_id, RoleName::ContestAdmin).await?)
        } else {
            false
        };
        if !staff {
            if tx.now() < c.starts_at {
                return Err(ContestError::NotFound);
            }
            if c.visibility != ContestVisibility::Public {
                let participant = if let Some(actor) = actor {
                    has_role(&mut tx, actor.user_id, kind.participant()).await?
                } else {
                    false
                };
                if !participant {
                    return Err(ContestError::NotFound);
                }
            }
        }
        let problems = tx.problems().await?;
        tx.commit().await?;
        Ok(problems)
    }
    pub async fn add_problem(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        id: Uuid,
        problem_id: Uuid,
        input: ContestProblemInput,
    ) -> Result<ContestProblem, ContestError> {
        if let Some(label) = &input.label {
            validate_text("label", label, 1, 32)?;
        }
        if input.order.is_some_and(|order| order < 0) {
            return Err(ContestError::validation("order", "must be nonnegative"));
        }
        let mut tx = self
            .unit_of_work
            .begin(Some(actor), ContestScope::existing(kind, id))
            .await?;
        let c = tx.current()?;
        require_setter(&mut tx, actor.user_id, &c).await?;
        if tx.now() >= c.ends_at {
            return Err(ContestError::Conflict);
        }
        // TODO(problems): 本文・採点設定の版を追加したら、出題の版固定ルールを接続する。
        let problem = tx
            .problem(problem_id)
            .await?
            .ok_or(ContestError::NotFound)?;
        if problem.deleted_at.is_some() {
            return Err(ContestError::NotFound);
        }
        match kind {
            ContestKind::Regular if problem.created_by_user_id != Some(actor.user_id) => {
                return Err(ContestError::Forbidden);
            }
            ContestKind::Virtual if problem.visibility != ProblemVisibility::Public => {
                return Err(ContestError::NotFound);
            }
            _ => {}
        }
        let problems = tx.problems().await?;
        if let Some(existing) = problems.iter().find(|p| p.problem_id == problem_id) {
            if input
                .label
                .as_ref()
                .is_some_and(|label| label != &existing.label)
                || input.order.is_some_and(|order| order != existing.order)
            {
                return Err(ContestError::Conflict);
            }
            let existing = existing.clone();
            tx.commit().await?;
            return Ok(existing);
        }
        let order = match input.order {
            Some(order) => order,
            None => tx
                .max_problem_order()
                .await?
                .unwrap_or(-1)
                .checked_add(1)
                .ok_or(ContestError::Conflict)?,
        };
        let label = input.label.unwrap_or_else(|| problem_label(order));
        let p = ContestProblem {
            problem_id,
            label,
            order,
        };
        tx.insert_problem(&p).await?;
        tx.commit().await?;
        Ok(p)
    }
    pub async fn remove_problem(
        &self,
        actor: AuthenticatedSession,
        kind: ContestKind,
        id: Uuid,
        problem_id: Uuid,
    ) -> Result<(), ContestError> {
        let mut tx = self
            .unit_of_work
            .begin(Some(actor), ContestScope::existing(kind, id))
            .await?;
        let c = tx.current()?;
        require_setter(&mut tx, actor.user_id, &c).await?;
        if tx.now() >= c.ends_at {
            return Err(ContestError::Conflict);
        }
        tx.remove_problem(problem_id).await?;
        tx.commit().await
    }
}
fn problem_label(order: i32) -> String {
    let mut n = order as u64 + 1;
    let mut chars = Vec::new();
    while n > 0 {
        n -= 1;
        chars.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    chars.into_iter().rev().collect()
}
