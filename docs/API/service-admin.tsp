model ContestPatchRequest {
    added_contest_admins: string[];
    removed_contest_admin: string[];
}

@route("/service-admin")
@tag("Service Admin")
interface ServiceAdmin {
  /** Create a contest */
  @route("contests")
  @post create(): OkResponse | Error;

  @route("contests/{id}")
  @patch("")

  /** Revoke problem create role */
  @route("users/{id}/problem-create/revoke")
  @post revokeProblemCreate(@path id: string) OkResponse | Error;

  /** Grant problem create role */
  @route("users/{id}/problem-create/grant")
  @post grantProblemCreate(@path id: string) OkResponse | Error;
}
