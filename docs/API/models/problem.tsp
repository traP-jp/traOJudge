import "./user.tsp";

model ProblemSummary {
  id: string;
  writer: User;
  title: string;
  published_at: utcDateTime;
}

model InContestProblemSummary {
  id: string;
  title: string;
}

model Problem {
  id: string;
  title: string;
  statement: string;
  languages: string;
}

model ProblemInContestSummary {

}