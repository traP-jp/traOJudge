-- 解説の親を検査するための最小基盤。本文・投稿者・版などは問題側の実装時に追加する。
CREATE TABLE problems (
    id UUID PRIMARY KEY,
    visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('public', 'private')),
    deleted_at TIMESTAMPTZ
);

-- 既存の孤立した解説がある場合は移行を失敗させ、架空の問題は補わない。
ALTER TABLE editorials ADD CONSTRAINT editorials_problem_id_fkey
    FOREIGN KEY (problem_id) REFERENCES problems(id) ON DELETE RESTRICT;

-- 問題の論理削除と解説の取り下げを同じトランザクションで確定する。
CREATE FUNCTION delete_problem_editorials() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    UPDATE editorials
        SET deleted_at = NEW.deleted_at, revision = gen_random_uuid()::text
        WHERE problem_id = NEW.id AND deleted_at IS NULL;
    RETURN NEW;
END;
$$;
CREATE TRIGGER problem_editorial_deletion AFTER UPDATE OF deleted_at ON problems
    FOR EACH ROW WHEN (OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL)
    EXECUTE FUNCTION delete_problem_editorials();
