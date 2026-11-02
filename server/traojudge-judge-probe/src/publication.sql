ALTER TABLE judge_probe_submissions ADD COLUMN IF NOT EXISTS published_at TIMESTAMPTZ;
CREATE OR REPLACE FUNCTION judge_probe_stamp_publication() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.published AND NOT OLD.published THEN NEW.published_at=clock_timestamp(); END IF;
    RETURN NEW;
END $$;
CREATE OR REPLACE TRIGGER probe_stamp_publication BEFORE UPDATE ON judge_probe_submissions
    FOR EACH ROW EXECUTE FUNCTION judge_probe_stamp_publication();
