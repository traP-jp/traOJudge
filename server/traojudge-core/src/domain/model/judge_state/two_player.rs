pub enum TPJudgeSystemState {
    Judging(TPJudgeJudging),
    SystemOk(TPJudgeUserResult),
    InternalError(TPJudgeInternalError),
    WriterError(TPJudgeWriterError),
}

pub struct TPJudgeJudging {
    pub progress_now: u32,
    pub progress_full: u32,
}

pub struct TPJudgeUserResult {
    alice_result: TPJudgeHalfUserResult,
    bob_result: TPJudgeHalfUserResult,
}

pub struct TPJudgeHalfUserResult {
    score: i64,
    max_time_ms: f64,
    max_memory_byte: f64,
    status: TPJudgeSystemOkStatus,
}

pub enum TPJudgeSystemOkStatus {
    AC,
    WA,
    CE,
    MLE,
    RE,
    TLE,
}

pub struct TPJudgeInternalError {
    pub message: String,
}

pub struct TPJudgeWriterError {
    pub message: String,
}
