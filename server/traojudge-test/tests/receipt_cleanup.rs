use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use traojudge_core::{
    application::receipt_cleanup::ReceiptCleanupService,
    domain::model::request::{ReceiptCleanupBatchSize, ReceiptCleanupCounts, ReceiptCleanupError},
    ports::repository::receipt_cleanup::{
        ReceiptCleanupUnitOfWork, ReceiptCleanupUnitOfWorkProvider,
    },
};
use traojudge_infra::postgres::receipt_cleanup::PgReceiptCleanupUnitOfWorkProvider;
use uuid::Uuid;

struct Fixture {
    user: Uuid,
    editorial: Uuid,
    problem: Uuid,
    contest: Uuid,
}
async fn fixture(pool: &PgPool) -> Fixture {
    let fixture = Fixture {
        user: Uuid::new_v4(),
        editorial: Uuid::new_v4(),
        problem: Uuid::new_v4(),
        contest: Uuid::new_v4(),
    };
    sqlx::query(
        "INSERT INTO users (id,display_name,revision) VALUES ($1,'cleanup-test','initial')",
    )
    .bind(fixture.user)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO problems (id) VALUES ($1)")
        .bind(fixture.problem)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO editorials (id,problem_id,author_id,title,body,visibility,revision,created_at,updated_at) VALUES ($1,$2,$3,'title','body','private','initial',clock_timestamp(),clock_timestamp())")
        .bind(fixture.editorial).bind(fixture.problem).bind(fixture.user).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO contests (id,kind,created_by_user_id,title,visibility,participation_mode,starts_at,ends_at,tie_break,penalty_minutes,revision,participation_epoch,team_salt,created_at) VALUES ($1,'regular',$2,'title','private','individual',clock_timestamp()+INTERVAL '1 hour',clock_timestamp()+INTERVAL '2 hours','icpc',0,'initial',gen_random_uuid(),gen_random_uuid(),clock_timestamp())")
        .bind(fixture.contest).bind(fixture.user).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO account_roles (user_id,role,scope_kind,scope_id) VALUES ($1,'contest-setter','contest',$2)")
        .bind(fixture.user).bind(fixture.contest).execute(pool).await.unwrap();
    fixture
}
async fn receipts(pool: &PgPool, fixture: &Fixture, key: &str, expires_at: DateTime<Utc>) {
    sqlx::query("INSERT INTO editorial_creation_receipts (user_id,key,request_digest,editorial_id,problem_id,title,body,revision,created_at,expires_at) VALUES ($1,$2,decode(repeat('00',32),'hex'),$3,$4,'original title','original body','initial',clock_timestamp(),$5)")
        .bind(fixture.user).bind(key).bind(fixture.editorial).bind(fixture.problem).bind(expires_at).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO contest_request_receipts (user_id,operation,scope_id,key,request_digest,participation_epoch,result,expires_at) VALUES ($1,'create-regular',$2,$3,decode(repeat('00',32),'hex'),gen_random_uuid(),'{}'::jsonb,$4)")
        .bind(fixture.user).bind(fixture.contest).bind(key).bind(expires_at).execute(pool).await.unwrap();
}
fn service(
    pool: &PgPool,
    batch_size: u32,
) -> ReceiptCleanupService<PgReceiptCleanupUnitOfWorkProvider> {
    ReceiptCleanupService::new(
        PgReceiptCleanupUnitOfWorkProvider::new(pool.clone()),
        ReceiptCleanupBatchSize::new(batch_size).unwrap(),
    )
}
async fn counts(pool: &PgPool) -> (i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM editorial_creation_receipts), (SELECT count(*) FROM contest_request_receipts)").fetch_one(pool).await.unwrap()
}
async fn bounded_batch(pool: &PgPool, batch: u32) -> ReceiptCleanupCounts {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        service(pool, batch).run_batch(),
    )
    .await
    .expect("cleanup must skip active rows rather than wait")
    .unwrap()
}

