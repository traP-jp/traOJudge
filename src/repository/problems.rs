use std::fmt;

use serde::Serialize;
use sqlx::FromRow;
use sqlx::Type;
use sqlx::{types::chrono, Decode, Encode, MySql};
use uuid::Uuid;

use super::Repository;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub id: String,
    pub title: String,
    pub author_id: i64,
    pub is_public: bool,
    pub difficulty: i64,
    pub statement: String,
    pub time_limit: i64,
    pub memory_limit: i64,
    pub solved_count: i64,
    // todo: add more fields
    //pub testcases:
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Repository {
    pub async fn post_problems(&self, user_id: i64, body: Problem) -> anyhow::Result<()> {
        let problem = Problem {
            id: Uuid::now_v7().to_string(),
            title: body.title,
            author_id: user_id,
            is_public: false,
            difficulty: body.difficulty,
            statement: body.statement,
            time_limit: body.time_limit,
            memory_limit: body.memory_limit,
            solved_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        // let problem_id = Uuid::new_v4().to_string();
        // sqlx::query("INSERT INTO problems (id, title, author_id, difficulty, statement, time_limit, memory_limit) VALUES (?, ?, ?, ?, ?, ?, ?)")
        //     .bind(problem_id)
        //     .bind(body.title)
        //     .bind(user_id)
        //     .bind(body.difficulty)
        //     .bind(body.statement)
        //     .bind(body.time_limit)
        //     .bind(body.memory_limit)
        //     .execute(&self.pool).await?;

        Ok(())
    }
}
