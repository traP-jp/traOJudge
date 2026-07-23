model User {
  id: string;
  name: string;
}

model Team {
  id: string;
  users: User[];
  name: string;
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
