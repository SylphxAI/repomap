# Database map

```bash
repomap db                          # schema from the repository
repomap db --url-env DATABASE_URL   # live database, read-only
repomap db users                    # one table in detail
repomap db --serve                  # graph UI (or --out db.html)
repomap db --json                   # machine-readable
```

![Database map of crates.io: tables grouped by foreign keys, the crates table's dependents highlighted](/img/db-crates-io.webp)

## Sources

| Source | What is read |
|---|---|
| SQL migrations | every `*.sql` file in path order (timestamps sort): `CREATE TABLE`, `ALTER TABLE … ADD/DROP/RENAME/ALTER COLUMN`, `ADD CONSTRAINT … FOREIGN KEY`, `CREATE [UNIQUE] INDEX`, `DROP TABLE`. `down.sql`, rollback, seed and fixture files are skipped. |
| Prisma | `*.prisma`: models, `@@map`/`@map`, `@id`, `@unique`, `@relation(fields, references)`, `@@index`/`@@unique`/`@@id` |
| Drizzle | `pgTable` / `mysqlTable` / `sqliteTable`: columns, `.primaryKey()`, `.notNull()`, `.unique()`, `.references(() => t.col)` |
| SQLAlchemy | declarative and Flask-SQLAlchemy models: `__tablename__` (or the snake-cased class name), `Column(...)` / `mapped_column(...)`, `ForeignKey("t.c")` or `ForeignKey(Model.col)`. Alembic revisions are skipped. |
| Diesel | `table!` (including doc comments and schemas) and `joinable!` |
| Postgres | `information_schema` + `pg_catalog`: tables, views, columns, primary/foreign/unique constraints, indexes |
| MySQL / MariaDB | `information_schema`: tables, columns, keys, foreign keys, indexes |
| SQLite | `sqlite_master` + `pragma_table_info` / `pragma_foreign_key_list` / `pragma_index_list` |

When a live database and schema files are both present, repomap uses the live structure and borrows the ORM names from the repository so that code links still resolve.

## Read-only, always

- **Postgres:** the session is set to `READ ONLY` and the work runs in `BEGIN READ ONLY`. repomap checks `show transaction_read_only` and stops unless the server answers `on`. There is a 20 s statement timeout, and the transaction is rolled back at the end.
- **MySQL:** runs `SET SESSION TRANSACTION READ ONLY` and then `START TRANSACTION READ ONLY`, rolled back at the end.
- **SQLite:** the file is opened read-only (never created) with `PRAGMA query_only = 1`.
- Only catalog metadata is read, never rows.
- The connection string comes from `--url` or the variable named by `--url-env` (the MCP tool takes `url_env`). It is used for one connection and never written to the cache, the output or logs; error messages have the URL and password redacted.

Use a read-only database role anyway: it is the strongest guarantee.

## Code links

Each table lists where the code touches it, as `file:line` plus the enclosing function:

- raw SQL: `FROM|JOIN|INTO|UPDATE|TABLE users`
- Prisma client calls: `prisma.user.findMany(…)`
- Diesel: `users::table`, `users::dsl`
- ORM identifiers (Drizzle table variables, SQLAlchemy model classes) in files that import the schema module

Import lines are skipped. Links are text-based and good at "where is this table used", but they can miss dynamic table names.
