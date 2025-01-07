use trao_judge_backend::Repository;

#[sqlx::test]
async fn upload_and_get_object(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let state = Repository::create_by_pool(pool).await?;

    let id_and_value = vec![
        ("test", "test_value"),
        ("test2", "test_value2"),
        ("test3", "test_value3"),
    ];

    for (id, value) in &id_and_value {
        state.upload_object(id, value).await?;
    }

    for (id, value) in &id_and_value {
        let resp = state.get_object(id).await?;
        assert_eq!(resp, *value);
    }

    Ok(())
}
