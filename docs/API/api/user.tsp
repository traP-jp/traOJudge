import "@typespec/http";
import "../models/user.tsp";

using Http;
namespace traOJudge;

@route("/users")
@tag("Users")
namespace Users {
  @route("{id}") 
  @get op getUser(@path id: string): User | Error;

  @route("me")
  namespace Me {
    @get
    op getMe(): User | Error;

    @put
    op updateMe(@body input: User): User | Error;

    @route("email")
    namespace Email {
      @put
      op updateEmail(@body input: User): User | Error;
    }

    @route("password")
    namespace Password {
      @put
      op updatePassword(@body input: User): User | Error;
    }

    @route("devTokens")
    namespace DevTokens {
      @get
      op getDevTokens(): DevTokens | Error;

      @post
      op createDevToken(@body input: PostDevTokenRequest): DevTokenIssueResponse | Error;

      @route("{id}")
      namespace DevToken {
        @delete
        op deleteDevToken(@path id: string): void | Error;
      }
    }
  }
}