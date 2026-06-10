# traOJudge MariaDB

Build the local MariaDB image:

```sh
docker build \
  -t traojudge-mariadb:local \
  -f deploy/database/Dockerfile \
  deploy/database
```

Run MariaDB for local repository development:

```sh
docker run --rm \
  --name traojudge-mariadb \
  -p 127.0.0.1:3307:3306 \
  -e MARIADB_DATABASE=traojudge \
  -e MARIADB_USER=traojudge \
  -e MARIADB_PASSWORD=traojudge \
  -e MARIADB_ROOT_PASSWORD=traojudge_root \
  -v traojudge-mariadb-data:/var/lib/mysql \
  traojudge-mariadb:local
```

Use this URL from the backend app:

```sh
TRAOJUDGE_DATABASE_URL=mysql://traojudge:traojudge@127.0.0.1:3307/traojudge
```
