import "@typespec/http";

using Http;
@service(#{ title: "Widget Service" })
namespace DemoService;

model User {
  id: string;
  name: string;
}

model Team {
  id: string;
  users: User[];
  name: string;
}

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

model ContestSummary {
  id: string;
  name: string;
  start_at: utcDateTime;
  end_at: utcDateTime;
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

model Penalty {
  time_sec: int32;
  base: "sum" | "max";
}

model Problem {
  id: string;
  title: string;
  statement: string;
  languages: string;
}

model SubmissionSummary {
  id: string;
  user: User;
  submitted_at: utcDateTime;
  max_time_ms: int32;
  max_memory_kb: i32;
  code_length: i32;
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

model DevToken {
  id: string;
  name: string;
  created_at: utcDateTime;
  expired_at: utcDateTime;
  last_used_at: utcDateTime;
}

model DevTokens {
  total: int32;
  dev_tokens: DevToken[];
}

model PostDevTokenRequest {
  name: string;
}

model DevTokenIssueResponse {
  token: string;
  dev_token: DevToken;
}

@error
model Error {
  code: int32;
  message: string;
}



@route("/widgets")
@tag("Widgets")
interface Widgets {
  /** List widgets */
  @get list(): WidgetList | Error;
  /** Read widgets */
  @get read(@path id: string): Widget | Error;
  /** Create a widget */
  @post create(@body body: Widget): Widget | Error;
  /** Update a widget */
  @patch update(@path id: string, @body body: MergePatchUpdate<Widget>): Widget | Error;
  /** Delete a widget */
  @delete delete(@path id: string): void | Error;

  /** Analyze a widget */
  @route("{id}/analyze") @post analyze(@path id: string): AnalyzeResult | Error;
}