#[test]
fn invalid_batch_sizes_are_rejected() {
    assert!(ReceiptCleanupBatchSize::new(0).is_err());
    assert!(ReceiptCleanupBatchSize::new(10_001).is_err());
    assert_eq!(ReceiptCleanupBatchSize::new(1).unwrap().get(), 1);
    assert_eq!(ReceiptCleanupBatchSize::new(10_000).unwrap().get(), 10_000);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn cleanup_includes_expiration_boundary_and_preserves_live_receipts_and_entities(
    pool: PgPool,
) {
    let fixture = fixture(&pool).await;
    let provider = PgReceiptCleanupUnitOfWorkProvider::new(pool.clone());
    let mut tx = provider.begin().await.unwrap();
    let cutoff = tx.now();
    receipts(
        &pool,
        &fixture,
        "expired",
        cutoff - Duration::microseconds(1),
    )
    .await;
    receipts(&pool, &fixture, "boundary", cutoff).await;
    receipts(&pool, &fixture, "live", cutoff + Duration::days(1)).await;
    let revision: String = sqlx::query_scalar("SELECT revision FROM users WHERE id=$1")
        .bind(fixture.user)
        .fetch_one(&pool)
        .await
        .unwrap();
    let batch = ReceiptCleanupBatchSize::new(10).unwrap();
    assert_eq!(
        tx.delete_expired_editorial_receipts(cutoff, batch)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        tx.delete_expired_contest_receipts(cutoff, batch)
            .await
            .unwrap(),
        2
    );
    tx.commit().await.unwrap();
    assert_eq!(counts(&pool).await, (1, 1));
    let body: String = sqlx::query_scalar(
        "SELECT body FROM editorial_creation_receipts WHERE user_id=$1 AND key='live'",
    )
    .bind(fixture.user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(body, "original body");
    let unchanged: (i64,i64,i64,String) = sqlx::query_as("SELECT (SELECT count(*) FROM editorials), (SELECT count(*) FROM contests), (SELECT count(*) FROM account_roles), revision FROM users WHERE id=$1")
        .bind(fixture.user).fetch_one(&pool).await.unwrap();
    assert_eq!(unchanged, (1, 1, 1, revision));
    assert_eq!(
        service(&pool, 10).run_batch().await.unwrap(),
        ReceiptCleanupCounts::default()
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn bounded_concurrent_cleanup_skips_api_users_without_losing_records(pool: PgPool) {
    let active = fixture(&pool).await;
    let free_one = fixture(&pool).await;
    let free_two = fixture(&pool).await;
    for fixture in [&active, &free_one, &free_two] {
        for key in ["one", "two", "three"] {
            receipts(&pool, fixture, key, Utc::now() - Duration::days(1)).await;
        }
    }
    let mut request = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(active.user)
        .execute(&mut *request)
        .await
        .unwrap();
    let first = bounded_batch(&pool, 2).await;
    assert_eq!(
        first,
        ReceiptCleanupCounts {
            editorial_receipts: 2,
            contest_receipts: 2
        }
    );
    assert_eq!(counts(&pool).await, (7, 7));
    let svc = service(&pool, 100);
    let (one, two) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(svc.run_batch(), svc.run_batch())
    })
    .await
    .unwrap();
    let (one, two) = (one.unwrap(), two.unwrap());
    assert_eq!(one.editorial_receipts + two.editorial_receipts, 4);
    assert_eq!(one.contest_receipts + two.contest_receipts, 4);
    assert_eq!(counts(&pool).await, (3, 3));
    let owners: Vec<Uuid> =
        sqlx::query_scalar("SELECT DISTINCT user_id FROM editorial_creation_receipts")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(owners, vec![active.user]);
    request.commit().await.unwrap();
    assert_eq!(
        bounded_batch(&pool, 100).await,
        ReceiptCleanupCounts {
            editorial_receipts: 3,
            contest_receipts: 3
        }
    );
    assert_eq!(counts(&pool).await, (0, 0));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn receipt_renewal_is_skipped_and_not_deleted_after_commit(pool: PgPool) {
    let fixture = fixture(&pool).await;
    receipts(&pool, &fixture, "renewed", Utc::now() - Duration::days(1)).await;
    let mut renewal = pool.begin().await.unwrap();
    sqlx::query("UPDATE editorial_creation_receipts SET expires_at=clock_timestamp()+INTERVAL '1 day' WHERE user_id=$1").bind(fixture.user).execute(&mut *renewal).await.unwrap();
    sqlx::query("UPDATE contest_request_receipts SET expires_at=clock_timestamp()+INTERVAL '1 day' WHERE user_id=$1").bind(fixture.user).execute(&mut *renewal).await.unwrap();
    assert_eq!(
        bounded_batch(&pool, 10).await,
        ReceiptCleanupCounts::default()
    );
    renewal.commit().await.unwrap();
    assert_eq!(
        bounded_batch(&pool, 10).await,
        ReceiptCleanupCounts::default()
    );
    assert_eq!(counts(&pool).await, (1, 1));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn failure_rolls_back_both_tables_and_next_run_can_recover(pool: PgPool) {
    let fixture = fixture(&pool).await;
    receipts(&pool, &fixture, "expired", Utc::now() - Duration::days(1)).await;
    sqlx::raw_sql("CREATE FUNCTION reject_receipt_cleanup() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected failure'; END $$; CREATE TRIGGER reject_receipt_cleanup BEFORE DELETE ON contest_request_receipts FOR EACH ROW EXECUTE FUNCTION reject_receipt_cleanup();").execute(&pool).await.unwrap();
    let result = service(&pool, 10).run_batch().await;
    assert!(matches!(result, Err(ReceiptCleanupError::Storage(_))));
    assert_eq!(counts(&pool).await, (1, 1));
    sqlx::query("DROP TRIGGER reject_receipt_cleanup ON contest_request_receipts")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        bounded_batch(&pool, 10).await,
        ReceiptCleanupCounts {
            editorial_receipts: 1,
            contest_receipts: 1
        }
    );
    assert_eq!(counts(&pool).await, (0, 0));
}
