# traOJudge PostgreSQL

Build the local PostgreSQL image:

```sh
docker build \
  -t traojudge-postgres:local \
  -f deploy/database/Dockerfile \
  deploy/database
```

Run PostgreSQL for local repository development:

```sh
docker run --rm \
  --name traojudge-postgres \
  -p 127.0.0.1:5433:5432 \
  -e POSTGRES_DB=traojudge \
  -e POSTGRES_USER=traojudge \
  -e POSTGRES_PASSWORD=traojudge \
  -v traojudge-postgres-data:/var/lib/postgresql/data \
  traojudge-postgres:local
```

The credentials above are for local development only. Use this URL from the backend app:

```sh
TRAOJUDGE_DATABASE_URL=postgresql://traojudge:traojudge@127.0.0.1:5433/traojudge
```

The backend requires `TRAOJUDGE_DATABASE_URL` and `TRAOJUDGE_WEB_ORIGIN`, and applies embedded SQLx migrations
before accepting HTTP requests. See [public user profile development](../../docs/backend/users.md)
for startup, sample data, and integration test commands.
