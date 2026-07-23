import "./problem.tsp";



model ContestSummary {
  id: string;
  name: string;
  start_at: utcDateTime;
  end_at: utcDateTime;
}

model Penalty {
  time_sec: int32;
  base: "sum" | "max";
}

model Contest {
  id: string;
  name: string;
  start_at: utcDateTime;
  end_at: utcDateTime;
  penalty: Penalty;
  description: string;
  problems: ProblemInContestSummary[];
  status: "unstarted" | "running" | "finished";
}
