use chrono::{DateTime, Utc};

/// 解説から参照する問題の最小情報。問題本文や採点設定とは独立する。
pub struct ProblemReference {
    pub visibility: ProblemVisibility,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemVisibility {
    Public,
    Private,
}
