@route("/users")
@tag("Users")
interface Users {
  @route("{id}") @get getUser(@path id: string): User | Error;

  @route("me")
  namespace Me {
    @get
    op getMe(): User | Error;

    @put
    op updateMe(@body input: UpdateUser): User | Error;

    @route("email")
    namespace Email {
      @put
      op updateEmail(@body input: UpdateEmail): User | Error;
    }

    @route("password")
    namespace Password {
      @put
      op updatePassword(@body input: UpdatePassword): User | Error;
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