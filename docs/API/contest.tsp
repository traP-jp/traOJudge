@route("/contests")
@tag("Contests")
interface Contests {
  /** List public contests */
  @get list(): ContestSummary[] | Error;

  /** Participate to a contest */
  @post participate(@path id: string): Contest | Error;
}