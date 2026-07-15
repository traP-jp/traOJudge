import "@typespec/http";
using Http;
namespace traOJudge;

@route("/contests")
@tag("Contests")
namespace Contests {
  /** List public contests */
  @get op list(): ContestSummary[] | Error;

  /** Participate to a contest */
  @post op participate(@path id: string): Contest | Error;
}