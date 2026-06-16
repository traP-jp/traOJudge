#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
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
