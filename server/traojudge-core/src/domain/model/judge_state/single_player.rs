pub enum SPJudgeSystemState {
    Judging(SPJudgeJudging),
    SystemOk(SPJudgeUserResult),
    InternalError(SPJudgeInternalError),
    WriterError(SPJudgeWriterError),
}

pub struct SPJudgeJudging {
    pub progress_now: u32,
    pub progress_full: u32,
}

pub struct SPJudgeUserResult {
    score: i64,
    max_time_ms: f64,
    max_memory_byte: f64,
    status: SPJudgeSystemOkStatus,
}

pub enum SPJudgeSystemOkStatus {
    AC,
    WA,
    CE,
    MLE,
    RE,
    TLE,
}

pub struct SPJudgeInternalError {
    pub message: String,
}

pub struct SPJudgeWriterError {
    pub message: String,
}
