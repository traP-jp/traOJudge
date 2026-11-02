-- The outbox sequence is NOT a client cursor: an older transaction may commit
-- later. Dispatch scans undelivered rows, then assigns a commit-safe cursor.
CREATE TABLE judge_probe_event_outbox (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    scope TEXT NOT NULL,
    submission UUID NOT NULL,
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    delivered BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX judge_probe_events_pending ON judge_probe_event_outbox(scope,id) WHERE NOT delivered;
CREATE TABLE judge_probe_event_streams (scope TEXT PRIMARY KEY, cursor BIGINT NOT NULL DEFAULT 0);
CREATE TABLE judge_probe_events (
    scope TEXT NOT NULL,
    cursor BIGINT NOT NULL,
    submission UUID NOT NULL,
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    PRIMARY KEY(scope,cursor)
);

CREATE FUNCTION judge_probe_capture_event() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE submission_id UUID; event_scope TEXT; event_kind TEXT; body JSONB; verdict TEXT;
BEGIN
    IF TG_TABLE_NAME='judge_probe_submissions' THEN
        submission_id=NEW.id;
        IF TG_OP='INSERT' THEN
            event_kind='submission.accepted'; body=jsonb_build_object('ordinal',NEW.ordinal);
        ELSIF NEW.published AND NOT OLD.published THEN
            SELECT q.result->>'verdict' INTO verdict FROM judge_probe_queue q
                JOIN workflow_jobs j ON j.id=q.id WHERE j.pipeline_id=NEW.id::text
                AND q.payload->>'phase'='check' AND q.status='done' LIMIT 1;
            event_kind='result.published'; body=jsonb_build_object('ordinal',NEW.ordinal,
                'verdict',coalesce(verdict,'SYSTEM_ERROR'));
        ELSE RETURN NEW; END IF;
    ELSIF TG_TABLE_NAME='workflow_jobs' THEN
        IF TG_OP='UPDATE' AND NEW.state IS NOT DISTINCT FROM OLD.state THEN RETURN NEW; END IF;
        submission_id=NEW.pipeline_id::uuid; event_kind='job.state';
        body=jsonb_build_object('job',NEW.id,'phase',NEW.payload::jsonb->'json'->>'phase','state',NEW.state);
    ELSIF TG_TABLE_NAME='workflow_pipelines' THEN
        IF TG_OP='UPDATE' AND NEW.state IS NOT DISTINCT FROM OLD.state THEN RETURN NEW; END IF;
        submission_id=NEW.id::uuid;event_kind='workflow.state';body=jsonb_build_object('state',NEW.state);
    ELSE
        IF TG_OP='UPDATE' AND NEW.status IS NOT DISTINCT FROM OLD.status AND NEW.attempt=OLD.attempt THEN RETURN NEW; END IF;
        SELECT pipeline_id::uuid INTO submission_id FROM workflow_jobs WHERE id=NEW.id;
        event_kind=CASE NEW.status WHEN 'ready' THEN 'job.queued' WHEN 'leased' THEN 'job.started'
            WHEN 'done' THEN 'job.finished' ELSE 'job.deadlettered' END;
        body=jsonb_build_object('job',NEW.id,'phase',NEW.payload->>'phase','attempt',NEW.attempt);
    END IF;
    SELECT scope INTO event_scope FROM judge_probe_submissions WHERE id=submission_id;
    IF event_scope IS NULL THEN RETURN NEW; END IF;
    INSERT INTO judge_probe_event_outbox(scope,submission,kind,payload)
        VALUES(event_scope,submission_id,event_kind,body);
    RETURN NEW;
END $$;
CREATE TRIGGER probe_receipt_events AFTER INSERT OR UPDATE ON judge_probe_submissions FOR EACH ROW EXECUTE FUNCTION judge_probe_capture_event();
CREATE TRIGGER probe_job_events AFTER INSERT OR UPDATE ON workflow_jobs FOR EACH ROW EXECUTE FUNCTION judge_probe_capture_event();
CREATE TRIGGER probe_pipeline_events AFTER INSERT OR UPDATE ON workflow_pipelines FOR EACH ROW EXECUTE FUNCTION judge_probe_capture_event();
CREATE TRIGGER probe_queue_events AFTER INSERT OR UPDATE ON judge_probe_queue FOR EACH ROW EXECUTE FUNCTION judge_probe_capture_event();

CREATE FUNCTION judge_probe_dispatch_events(batch_size INTEGER DEFAULT 256) RETURNS void LANGUAGE plpgsql AS $$
DECLARE stream RECORD; event RECORD; position BIGINT;
BEGIN
    FOR stream IN SELECT DISTINCT scope FROM judge_probe_event_outbox WHERE NOT delivered ORDER BY scope LOOP
        INSERT INTO judge_probe_event_streams(scope) VALUES(stream.scope) ON CONFLICT DO NOTHING;
        SELECT cursor INTO position FROM judge_probe_event_streams WHERE scope=stream.scope FOR UPDATE;
        FOR event IN SELECT * FROM judge_probe_event_outbox WHERE scope=stream.scope AND NOT delivered
            ORDER BY id LIMIT batch_size FOR UPDATE SKIP LOCKED LOOP
            position=position+1;
            INSERT INTO judge_probe_events VALUES(event.scope,position,event.submission,event.kind,event.payload);
            UPDATE judge_probe_event_outbox SET delivered=TRUE WHERE id=event.id;
        END LOOP;
        UPDATE judge_probe_event_streams SET cursor=position WHERE scope=stream.scope;
    END LOOP;
END $$;
