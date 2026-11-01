use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use traojudge_backend_app::http::{build_router, middleware::origin::WebOrigin};
use traojudge_core::{
    application::{
        account::AccountService, contest::ContestService, editorial::EditorialService,
        session::SessionService, users::GetUserProfile,
    },
    domain::model::{
        account::{AuthenticatedSession, RoleName, Secret},
        contest::*,
        request::IdempotencyKey,
        user::UserId,
    },
};
use traojudge_infra::{
    account_services::UnconfiguredAccountServices,
    postgres::{
        accounts::PgAccountUnitOfWorkProvider, contest::PgContestUnitOfWorkProvider,
        editorial::PgEditorialUnitOfWorkProvider, sessions::PgSessionRepository,
        users::PgUserProfileRepository,
    },
};
use uuid::Uuid;
const ORIGIN: &str = "https://judge.example";
struct Actor {
    id: Uuid,
    session: AuthenticatedSession,
    secret: String,
}
async fn actor(pool: &PgPool, role: Option<&str>) -> Actor {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id,display_name,revision) VALUES ($1,'利用者','initial')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    if let Some(role) = role {
        sqlx::query("INSERT INTO account_roles (user_id,role,scope_kind) VALUES ($1,$2,'service')")
            .bind(id)
            .bind(role)
            .execute(pool)
            .await
            .unwrap();
    }
    let secret = Secret::random();
    let session_id = Uuid::new_v4();
    sqlx::query("INSERT INTO web_sessions (id,user_id,token_digest,expires_at) VALUES ($1,$2,$3,clock_timestamp()+INTERVAL '1 day')")
        .bind(session_id).bind(id).bind(secret.digest()).execute(pool).await.unwrap();
    Actor {
        id,
        session: AuthenticatedSession {
            id: session_id,
            user_id: UserId::new(id),
        },
        secret: secret.expose().to_owned(),
    }
}
fn app(pool: &PgPool) -> Router {
    build_router(
        GetUserProfile::new(PgUserProfileRepository::new(pool.clone())),
        AccountService::new(
            PgAccountUnitOfWorkProvider::new(pool.clone()),
            UnconfiguredAccountServices,
        ),
        SessionService::new(PgSessionRepository::new(pool.clone())),
        EditorialService::new(
            PgEditorialUnitOfWorkProvider::new(pool.clone()),
            Duration::days(1),
        )
        .unwrap(),
        traojudge_core::application::contest::ContestService::new(
            traojudge_infra::postgres::contest::PgContestUnitOfWorkProvider::new(pool.clone()),
            Duration::days(1),
        )
        .unwrap(),
        WebOrigin::parse(ORIGIN).unwrap(),
    )
}
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}
struct Client {
    router: Router,
    secret: Option<String>,
}
impl Client {
    fn new(router: &Router, actor: Option<&Actor>) -> Self {
        Self {
            router: router.clone(),
            secret: actor.map(|actor| actor.secret.clone()),
        }
    }
    async fn call(
        &self,
        verb: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<Value>,
    ) -> Reply {
        let mut builder = Request::builder().method(verb).uri(path);
        if let Some(secret) = &self.secret {
            builder = builder.header("cookie", format!("traojudge_session={secret}"));
        }
        if !matches!(verb, "GET" | "HEAD") && !headers.iter().any(|(name, _)| *name == "origin") {
            builder = builder.header("origin", ORIGIN);
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let body = match body {
            Some(body) => {
                builder = builder.header("content-type", "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .router
            .clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        assert_eq!(headers["cache-control"], "no-store");
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        Reply {
            status,
            headers,
            body,
        }
    }
}

fn service(pool: &PgPool) -> ContestService<PgContestUnitOfWorkProvider> {
    ContestService::new(
        PgContestUnitOfWorkProvider::new(pool.clone()),
        Duration::days(1),
    )
    .unwrap()
}
fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).unwrap()
}
fn input(mode: ParticipationMode) -> NewContest {
    NewContest {
        title: "大会".into(),
        description: String::new(),
        visibility: ContestVisibility::Public,
        participation_mode: mode,
        starts_at: Utc::now() + Duration::hours(1),
        ends_at: Utc::now() + Duration::hours(3),
        rules: ContestRules {
            tie_break: TieBreak::Icpc,
            penalty_minutes: 5,
        },
    }
}
async fn normal(pool: &PgPool, admin: &Actor, setter: &Actor, mode: ParticipationMode) -> Contest {
    let svc = service(pool);
    let c = svc
        .create(
            admin.session,
            ContestKind::Regular,
            key(&Uuid::new_v4().to_string()),
            input(mode),
        )
        .await
        .unwrap();
    svc.grant_role(
        admin.session,
        c.kind,
        c.id,
        setter.session.user_id,
        RoleName::ContestSetter,
    )
    .await
    .unwrap();
    c
}
async fn problem(pool: &PgPool, creator: Option<&Actor>, public: bool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO problems (id,created_by_user_id,visibility) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(creator.map(|a| a.id))
        .bind(if public { "public" } else { "private" })
        .execute(pool)
        .await
        .unwrap();
    id
}
async fn running(pool: &PgPool, id: Uuid) {
    sqlx::query(
        "UPDATE contests SET starts_at = clock_timestamp()-INTERVAL '1 hour' WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}
async fn ended(pool: &PgPool, id: Uuid) {
    sqlx::query("UPDATE contests SET starts_at = clock_timestamp()-INTERVAL '2 hours',ends_at = clock_timestamp()-INTERVAL '1 hour' WHERE id = $1").bind(id).execute(pool).await.unwrap();
}
async fn count(pool: &PgPool, table: &str, id: Uuid) -> i64 {
    let query = match table {
        "contest_teams" => "SELECT count(*) FROM contest_teams WHERE contest_id = $1",
        "contest_team_memberships" => {
            "SELECT count(*) FROM contest_team_memberships WHERE contest_id = $1"
        }
        _ => panic!("unknown fixture table"),
    };
    sqlx::query_scalar(query)
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn normal_roles_are_manual_and_scoped(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let organizer = actor(&pool, None).await;
    let setter = actor(&pool, None).await;
    let stranger = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let mut private = input(ParticipationMode::Individual);
    private.visibility = ContestVisibility::Private;
    let c = svc
        .create(admin.session, ContestKind::Regular, key("create"), private)
        .await
        .unwrap();
    assert!(matches!(
        svc.get(Some(admin.session), c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
    assert!(matches!(
        svc.create(
            stranger.session,
            c.kind,
            key("denied"),
            input(ParticipationMode::Individual)
        )
        .await,
        Err(ContestError::Forbidden)
    ));
    let (role, changed) = svc
        .grant_role(
            admin.session,
            c.kind,
            c.id,
            organizer.session.user_id,
            RoleName::ContestAdmin,
        )
        .await
        .unwrap();
    assert!(changed);
    assert_eq!(role.role, RoleName::ContestAdmin);
    assert!(
        !svc.grant_role(
            admin.session,
            c.kind,
            c.id,
            organizer.session.user_id,
            RoleName::ContestAdmin
        )
        .await
        .unwrap()
        .1
    );
    svc.grant_role(
        organizer.session,
        c.kind,
        c.id,
        setter.session.user_id,
        RoleName::ContestSetter,
    )
    .await
    .unwrap();
    assert!(svc.get(Some(setter.session), c.kind, c.id).await.is_ok());
    assert!(matches!(
        svc.grant_role(
            organizer.session,
            c.kind,
            c.id,
            stranger.session.user_id,
            RoleName::ContestAdmin
        )
        .await,
        Err(ContestError::Forbidden)
    ));
    assert!(matches!(
        svc.revoke_role(
            organizer.session,
            c.kind,
            c.id,
            setter.session.user_id,
            RoleName::ContestSetter
        )
        .await,
        Err(ContestError::Forbidden)
    ));
    svc.revoke_role(
        admin.session,
        c.kind,
        c.id,
        setter.session.user_id,
        RoleName::ContestSetter,
    )
    .await
    .unwrap();
    assert!(matches!(
        svc.get(Some(setter.session), c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
    assert!(matches!(
        svc.get(Some(organizer.session), ContestKind::Virtual, c.id)
            .await,
        Err(ContestError::NotFound)
    ));
    assert!(matches!(
        svc.delete(organizer.session, c.kind, c.id).await,
        Err(ContestError::Forbidden)
    ));
    svc.delete(admin.session, c.kind, c.id).await.unwrap();
    assert!(matches!(
        svc.get(Some(organizer.session), c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM contests WHERE id = $1 AND deleted_at IS NOT NULL"
        )
        .bind(c.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn virtual_creator_is_setter_and_only_public_problems_are_assignable(pool: PgPool) {
    let creator = actor(&pool, Some("standard-user")).await;
    let admin = actor(&pool, Some("service-admin")).await;
    let svc = service(&pool);
    let c = svc
        .create(
            creator.session,
            ContestKind::Virtual,
            key("create"),
            input(ParticipationMode::Individual),
        )
        .await
        .unwrap();
    assert_eq!(
        svc.create(creator.session, c.kind, key("create"), input_same(&c))
            .await
            .unwrap()
            .id,
        c.id
    );
    let public = problem(&pool, None, true).await;
    let private = problem(&pool, Some(&creator), false).await;
    assert!(matches!(
        svc.add_problem(
            creator.session,
            c.kind,
            c.id,
            private,
            ContestProblemInput::default()
        )
        .await,
        Err(ContestError::NotFound)
    ));
    assert_eq!(
        svc.add_problem(
            creator.session,
            c.kind,
            c.id,
            public,
            ContestProblemInput::default()
        )
        .await
        .unwrap()
        .label,
        "A"
    );
    assert!(matches!(
        svc.delete(admin.session, c.kind, c.id).await,
        Err(ContestError::Forbidden)
    ));
    svc.remove_problem(creator.session, c.kind, c.id, public)
        .await
        .unwrap();
    svc.delete(creator.session, c.kind, c.id).await.unwrap();
    assert!(matches!(
        svc.create(creator.session, c.kind, key("create"), input_same(&c))
            .await,
        Err(ContestError::NotFound)
    ));
}
fn input_same(c: &Contest) -> NewContest {
    NewContest {
        title: c.title.clone(),
        description: c.description.clone(),
        visibility: c.visibility,
        participation_mode: c.participation_mode,
        starts_at: c.starts_at,
        ends_at: c.ends_at,
        rules: c.rules,
    }
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn settings_freeze_at_original_start_and_etag_prevents_lost_updates(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let c = normal(&pool, &admin, &setter, ParticipationMode::Individual).await;
    let svc = service(&pool);
    assert!(matches!(
        svc.update(
            setter.session,
            c.kind,
            c.id,
            "stale",
            ContestChanges {
                description: Some("概要".into()),
                ..Default::default()
            }
        )
        .await,
        Err(ContestError::RevisionMismatch)
    ));
    running(&pool, c.id).await;
    for changes in [
        ContestChanges {
            starts_at: Some(Utc::now() + Duration::days(1)),
            ..Default::default()
        },
        ContestChanges {
            participation_mode: Some(ParticipationMode::Team),
            ..Default::default()
        },
        ContestChanges {
            rules: Some(ContestRules {
                tie_break: TieBreak::Atcoder,
                penalty_minutes: 0,
            }),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            svc.update(setter.session, c.kind, c.id, &c.revision, changes)
                .await,
            Err(ContestError::Conflict)
        ));
    }
    let updated = svc
        .update(
            setter.session,
            c.kind,
            c.id,
            &c.revision,
            ContestChanges {
                description: Some("## 概要\n`<html>`".into()),
                ends_at: Some(Utc::now() + Duration::hours(4)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_ne!(updated.revision, c.revision);
    assert!(updated.description.contains("概要"));
    assert!(matches!(
        svc.update(
            setter.session,
            c.kind,
            c.id,
            &updated.revision,
            ContestChanges {
                description: Some("<script>alert(1)</script>".into()),
                ..Default::default()
            }
        )
        .await,
        Err(ContestError::Validation { .. })
    ));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn assignment_checks_creator_not_owner_role_and_visibility_changes_at_start(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let other = actor(&pool, None).await;
    let viewer = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let c = normal(&pool, &admin, &setter, ParticipationMode::Individual).await;
    let own = problem(&pool, Some(&setter), false).await;
    let foreign = problem(&pool, Some(&other), true).await;
    let legacy = problem(&pool, None, true).await;
    sqlx::query("INSERT INTO account_roles (user_id,role,scope_kind,scope_id) VALUES ($1,'problem-owner','problem',$2)").bind(setter.id).bind(foreign).execute(&pool).await.unwrap();
    for p in [foreign, legacy] {
        assert!(matches!(
            svc.add_problem(
                setter.session,
                c.kind,
                c.id,
                p,
                ContestProblemInput::default()
            )
            .await,
            Err(ContestError::Forbidden)
        ));
    }
    let added = svc
        .add_problem(
            setter.session,
            c.kind,
            c.id,
            own,
            ContestProblemInput::default(),
        )
        .await
        .unwrap();
    assert_eq!(added.order, 0);
    assert_eq!(
        svc.add_problem(
            setter.session,
            c.kind,
            c.id,
            own,
            ContestProblemInput::default()
        )
        .await
        .unwrap()
        .label,
        "A"
    );
    assert!(matches!(
        svc.add_problem(
            setter.session,
            c.kind,
            c.id,
            own,
            ContestProblemInput {
                label: Some("Z".into()),
                order: None
            }
        )
        .await,
        Err(ContestError::Conflict)
    ));
    assert!(matches!(
        svc.problems(None, c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
    svc.join_individual(viewer.session, c.kind, c.id, key("join"))
        .await
        .unwrap();
    assert!(matches!(
        svc.problems(Some(viewer.session), c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
    assert_eq!(
        svc.problems(Some(setter.session), c.kind, c.id)
            .await
            .unwrap()
            .len(),
        1
    );
    running(&pool, c.id).await;
    assert_eq!(svc.problems(None, c.kind, c.id).await.unwrap().len(), 1);
    let second = problem(&pool, Some(&setter), true).await;
    assert_eq!(
        svc.add_problem(
            setter.session,
            c.kind,
            c.id,
            second,
            ContestProblemInput::default()
        )
        .await
        .unwrap()
        .label,
        "B"
    );
    svc.remove_problem(setter.session, c.kind, c.id, own)
        .await
        .unwrap();
    ended(&pool, c.id).await;
    assert!(matches!(
        svc.remove_problem(setter.session, c.kind, c.id, second)
            .await,
        Err(ContestError::Conflict)
    ));
    assert!(matches!(
        svc.add_problem(
            setter.session,
            c.kind,
            c.id,
            own,
            ContestProblemInput::default()
        )
        .await,
        Err(ContestError::Conflict)
    ));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn individual_join_until_end_is_idempotent_and_revocation_cannot_be_replayed(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let user = actor(&pool, Some("standard-user")).await;
    let late = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    for c in [
        normal(&pool, &admin, &setter, ParticipationMode::Individual).await,
        svc.create(
            user.session,
            ContestKind::Virtual,
            key("virtual"),
            input(ParticipationMode::Individual),
        )
        .await
        .unwrap(),
    ] {
        running(&pool, c.id).await;
        let first = svc
            .join_individual(user.session, c.kind, c.id, key("join"))
            .await
            .unwrap();
        assert_eq!(
            svc.join_individual(user.session, c.kind, c.id, key("join"))
                .await
                .unwrap()
                .joined_at,
            first.joined_at
        );
        ended(&pool, c.id).await;
        assert!(matches!(
            svc.join_individual(late.session, c.kind, c.id, key("late"))
                .await,
            Err(ContestError::Conflict)
        ));
        svc.revoke_role(
            admin.session,
            c.kind,
            c.id,
            user.session.user_id,
            c.kind.participant(),
        )
        .await
        .unwrap();
        assert!(matches!(
            svc.join_individual(user.session, c.kind, c.id, key("join"))
                .await,
            Err(ContestError::Conflict)
        ));
    }
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn team_join_move_leave_and_post_start_rejection_are_atomic(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let a = actor(&pool, Some("standard-user")).await;
    let b = actor(&pool, Some("standard-user")).await;
    let late = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let c = normal(&pool, &admin, &setter, ParticipationMode::Team).await;
    let first = svc
        .join_team(
            a.session,
            c.id,
            key("join"),
            "チーム".into(),
            "secret".into(),
        )
        .await
        .unwrap();
    assert!(first.created);
    let joined = svc
        .join_team(
            b.session,
            c.id,
            key("join"),
            "チーム".into(),
            "secret".into(),
        )
        .await
        .unwrap();
    assert!(!joined.created);
    assert_eq!(first.team.id, joined.team.id);
    assert_eq!(joined.team.members.len(), 2);
    let different = svc
        .join_team(
            a.session,
            c.id,
            key("move"),
            "チーム".into(),
            "different".into(),
        )
        .await
        .unwrap();
    assert_ne!(different.team.id, first.team.id);
    assert_eq!(
        svc.team(b.session, c.id, first.team.id)
            .await
            .unwrap()
            .members
            .len(),
        1
    );
    assert!(matches!(
        svc.join_team(
            a.session,
            c.id,
            key("join"),
            "チーム".into(),
            "secret".into()
        )
        .await,
        Err(ContestError::Conflict)
    ));
    svc.leave_team(a.session, c.id).await.unwrap();
    svc.leave_team(a.session, c.id).await.unwrap();
    let hashes: Vec<String> = sqlx::query_scalar("SELECT password_hash FROM contest_teams")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        hashes
            .iter()
            .all(|hash| hash.starts_with("$argon2id$") && !hash.contains("secret"))
    );
    running(&pool, c.id).await;
    assert_eq!(
        svc.join_team(
            b.session,
            c.id,
            key("retry"),
            "チーム".into(),
            "secret".into()
        )
        .await
        .unwrap()
        .team
        .id,
        first.team.id
    );
    for (actor, name, password) in [(&late, "new", "new"), (&b, "チーム", "different")] {
        assert!(matches!(
            svc.join_team(
                actor.session,
                c.id,
                key("rejected"),
                name.into(),
                password.into()
            )
            .await,
            Err(ContestError::Conflict)
        ));
    }
    assert!(matches!(
        svc.leave_team(b.session, c.id).await,
        Err(ContestError::Conflict)
    ));
    assert_eq!(count(&pool, "contest_teams", c.id).await, 2);
    assert_eq!(count(&pool, "contest_team_memberships", c.id).await, 1);
    assert!(matches!(
        svc.team(admin.session, c.id, first.team.id).await,
        Err(ContestError::Forbidden)
    ));
    let another = normal(&pool, &admin, &setter, ParticipationMode::Team).await;
    assert!(matches!(
        svc.team(setter.session, another.id, first.team.id).await,
        Err(ContestError::NotFound)
    ));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn mode_changes_reset_roles_teams_and_invalidate_old_receipts(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let user = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let c = normal(&pool, &admin, &setter, ParticipationMode::Team).await;
    svc.join_team(
        user.session,
        c.id,
        key("old"),
        "team".into(),
        "secret".into(),
    )
    .await
    .unwrap();
    let before: String = sqlx::query_scalar("SELECT revision FROM users WHERE id=$1")
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let individual = svc
        .update(
            setter.session,
            c.kind,
            c.id,
            &c.revision,
            ContestChanges {
                participation_mode: Some(ParticipationMode::Individual),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(count(&pool, "contest_teams", c.id).await, 0);
    assert_eq!(count(&pool, "contest_team_memberships", c.id).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM account_roles WHERE scope_id=$1 AND role='contestant'"
        )
        .bind(c.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let after: String = sqlx::query_scalar("SELECT revision FROM users WHERE id=$1")
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(before, after);
    let team = svc
        .update(
            setter.session,
            c.kind,
            c.id,
            &individual.revision,
            ContestChanges {
                participation_mode: Some(ParticipationMode::Team),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_ne!(team.participation_epoch, c.participation_epoch);
    assert!(matches!(
        svc.join_team(
            user.session,
            c.id,
            key("old"),
            "team".into(),
            "secret".into()
        )
        .await,
        Err(ContestError::Conflict)
    ));
    assert_eq!(count(&pool, "contest_teams", c.id).await, 0);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn concurrent_team_joins_produce_one_team_and_one_membership_per_user(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let a = actor(&pool, Some("standard-user")).await;
    let b = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let c = normal(&pool, &admin, &setter, ParticipationMode::Team).await;
    let (one, two, repeat) = tokio::join!(
        svc.join_team(a.session, c.id, key("same"), "team".into(), "pass".into()),
        svc.join_team(b.session, c.id, key("same"), "team".into(), "pass".into()),
        svc.join_team(a.session, c.id, key("same"), "team".into(), "pass".into())
    );
    let one = one.unwrap();
    assert_eq!(one.team.id, two.unwrap().team.id);
    assert_eq!(one.team.id, repeat.unwrap().team.id);
    assert_eq!(count(&pool, "contest_teams", c.id).await, 1);
    assert_eq!(count(&pool, "contest_team_memberships", c.id).await, 2);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn http_routes_share_auth_strict_json_and_typed_responses(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let standard = actor(&pool, Some("standard-user")).await;
    let router = app(&pool);
    let ac = Client::new(&router, Some(&admin));
    let sc = Client::new(&router, Some(&standard));
    let anonymous = Client::new(&router, None);
    let payload = json!({"title":"大会","participationMode":"individual","startsAt":(Utc::now()+Duration::hours(1)).to_rfc3339(),"endsAt":(Utc::now()+Duration::hours(3)).to_rfc3339(),"rules":{"tieBreak":"icpc","penaltyMinutes":5},"visibility":"public"});
    assert_eq!(
        anonymous
            .call(
                "POST",
                "/api/v1/contests",
                &[("idempotency-key", "create")],
                Some(payload.clone())
            )
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        ac.call(
            "POST",
            "/api/v1/contests",
            &[
                ("origin", "https://other.example"),
                ("idempotency-key", "create")
            ],
            Some(payload.clone())
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        ac.call("POST", "/api/v1/contests", &[], Some(payload.clone()))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    let mut invalid = payload.clone();
    invalid["rules"]["surprise"] = json!(true);
    assert_eq!(
        ac.call(
            "POST",
            "/api/v1/contests",
            &[("idempotency-key", "invalid")],
            Some(invalid)
        )
        .await
        .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut invalid = payload.clone();
    invalid["rules"]["penaltyMinutes"] = Value::Null;
    assert_eq!(
        ac.call(
            "POST",
            "/api/v1/contests",
            &[("idempotency-key", "invalid")],
            Some(invalid)
        )
        .await
        .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let created = ac
        .call(
            "POST",
            "/api/v1/contests",
            &[("idempotency-key", "create")],
            Some(payload.clone()),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.body["id"].as_str().unwrap();
    let url = format!("/api/v1/contests/{id}");
    assert!(created.body.get("team_salt").is_none());
    assert!(created.body.get("participation_epoch").is_none());
    assert_eq!(
        ac.call(
            "POST",
            "/api/v1/contests",
            &[("idempotency-key", "create")],
            Some(payload)
        )
        .await
        .body,
        created.body
    );
    assert_eq!(
        ac.call("PUT", &format!("{url}/admins/{}", admin.id), &[], None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        ac.call("PUT", &format!("{url}/setters/{}", standard.id), &[], None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        sc.call("PATCH", &url, &[], Some(json!({"description":"更新"})))
            .await
            .status,
        StatusCode::PRECONDITION_REQUIRED
    );
    let updated = sc
        .call(
            "PATCH",
            &url,
            &[("if-match", created.headers["etag"].to_str().unwrap())],
            Some(json!({"description":"更新","rules":{"tieBreak":"atcoder","penaltyMinutes":0}})),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_ne!(updated.headers["etag"], created.headers["etag"]);
    assert_eq!(
        sc.call(
            "PATCH",
            &url,
            &[("if-match", created.headers["etag"].to_str().unwrap())],
            Some(json!({"description":"古い"}))
        )
        .await
        .status,
        StatusCode::PRECONDITION_FAILED
    );
    let problem = problem(&pool, Some(&standard), false).await;
    assert_eq!(
        sc.call(
            "PUT",
            &format!("{url}/problems/{problem}"),
            &[],
            Some(json!({"label":"A","order":0}))
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        anonymous
            .call("GET", &format!("{url}/problems"), &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        sc.call(
            "POST",
            &format!("{url}/participation"),
            &[("idempotency-key", "join")],
            None
        )
        .await
        .status,
        StatusCode::CREATED
    );
    let mut virtual_payload = json!({"title":"練習","startsAt":(Utc::now()+Duration::hours(1)).to_rfc3339(),"endsAt":(Utc::now()+Duration::hours(3)).to_rfc3339(),"rules":{"tieBreak":"icpc","penaltyMinutes":0}});
    let virtual_c = sc
        .call(
            "POST",
            "/api/v1/virtual-contests",
            &[("idempotency-key", "virtual")],
            Some(virtual_payload.clone()),
        )
        .await;
    assert_eq!(virtual_c.status, StatusCode::CREATED);
    virtual_payload["participationMode"] = json!("team");
    assert_eq!(
        sc.call(
            "POST",
            "/api/v1/virtual-contests",
            &[("idempotency-key", "invalid")],
            Some(virtual_payload)
        )
        .await
        .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let vurl = format!(
        "/api/v1/virtual-contests/{}",
        virtual_c.body["id"].as_str().unwrap()
    );
    assert_eq!(
        sc.call("GET", &vurl, &[], None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        anonymous.call("GET", &vurl, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        sc.call("DELETE", &vurl, &[], None).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        anonymous
            .call("GET", "/api/v1/contests/not-a-uuid", &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn public_pagination_excludes_private_and_deleted_and_binds_kind(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let standard = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let c = svc
        .create(
            admin.session,
            ContestKind::Regular,
            key("one"),
            input(ParticipationMode::Individual),
        )
        .await
        .unwrap();
    svc.create(
        admin.session,
        ContestKind::Regular,
        key("two"),
        input(ParticipationMode::Individual),
    )
    .await
    .unwrap();
    let mut private = input(ParticipationMode::Individual);
    private.visibility = ContestVisibility::Private;
    svc.create(admin.session, ContestKind::Regular, key("private"), private)
        .await
        .unwrap();
    let deleted = svc
        .create(
            admin.session,
            ContestKind::Regular,
            key("deleted"),
            input(ParticipationMode::Individual),
        )
        .await
        .unwrap();
    svc.delete(admin.session, deleted.kind, deleted.id)
        .await
        .unwrap();
    svc.create(
        standard.session,
        ContestKind::Virtual,
        key("virtual"),
        input(ParticipationMode::Individual),
    )
    .await
    .unwrap();
    let router = app(&pool);
    let client = Client::new(&router, None);
    let first = client
        .call("GET", "/api/v1/contests?limit=1", &[], None)
        .await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.body["items"][0]["id"], c.id.to_string());
    let cursor = first.body["nextCursor"].as_str().unwrap();
    let next = client
        .call(
            "GET",
            &format!("/api/v1/contests?limit=1&cursor={cursor}"),
            &[],
            None,
        )
        .await;
    assert_eq!(next.body["items"].as_array().unwrap().len(), 1);
    assert!(next.body["nextCursor"].is_null());
    assert_eq!(
        client
            .call(
                "GET",
                &format!("/api/v1/virtual-contests?cursor={cursor}"),
                &[],
                None
            )
            .await
            .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        client
            .call("GET", "/api/v1/contests?limit=101", &[], None)
            .await
            .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn contest_setter_inherits_private_problem_access_only_while_assigned(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, Some("standard-user")).await;
    let peer = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let c = normal(&pool, &admin, &setter, ParticipationMode::Individual).await;
    let problem = problem(&pool, Some(&setter), false).await;
    svc.grant_role(
        admin.session,
        c.kind,
        c.id,
        peer.session.user_id,
        RoleName::ContestSetter,
    )
    .await
    .unwrap();
    let router = app(&pool);
    let client = Client::new(&router, Some(&peer));
    let url = format!("/api/v1/problems/{problem}/editorials");
    assert_eq!(
        client.call("GET", &url, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    svc.add_problem(
        setter.session,
        c.kind,
        c.id,
        problem,
        ContestProblemInput::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        client.call("GET", &url, &[], None).await.status,
        StatusCode::OK
    );
    svc.remove_problem(setter.session, c.kind, c.id, problem)
        .await
        .unwrap();
    assert_eq!(
        client.call("GET", &url, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    svc.add_problem(
        setter.session,
        c.kind,
        c.id,
        problem,
        ContestProblemInput::default(),
    )
    .await
    .unwrap();
    svc.delete(admin.session, c.kind, c.id).await.unwrap();
    assert_eq!(
        client.call("GET", &url, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn failed_mode_change_rolls_back_reset_and_stale_sessions_cannot_mutate(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let user = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let c = normal(&pool, &admin, &setter, ParticipationMode::Team).await;
    svc.join_team(
        user.session,
        c.id,
        key("join"),
        "team".into(),
        "pass".into(),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_contest_update() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected failure'; END $$; CREATE TRIGGER reject_contest BEFORE UPDATE ON contests FOR EACH ROW EXECUTE FUNCTION reject_contest_update();").execute(&pool).await.unwrap();
    assert!(matches!(
        svc.update(
            setter.session,
            c.kind,
            c.id,
            &c.revision,
            ContestChanges {
                participation_mode: Some(ParticipationMode::Individual),
                ..Default::default()
            }
        )
        .await,
        Err(ContestError::Storage(_))
    ));
    assert_eq!(count(&pool, "contest_teams", c.id).await, 1);
    assert_eq!(count(&pool, "contest_team_memberships", c.id).await, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM account_roles WHERE scope_id=$1 AND role='contestant'"
        )
        .bind(c.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let current = svc.get(Some(setter.session), c.kind, c.id).await.unwrap();
    assert_eq!(current.revision, c.revision);
    assert_eq!(current.participation_epoch, c.participation_epoch);
    sqlx::query("UPDATE web_sessions SET revoked_at=clock_timestamp() WHERE id=$1")
        .bind(setter.session.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        svc.update(
            setter.session,
            c.kind,
            c.id,
            &c.revision,
            ContestChanges {
                description: Some("更新".into()),
                ..Default::default()
            }
        )
        .await,
        Err(ContestError::Unauthenticated)
    ));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn rejoining_same_team_invalidates_old_receipt_and_http_hides_credentials(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let user = actor(&pool, Some("standard-user")).await;
    let c = normal(&pool, &admin, &setter, ParticipationMode::Team).await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&user));
    let url = format!("/api/v1/contests/{}/teams", c.id);
    let request = json!({"name":"チーム","password":"private-password"});
    let joined = client
        .call(
            "POST",
            &url,
            &[("idempotency-key", "first")],
            Some(request.clone()),
        )
        .await;
    assert_eq!(joined.status, StatusCode::CREATED);
    assert!(!joined.body.to_string().contains("password"));
    assert!(!joined.body.to_string().contains("argon2"));
    assert_eq!(
        client
            .call(
                "DELETE",
                &format!("/api/v1/contests/{}/participation", c.id),
                &[],
                None
            )
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let new = client
        .call(
            "POST",
            &url,
            &[("idempotency-key", "again")],
            Some(request.clone()),
        )
        .await;
    assert_eq!(new.status, StatusCode::OK);
    assert_ne!(new.body["joinedAt"], joined.body["joinedAt"]);
    assert_eq!(
        client
            .call("POST", &url, &[("idempotency-key", "first")], Some(request))
            .await
            .status,
        StatusCode::CONFLICT
    );
    let team_id = new.body["team"]["id"].as_str().unwrap();
    let team = client
        .call("GET", &format!("{url}/{team_id}"), &[], None)
        .await;
    assert_eq!(team.status, StatusCode::OK);
    assert_eq!(team.body["members"][0]["userId"], user.id.to_string());
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn receipt_write_failure_rolls_back_created_contest_and_auto_role(pool: PgPool) {
    let user = actor(&pool, Some("standard-user")).await;
    sqlx::raw_sql("CREATE FUNCTION reject_receipt() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected receipt failure'; END $$; CREATE TRIGGER reject_receipt BEFORE INSERT ON contest_request_receipts FOR EACH ROW EXECUTE FUNCTION reject_receipt();").execute(&pool).await.unwrap();
    assert!(matches!(
        service(&pool)
            .create(
                user.session,
                ContestKind::Virtual,
                key("failure"),
                input(ParticipationMode::Individual)
            )
            .await,
        Err(ContestError::Storage(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM contests")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM account_roles WHERE role='virtual-contest-setter'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn deleted_problem_slots_do_not_block_defaults_and_private_lists_require_participation(
    pool: PgPool,
) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, None).await;
    let user = actor(&pool, Some("standard-user")).await;
    let other = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let mut request = input(ParticipationMode::Individual);
    request.visibility = ContestVisibility::Private;
    let c = svc
        .create(admin.session, ContestKind::Regular, key("private"), request)
        .await
        .unwrap();
    svc.grant_role(
        admin.session,
        c.kind,
        c.id,
        setter.session.user_id,
        RoleName::ContestSetter,
    )
    .await
    .unwrap();
    svc.grant_role(
        admin.session,
        c.kind,
        c.id,
        user.session.user_id,
        RoleName::Contestant,
    )
    .await
    .unwrap();
    let first = problem(&pool, Some(&setter), true).await;
    let second = problem(&pool, Some(&setter), true).await;
    svc.add_problem(
        setter.session,
        c.kind,
        c.id,
        first,
        ContestProblemInput::default(),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE problems SET deleted_at=clock_timestamp() WHERE id=$1")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        svc.add_problem(
            setter.session,
            c.kind,
            c.id,
            second,
            ContestProblemInput::default()
        )
        .await
        .unwrap()
        .label,
        "B"
    );
    assert!(matches!(
        svc.problems(Some(user.session), c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
    running(&pool, c.id).await;
    assert_eq!(
        svc.problems(Some(user.session), c.kind, c.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        svc.problems(Some(other.session), c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
    assert!(matches!(
        svc.problems(None, c.kind, c.id).await,
        Err(ContestError::NotFound)
    ));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn title_and_visibility_updates_require_setter_and_preserve_participation(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, Some("standard-user")).await;
    let participant = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let router = app(&pool);
    let client = Client::new(&router, Some(&setter));
    let administrator = Client::new(&router, Some(&admin));
    let anonymous = Client::new(&router, None);
    for kind in [ContestKind::Regular, ContestKind::Virtual] {
        let (contest, collection) = match kind {
            ContestKind::Regular => (
                normal(&pool, &admin, &setter, ParticipationMode::Individual).await,
                "/api/v1/contests",
            ),
            ContestKind::Virtual => (
                svc.create(
                    setter.session,
                    kind,
                    key("virtual"),
                    input(ParticipationMode::Individual),
                )
                .await
                .unwrap(),
                "/api/v1/virtual-contests",
            ),
        };
        svc.join_individual(participant.session, kind, contest.id, key("join"))
            .await
            .unwrap();
        let url = format!("{collection}/{}", contest.id);
        let initial = client.call("GET", &url, &[], None).await;
        let change = json!({"title":"新しい大会名", "visibility":"private"});
        let initial_etag = initial.headers["etag"].to_str().unwrap();
        assert_eq!(
            administrator
                .call(
                    "PATCH",
                    &url,
                    &[("if-match", initial_etag)],
                    Some(change.clone())
                )
                .await
                .status,
            StatusCode::FORBIDDEN
        );
        let hidden = client
            .call("PATCH", &url, &[("if-match", initial_etag)], Some(change))
            .await;
        assert_eq!(hidden.status, StatusCode::OK);
        assert_eq!(hidden.body["title"], "新しい大会名");
        assert_eq!(hidden.body["visibility"], "private");
        assert_ne!(hidden.headers["etag"], initial.headers["etag"]);
        assert_eq!(client.call("GET", &url, &[], None).await.body, hidden.body);
        assert_eq!(
            anonymous.call("GET", &url, &[], None).await.status,
            StatusCode::NOT_FOUND
        );
        let list = anonymous.call("GET", collection, &[], None).await;
        assert!(
            list.body["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["id"] != contest.id.to_string())
        );
        let current = svc
            .get(Some(setter.session), kind, contest.id)
            .await
            .unwrap();
        assert_eq!(current.participation_epoch, contest.participation_epoch);
        let role_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM account_roles WHERE scope_id=$1 AND user_id=$2 AND role=$3",
        )
        .bind(contest.id)
        .bind(participant.id)
        .bind(kind.participant().as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(role_count, 1);
        assert_eq!(
            client
                .call(
                    "PATCH",
                    &url,
                    &[("if-match", initial_etag)],
                    Some(json!({"title":"古い版"}))
                )
                .await
                .status,
            StatusCode::PRECONDITION_FAILED
        );
        let published = client
            .call(
                "PATCH",
                &url,
                &[("if-match", hidden.headers["etag"].to_str().unwrap())],
                Some(json!({"visibility":"public"})),
            )
            .await;
        assert_eq!(published.status, StatusCode::OK);
        assert_eq!(published.body["title"], hidden.body["title"]);
        assert_eq!(
            anonymous.call("GET", &url, &[], None).await.body,
            published.body
        );
        let list = anonymous.call("GET", collection, &[], None).await;
        assert!(
            list.body["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["id"] == contest.id.to_string())
        );
    }
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn title_and_visibility_validation_and_start_freeze_are_atomic(pool: PgPool) {
    let admin = actor(&pool, Some("service-admin")).await;
    let setter = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let router = app(&pool);
    let client = Client::new(&router, Some(&setter));
    for kind in [ContestKind::Regular, ContestKind::Virtual] {
        let (contest, collection) = match kind {
            ContestKind::Regular => (
                normal(&pool, &admin, &setter, ParticipationMode::Individual).await,
                "/api/v1/contests",
            ),
            ContestKind::Virtual => (
                svc.create(
                    setter.session,
                    kind,
                    key("virtual"),
                    input(ParticipationMode::Individual),
                )
                .await
                .unwrap(),
                "/api/v1/virtual-contests",
            ),
        };
        let url = format!("{collection}/{}", contest.id);
        let initial = client.call("GET", &url, &[], None).await;
        let tag = initial.headers["etag"].to_str().unwrap();
        for body in [
            json!({"title":""}),
            json!({"title":"長".repeat(201)}),
            json!({"title":"bad\u{0000}title"}),
            json!({"title":null}),
            json!({"visibility":null}),
            json!({"visibility":"secret"}),
        ] {
            assert_eq!(
                client
                    .call("PATCH", &url, &[("if-match", tag)], Some(body))
                    .await
                    .status,
                StatusCode::UNPROCESSABLE_ENTITY
            );
        }
        assert_eq!(client.call("GET", &url, &[], None).await.body, initial.body);
        assert_eq!(
            client
                .call("PATCH", &url, &[], Some(json!({"title":"変更"})))
                .await
                .status,
            StatusCode::PRECONDITION_REQUIRED
        );
        running(&pool, contest.id).await;
        let started = client.call("GET", &url, &[], None).await;
        let new_end = (Utc::now() + Duration::hours(4)).to_rfc3339();
        for body in [
            json!({"title":"開始後の変更","endsAt":new_end}),
            json!({"visibility":"private","endsAt":new_end}),
            json!({"title":"制約回避","startsAt":(Utc::now()+Duration::hours(1)).to_rfc3339(),"endsAt":new_end}),
        ] {
            assert_eq!(
                client
                    .call("PATCH", &url, &[("if-match", tag)], Some(body))
                    .await
                    .status,
                StatusCode::CONFLICT
            );
            assert_eq!(client.call("GET", &url, &[], None).await.body, started.body);
        }
        let mut allowed = json!({"endsAt":new_end});
        if kind == ContestKind::Regular {
            allowed["description"] = json!("開催中の案内");
        }
        let updated = client
            .call("PATCH", &url, &[("if-match", tag)], Some(allowed))
            .await;
        assert_eq!(updated.status, StatusCode::OK);
        assert_eq!(updated.body["title"], started.body["title"]);
        assert_eq!(updated.body["visibility"], started.body["visibility"]);
    }
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn virtual_participation_uses_shared_times_and_new_keys_do_not_restart(pool: PgPool) {
    let creator = actor(&pool, Some("standard-user")).await;
    let participant = actor(&pool, Some("standard-user")).await;
    let svc = service(&pool);
    let contest = svc
        .create(
            creator.session,
            ContestKind::Virtual,
            key("virtual"),
            input(ParticipationMode::Individual),
        )
        .await
        .unwrap();
    let initial = svc.get(None, contest.kind, contest.id).await.unwrap();
    let before = svc
        .join_individual(participant.session, contest.kind, contest.id, key("before"))
        .await
        .unwrap();
    let again = svc
        .join_individual(
            participant.session,
            contest.kind,
            contest.id,
            key("different-key"),
        )
        .await
        .unwrap();
    assert_eq!(before.joined_at, again.joined_at);
    let current = svc.get(None, contest.kind, contest.id).await.unwrap();
    assert_eq!(initial.starts_at, current.starts_at);
    assert_eq!(initial.ends_at, current.ends_at);
    assert_eq!(initial.revision, current.revision);
    running(&pool, contest.id).await;
    let during = svc
        .join_individual(participant.session, contest.kind, contest.id, key("during"))
        .await
        .unwrap();
    assert_eq!(before.joined_at, during.joined_at);
    ended(&pool, contest.id).await;
    let after = svc
        .join_individual(participant.session, contest.kind, contest.id, key("after"))
        .await
        .unwrap();
    assert_eq!(before.joined_at, after.joined_at);
    let roles: i64 = sqlx::query_scalar("SELECT count(*) FROM account_roles WHERE scope_id=$1 AND user_id=$2 AND role='virtual-contestant'")
        .bind(contest.id).bind(participant.id).fetch_one(&pool).await.unwrap();
    assert_eq!(roles, 1);
}
