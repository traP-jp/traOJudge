CREATE TABLE web_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX web_sessions_user_id ON web_sessions(user_id);

CREATE TABLE account_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_id UUID,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (role IN ('service-admin', 'writer', 'standard-user') AND scope_kind = 'service' AND scope_id IS NULL)
        OR (role IN ('problem-owner', 'tester') AND scope_kind = 'problem' AND scope_id IS NOT NULL)
        OR (role IN ('contest-admin', 'contest-setter', 'contestant') AND scope_kind = 'contest' AND scope_id IS NOT NULL)
        OR (role IN ('virtual-contest-setter', 'virtual-contestant') AND scope_kind = 'virtual-contest' AND scope_id IS NOT NULL)
        OR (role = 'editiorial-author' AND scope_kind = 'editorial' AND scope_id IS NOT NULL)
    ),
    UNIQUE NULLS NOT DISTINCT (user_id, role, scope_kind, scope_id)
);

-- /meの表現には役割も含む。役割が変わった場合も強いETagの版を進める。
CREATE FUNCTION update_account_role_revision() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        UPDATE users SET revision = gen_random_uuid()::text WHERE id = OLD.user_id;
    END IF;
    IF TG_OP <> 'DELETE' THEN
        UPDATE users SET revision = gen_random_uuid()::text WHERE id = NEW.user_id;
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER account_role_revision AFTER INSERT OR UPDATE OR DELETE ON account_roles
    FOR EACH ROW EXECUTE FUNCTION update_account_role_revision();

CREATE TABLE sign_in_methods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('email', 'google', 'github', 'traq')),
    subject TEXT NOT NULL CHECK (char_length(subject) BETWEEN 1 AND 512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (user_id, kind),
    UNIQUE (kind, subject)
);

CREATE TABLE developer_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX developer_tokens_user_id ON developer_tokens(user_id);

CREATE TABLE email_changes (
    token_digest BYTEA PRIMARY KEY CHECK (octet_length(token_digest) = 32),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ
);
CREATE INDEX email_changes_user_id ON email_changes(user_id);

CREATE TABLE account_authorizations (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES web_sessions(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('google', 'github', 'traq')),
    state_digest BYTEA NOT NULL CHECK (octet_length(state_digest) = 32),
    code_verifier TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    -- 完了済みの結果は認証手段を削除した後も残し、同じ認可操作で再追加させない。
    completed_method UUID
);
