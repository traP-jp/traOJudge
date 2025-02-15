use async_session::chrono;
use serde::{Deserialize, Serialize};

use crate::usecase::model::submission::{JudgeResultDto, SubmissionDto};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubmissionResponse {
    pub id: String,
    pub user_id: i64,
    pub user_name: String,
    pub problem_id: i64,
    pub submitted_at: chrono::DateTime<chrono::Utc>,
    pub language_id: i32,
    pub total_score: i64,
    pub max_time: i32,
    pub max_memory: i32,
    pub code_length: i32,
    pub overall_judge_status: String,
    pub judge_results: Vec<JudgeResultResponse>,
}

impl From<SubmissionDto> for SubmissionResponse {
    fn from(val: SubmissionDto) -> Self {
        SubmissionResponse {
            id: val.id,
            user_id: val.user_id,
            user_name: val.user_name,
            problem_id: val.problem_id,
            submitted_at: val.submitted_at,
            language_id: val.language_id,
            total_score: val.total_score,
            max_time: val.max_time,
            max_memory: val.max_memory,
            code_length: val.code_length,
            overall_judge_status: val.overall_judge_status,
            judge_results: val.judge_results.into_iter().map(|x| x.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JudgeResultResponse {
    pub testcase_id: i64,
    pub testcase_name: String,
    pub judge_status: String,
    pub score: i64,
    pub time: i32,
    pub memory: i32,
}

impl From<JudgeResultDto> for JudgeResultResponse {
    fn from(val: JudgeResultDto) -> Self {
        JudgeResultResponse {
            testcase_id: val.testcase_id,
            testcase_name: val.testcase_name,
            judge_status: val.judge_status,
            score: val.score,
            time: val.time,
            memory: val.memory,
        }
    }
}
