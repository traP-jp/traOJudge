"""Read-only evidence collector inside the trusted control container."""
import json
import os
import ssl
import urllib.parse
import pg8000.native

url=urllib.parse.urlsplit(os.environ['DATABASE_URL'])
tls=ssl.create_default_context(cafile='/opt/rds-ca.pem')
connection=pg8000.native.Connection(user=urllib.parse.unquote(url.username),password=urllib.parse.unquote(url.password),
    host=url.hostname,port=url.port,database=url.path.strip('/'),ssl_context=tls,timeout=10)

def query(sql):
    rows=connection.run(sql)
    names=[column['name'] for column in connection.columns]
    return [dict(zip(names,row)) for row in rows]

state={
    'submissions':query("""SELECT id::text,scope,ordinal,terminal,published,error,
        extract(epoch FROM(finished_at-created_at))::float8 AS seconds,
        extract(epoch FROM(max(finished_at) OVER(PARTITION BY scope ORDER BY ordinal)-created_at))::float8 AS ordered_seconds,
        extract(epoch FROM((to_jsonb(judge_probe_submissions)->>'published_at')::timestamptz-created_at))::float8 AS publication_seconds
        FROM judge_probe_submissions ORDER BY scope,ordinal"""),
    'nodes':query("""SELECT q.id,j.pipeline_id,q.status,q.attempt,q.worker,j.state AS engine_state,
        q.payload->>'phase' AS phase,q.payload->>'language' AS language,
        q.result->>'verdict' AS verdict,(q.result->>'host_cpu_usec')::bigint AS host_cpu_usec,
        (q.result->>'wall_seconds')::float8 AS wall_seconds,q.result->>'runtime' AS runtime,
        CASE WHEN q.payload->>'phase'='compile' THEN md5(q.result->>'artifact') END AS artifact_md5
        FROM judge_probe_queue q JOIN workflow_jobs j ON j.id=q.id ORDER BY j.pipeline_id,q.id"""),
    'deadletters':query('SELECT * FROM judge_probe_deadletters'),
    'postgres':connection.run("SELECT current_setting('server_version')")[0][0],
}
print(json.dumps(state,default=str))
connection.close()
