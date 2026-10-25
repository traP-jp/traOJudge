use super::schema;
use traojudge_core::domain::model::{account::*, user::PublicUserProfile};

impl From<PublicUserProfile> for schema::Profile {
    fn from(p: PublicUserProfile) -> Self {
        Self::new(
            p.id.as_uuid(),
            p.display_name,
            p.bio,
            p.github_id,
            p.atcoder_id,
            p.x_id,
            p.revision,
        )
    }
}
impl From<CurrentAccount> for schema::CurrentUser {
    fn from(account: CurrentAccount) -> Self {
        let p = account.profile;
        Self::new(
            p.id.as_uuid(),
            p.display_name,
            p.bio,
            p.github_id,
            p.atcoder_id,
            p.x_id,
            p.revision,
            account.roles.into_iter().map(Into::into).collect(),
        )
    }
}
impl From<AccountRole> for schema::RoleAssignment {
    fn from(role: AccountRole) -> Self {
        use schema::role_assignment::Role;
        let name = match role.role {
            RoleName::ServiceAdmin => Role::ServiceAdmin,
            RoleName::Writer => Role::Writer,
            RoleName::StandardUser => Role::StandardUser,
            RoleName::ProblemOwner => Role::ProblemOwner,
            RoleName::Tester => Role::Tester,
            RoleName::ContestAdmin => Role::ContestAdmin,
            RoleName::ContestSetter => Role::ContestSetter,
            RoleName::VirtualContestSetter => Role::VirtualContestSetter,
            RoleName::Contestant => Role::Contestant,
            RoleName::VirtualContestant => Role::VirtualContestant,
            RoleName::EditorialAuthor => Role::EditiorialAuthor,
        };
        let scope = match role.scope {
            RoleScope::Service => schema::Scope::ScopeOneOf(schema::ScopeOneOf::new(
                schema::scope_one_of::Kind::Service,
            )),
            RoleScope::Problem(id) => schema::Scope::ScopeOneOf1(schema::ScopeOneOf1::new(
                schema::scope_one_of_1::Kind::Problem,
                id,
            )),
            RoleScope::Contest(id) => schema::Scope::ScopeOneOf2(schema::ScopeOneOf2::new(
                schema::scope_one_of_2::Kind::Contest,
                id,
            )),
            RoleScope::VirtualContest(id) => schema::Scope::ScopeOneOf3(schema::ScopeOneOf3::new(
                schema::scope_one_of_3::Kind::VirtualContest,
                id,
            )),
            RoleScope::Editorial(id) => schema::Scope::ScopeOneOf4(schema::ScopeOneOf4::new(
                schema::scope_one_of_4::Kind::Editorial,
                id,
            )),
        };
        Self::new(
            name,
            scope,
            role.user_id.as_uuid(),
            role.granted_at.to_rfc3339(),
        )
    }
}
impl From<SignInMethod> for schema::SignInMethod {
    fn from(method: SignInMethod) -> Self {
        use schema::sign_in_method::Kind;
        let kind = match method.kind {
            SignInKind::Email => Kind::Email,
            SignInKind::External(Provider::Google) => Kind::Google,
            SignInKind::External(Provider::Github) => Kind::Github,
            SignInKind::External(Provider::Traq) => Kind::Traq,
        };
        Self::new(method.id, kind, method.created_at.to_rfc3339())
    }
}
impl From<DeveloperToken> for schema::DeveloperToken {
    fn from(token: DeveloperToken) -> Self {
        Self::new(
            token.id,
            token.name,
            token.created_at.to_rfc3339(),
            token.expires_at.map(|t| t.to_rfc3339()),
            token.revoked_at.map(|t| t.to_rfc3339()),
        )
    }
}
impl From<schema::ProfileUpdate> for ProfileChanges {
    fn from(p: schema::ProfileUpdate) -> Self {
        Self {
            display_name: p.display_name,
            bio: p.bio,
            github_id: p.github_id,
            atcoder_id: p.atcoder_id,
            x_id: p.x_id,
        }
    }
}
impl From<schema::ProviderId> for Provider {
    fn from(provider: schema::ProviderId) -> Self {
        match provider {
            schema::ProviderId::Google => Self::Google,
            schema::ProviderId::Github => Self::Github,
            schema::ProviderId::Traq => Self::Traq,
        }
    }
}

impl From<traojudge_core::domain::model::editorial::Editorial> for schema::Editorial {
    fn from(p: traojudge_core::domain::model::editorial::Editorial) -> Self {
        use traojudge_core::domain::model::editorial::EditorialVisibility;
        let visibility = match p.visibility {
            EditorialVisibility::Public => schema::VisibilityValue::Public,
            EditorialVisibility::Private => schema::VisibilityValue::Private,
        };
        Self::new(
            p.id,
            p.problem_id,
            p.author_id.as_uuid(),
            p.title,
            p.body,
            visibility,
            p.revision,
            p.created_at.to_rfc3339(),
            p.updated_at.to_rfc3339(),
        )
    }
}
impl From<schema::EditorialCreate> for traojudge_core::domain::model::editorial::NewEditorial {
    fn from(p: schema::EditorialCreate) -> Self {
        Self {
            problem_id: p.problem_id,
            title: p.title,
            body: p.body,
        }
    }
}
impl From<schema::EditorialUpdate> for traojudge_core::domain::model::editorial::EditorialChanges {
    fn from(p: schema::EditorialUpdate) -> Self {
        Self {
            title: p.title,
            body: p.body,
        }
    }
}

impl From<traojudge_core::domain::model::editorial::Editorial> for schema::Visibility {
    fn from(editorial: traojudge_core::domain::model::editorial::Editorial) -> Self {
        use traojudge_core::domain::model::editorial::EditorialVisibility;
        let visibility = match editorial.visibility {
            EditorialVisibility::Public => schema::VisibilityValue::Public,
            EditorialVisibility::Private => schema::VisibilityValue::Private,
        };
        Self::new(visibility, editorial.revision)
    }
}
