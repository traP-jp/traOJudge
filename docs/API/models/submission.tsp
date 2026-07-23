import "./user.tsp";

model SubmissionSummary {
  id: string;
  user: User;
  submitted_at: utcDateTime;
  max_time_ms: int32;
  max_memory_kb: int32;
  code_length: int32;
  total_score: int32;
  judge_status: JudgeStatus;
}

model SubmissionInContest {
  id: string;
  user: User;
  submitted_at: utcDateTime;
  max_time_ms: int32;
  max_memory_kb: int32;
  code_length: int32;
}

model JudgeStatus {
  status: "AC" | "WA" | "TLE" | "MLE" | "RE" | "CE" | "WJ" | "IE" | "WE";
}
