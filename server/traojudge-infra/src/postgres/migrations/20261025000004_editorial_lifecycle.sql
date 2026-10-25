-- 解説単体の削除と問題の削除に伴う取り下げは、履歴・再送記録を保持する論理削除とする。
ALTER TABLE editorials ADD COLUMN deleted_at TIMESTAMPTZ;
DROP INDEX editorials_public_page;
CREATE INDEX editorials_public_page ON editorials(problem_id, created_at, id)
    WHERE visibility = 'public' AND deleted_at IS NULL;
