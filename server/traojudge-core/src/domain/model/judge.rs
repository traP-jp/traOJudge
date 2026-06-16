#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JudgeStatus {
    AC,
    WA,
    CE,
    IE,
    MLE,
    RE,
    TLE,
    WJ,
    WE,
}
