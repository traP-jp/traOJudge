-- The disposable probe has its own DB role. Do not leave row/session locks
-- indefinitely when a worker is frozen or a network connection disappears.
DO $$ BEGIN
    EXECUTE format('ALTER ROLE %I SET idle_in_transaction_session_timeout=%L',current_user,'30s');
    EXECUTE format('ALTER ROLE %I SET idle_session_timeout=%L',current_user,'90s');
    EXECUTE format('ALTER ROLE %I SET lock_timeout=%L',current_user,'5s');
    EXECUTE format('ALTER ROLE %I SET statement_timeout=%L',current_user,'20s');
END $$;
