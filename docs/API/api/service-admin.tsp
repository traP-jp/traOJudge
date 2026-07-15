import "@typespec/http";
using Http;
namespace traOJudge;

model ContestPatchRequest {
    added_contest_admins: string[];
    removed_contest_admin: string[];
}

@tag("Service Admin")
namespace ServiceAdmin {
  /** Create a contest */
  @route("contests")
  @post op create(): OkResponse | Error;

  @route("contests/{id}")
  @patch op hoge(@path id: string): OkResponse | Error;

  /** Revoke problem create role */
  @route("users/{id}/problem-create/revoke")
  @post op revokeProblemCreate(@path id: string): OkResponse | Error;

  /** Grant problem create role */
  @route("users/{id}/problem-create/grant")
  @post op grantProblemCreate(@path id: string): OkResponse | Error;
}
