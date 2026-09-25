//! Read-only introspection of a live Postgres, MySQL or SQLite database.
//!
//! Only catalog / information_schema metadata is read, inside a read-only
//! transaction (Postgres, MySQL) or a read-only, query-only connection
//! (SQLite). The connection string comes from an argument or an environment
//! variable, is used once and is never written anywhere or echoed back.

use anyhow::{anyhow, bail, Result};
use repomap_core::db::{split_name, Column, DbSchema, ForeignKey, IndexDef, Table};
use sqlx::{Connection, Row};
use std::collections::BTreeMap;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

pub fn introspect(url: &str) -> Result<DbSchema> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let fut = async {
        let lower = url.to_ascii_lowercase();
        if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
            postgres(url).await
        } else if lower.starts_with("mysql://") || lower.starts_with("mariadb://") {
            mysql(&url.replacen("mariadb://", "mysql://", 1)).await
        } else if lower.starts_with("sqlite:") || lower.ends_with(".db") || lower.ends_with(".sqlite") || lower.ends_with(".sqlite3") {
            sqlite(url).await
        } else {
            bail!("unsupported database URL scheme (use postgres://, mysql:// or sqlite:)")
        }
    };
    let res = rt.block_on(async { tokio::time::timeout(TIMEOUT, fut).await });
    match res {
        Ok(r) => r.map_err(|e| anyhow!(redact(&e.to_string(), url))),
        Err(_) => bail!("database introspection timed out after {}s", TIMEOUT.as_secs()),
    }
}

/// Never let the connection string (or its password) reach output.
fn redact(msg: &str, url: &str) -> String {
    let mut m = msg.replace(url, "<database-url>");
    if let Some(at) = url.find('@') {
        if let Some(colon) = url[..at].rfind(':') {
            let pw = &url[colon + 1..at];
            if pw.len() >= 3 && !pw.contains('/') {
                m = m.replace(pw, "***");
            }
        }
    }
    m
}

fn table_entry<'a>(map: &'a mut BTreeMap<String, Table>, schema: &str, name: &str, origin: &str, default_schema: &str) -> &'a mut Table {
    let sch = if schema.is_empty() || schema == default_schema { None } else { Some(schema.to_string()) };
    let key = match &sch {
        Some(s) => format!("{s}.{name}"),
        None => name.to_string(),
    };
    map.entry(key).or_insert_with(|| Table { schema: sch, name: name.to_string(), kind: "table".into(), source: origin.to_string(), ..Default::default() })
}

fn index_cols(def: &str) -> Vec<String> {
    let Some(open) = def.rfind('(') else { return Vec::new() };
    def[open + 1..].trim_end_matches(')').split(',').map(|c| c.trim().trim_matches('"').split_whitespace().next().unwrap_or("").to_string()).filter(|c| !c.is_empty()).collect()
}

async fn postgres(url: &str) -> Result<DbSchema> {
    let mut conn = sqlx::PgConnection::connect(url).await?;
    sqlx::raw_sql("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY; SET statement_timeout = '20s'").execute(&mut conn).await?;
    sqlx::raw_sql("BEGIN READ ONLY").execute(&mut conn).await?;
    // Refuse to go on unless the server confirms the transaction is read-only.
    let ro: String = sqlx::query("show transaction_read_only").fetch_one(&mut conn).await?.try_get(0)?;
    if ro != "on" {
        bail!("could not open a read-only transaction; refusing to continue");
    }
    let mut tables: BTreeMap<String, Table> = BTreeMap::new();
    for r in sqlx::query("select table_schema::text, table_name::text, table_type::text from information_schema.tables where table_schema not in ('pg_catalog','information_schema') and table_schema not like 'pg_toast%' order by 1, 2").fetch_all(&mut conn).await? {
        let t = table_entry(&mut tables, &r.try_get::<String, _>(0)?, &r.try_get::<String, _>(1)?, "postgres", "public");
        if r.try_get::<String, _>(2)?.contains("VIEW") {
            t.kind = "view".into();
        }
    }
    for r in sqlx::query("select table_schema::text, table_name::text, column_name::text, case when data_type = 'USER-DEFINED' then udt_name::text else data_type::text end, is_nullable::text from information_schema.columns where table_schema not in ('pg_catalog','information_schema') and table_schema not like 'pg_toast%' order by table_schema, table_name, ordinal_position").fetch_all(&mut conn).await? {
        let t = table_entry(&mut tables, &r.try_get::<String, _>(0)?, &r.try_get::<String, _>(1)?, "postgres", "public");
        t.columns.push(Column { name: r.try_get(2)?, data_type: r.try_get(3)?, nullable: r.try_get::<String, _>(4)? == "YES", primary_key: false });
    }
    let cons = "select n.nspname::text, c.relname::text, con.contype::text, con.conname::text, array(select a.attname::text from unnest(con.conkey) with ordinality k(attnum, ord) join pg_attribute a on a.attrelid = con.conrelid and a.attnum = k.attnum order by k.ord), coalesce(fn.nspname::text, ''), coalesce(fc.relname::text, ''), array(select a.attname::text from unnest(coalesce(con.confkey, '{}'::smallint[])) with ordinality k(attnum, ord) join pg_attribute a on a.attrelid = con.confrelid and a.attnum = k.attnum order by k.ord) from pg_constraint con join pg_class c on c.oid = con.conrelid join pg_namespace n on n.oid = c.relnamespace left join pg_class fc on fc.oid = con.confrelid left join pg_namespace fn on fn.oid = fc.relnamespace where con.contype in ('p','f','u') and n.nspname not in ('pg_catalog','information_schema')";
    for r in sqlx::query(cons).fetch_all(&mut conn).await? {
        let (schema, table, kind, name): (String, String, String, String) = (r.try_get(0)?, r.try_get(1)?, r.try_get(2)?, r.try_get(3)?);
        let cols: Vec<String> = r.try_get(4)?;
        let (fs, ft): (String, String) = (r.try_get(5)?, r.try_get(6)?);
        let fcols: Vec<String> = r.try_get(7)?;
        let t = table_entry(&mut tables, &schema, &table, "postgres", "public");
        match kind.as_str() {
            "p" => {
                for c in t.columns.iter_mut().filter(|c| cols.contains(&c.name)) {
                    c.primary_key = true;
                }
            }
            "f" => t.foreign_keys.push(ForeignKey { columns: cols, ref_table: if fs == "public" { ft } else { format!("{fs}.{ft}") }, ref_columns: fcols }),
            _ => t.indexes.push(IndexDef { name, columns: cols, unique: true }),
        }
    }
    for r in sqlx::query("select schemaname::text, tablename::text, indexname::text, indexdef::text from pg_indexes where schemaname not in ('pg_catalog','information_schema')").fetch_all(&mut conn).await? {
        let (schema, table, name, def): (String, String, String, String) = (r.try_get(0)?, r.try_get(1)?, r.try_get(2)?, r.try_get(3)?);
        let t = table_entry(&mut tables, &schema, &table, "postgres", "public");
        if !t.indexes.iter().any(|i| i.name == name) {
            t.indexes.push(IndexDef { name, columns: index_cols(&def), unique: def.to_ascii_uppercase().starts_with("CREATE UNIQUE") });
        }
    }
    sqlx::raw_sql("ROLLBACK").execute(&mut conn).await?;
    conn.close().await?;
    Ok(DbSchema { origin: "postgres".into(), sources: vec!["live postgres (read-only)".into()], tables: tables.into_values().collect() })
}

async fn mysql(url: &str) -> Result<DbSchema> {
    let mut conn = sqlx::MySqlConnection::connect(url).await?;
    sqlx::raw_sql("SET SESSION TRANSACTION READ ONLY").execute(&mut conn).await?;
    sqlx::raw_sql("START TRANSACTION READ ONLY").execute(&mut conn).await?;
    let mut tables: BTreeMap<String, Table> = BTreeMap::new();
    for r in sqlx::query("select cast(table_name as char), cast(table_type as char) from information_schema.tables where table_schema = database() order by 1").fetch_all(&mut conn).await? {
        let t = table_entry(&mut tables, "", &r.try_get::<String, _>(0)?, "mysql", "");
        if r.try_get::<String, _>(1)?.contains("VIEW") {
            t.kind = "view".into();
        }
    }
    for r in sqlx::query("select cast(table_name as char), cast(column_name as char), cast(column_type as char), cast(is_nullable as char), cast(column_key as char) from information_schema.columns where table_schema = database() order by table_name, ordinal_position").fetch_all(&mut conn).await? {
        let t = table_entry(&mut tables, "", &r.try_get::<String, _>(0)?, "mysql", "");
        t.columns.push(Column { name: r.try_get(1)?, data_type: r.try_get(2)?, nullable: r.try_get::<String, _>(3)? == "YES", primary_key: r.try_get::<String, _>(4)? == "PRI" });
    }
    let mut fks: BTreeMap<(String, String), ForeignKey> = BTreeMap::new();
    for r in sqlx::query("select cast(table_name as char), cast(constraint_name as char), cast(column_name as char), cast(referenced_table_name as char), cast(referenced_column_name as char) from information_schema.key_column_usage where table_schema = database() and referenced_table_name is not null order by table_name, constraint_name, ordinal_position").fetch_all(&mut conn).await? {
        let (table, name): (String, String) = (r.try_get(0)?, r.try_get(1)?);
        let e = fks.entry((table, name)).or_insert_with(|| ForeignKey { ref_table: r.try_get(3).unwrap_or_default(), ..Default::default() });
        e.columns.push(r.try_get(2)?);
        e.ref_columns.push(r.try_get(4)?);
    }
    for ((table, _), fk) in fks {
        table_entry(&mut tables, "", &table, "mysql", "").foreign_keys.push(fk);
    }
    let mut idx: BTreeMap<(String, String), IndexDef> = BTreeMap::new();
    for r in sqlx::query("select cast(table_name as char), cast(index_name as char), cast(column_name as char), non_unique from information_schema.statistics where table_schema = database() order by table_name, index_name, seq_in_index").fetch_all(&mut conn).await? {
        let (table, name): (String, String) = (r.try_get(0)?, r.try_get(1)?);
        let non_unique: i64 = r.try_get(3).unwrap_or(1);
        let e = idx.entry((table, name.clone())).or_insert_with(|| IndexDef { name, columns: Vec::new(), unique: non_unique == 0 });
        e.columns.push(r.try_get(2).unwrap_or_default());
    }
    for ((table, _), ix) in idx {
        if ix.name != "PRIMARY" {
            table_entry(&mut tables, "", &table, "mysql", "").indexes.push(ix);
        }
    }
    sqlx::raw_sql("ROLLBACK").execute(&mut conn).await?;
    conn.close().await?;
    Ok(DbSchema { origin: "mysql".into(), sources: vec!["live mysql (read-only)".into()], tables: tables.into_values().collect() })
}

async fn sqlite(url: &str) -> Result<DbSchema> {
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;
    let opts = if url.to_ascii_lowercase().starts_with("sqlite:") {
        SqliteConnectOptions::from_str(url)?
    } else {
        SqliteConnectOptions::new().filename(url)
    }
    .read_only(true)
    .create_if_missing(false);
    let mut conn = sqlx::SqliteConnection::connect_with(&opts).await?;
    sqlx::raw_sql("PRAGMA query_only = 1").execute(&mut conn).await?;
    let mut tables: BTreeMap<String, Table> = BTreeMap::new();
    let list: Vec<(String, String)> = sqlx::query("select name, type from sqlite_master where type in ('table','view') and name not like 'sqlite_%' order by name")
        .fetch_all(&mut conn)
        .await?
        .into_iter()
        .map(|r| (r.get::<String, _>(0), r.get::<String, _>(1)))
        .collect();
    for (name, kind) in list {
        let mut t = Table { name: name.clone(), kind: kind.clone(), source: "sqlite".into(), ..Default::default() };
        for r in sqlx::query("select name, type, \"notnull\", pk from pragma_table_info(?)").bind(&name).fetch_all(&mut conn).await? {
            t.columns.push(Column { name: r.try_get(0)?, data_type: r.try_get::<String, _>(1)?.to_ascii_lowercase(), nullable: r.try_get::<i64, _>(2)? == 0, primary_key: r.try_get::<i64, _>(3)? > 0 });
        }
        let mut fks: BTreeMap<i64, ForeignKey> = BTreeMap::new();
        for r in sqlx::query("select id, \"table\", \"from\", \"to\" from pragma_foreign_key_list(?) order by id, seq").bind(&name).fetch_all(&mut conn).await? {
            let e = fks.entry(r.try_get(0)?).or_insert_with(|| ForeignKey { ref_table: r.try_get(1).unwrap_or_default(), ..Default::default() });
            e.columns.push(r.try_get(2)?);
            if let Ok(to) = r.try_get::<String, _>(3) {
                e.ref_columns.push(to);
            }
        }
        t.foreign_keys = fks.into_values().collect();
        let idx: Vec<(String, i64)> = sqlx::query("select name, \"unique\" from pragma_index_list(?)").bind(&name).fetch_all(&mut conn).await?.into_iter().map(|r| (r.get::<String, _>(0), r.get::<i64, _>(1))).collect();
        for (iname, unique) in idx {
            let cols: Vec<String> = sqlx::query("select name from pragma_index_info(?)").bind(&iname).fetch_all(&mut conn).await?.into_iter().filter_map(|r| r.try_get::<String, _>(0).ok()).collect();
            t.indexes.push(IndexDef { name: iname, columns: cols, unique: unique == 1 });
        }
        tables.insert(name, t);
    }
    conn.close().await?;
    let _ = split_name;
    Ok(DbSchema { origin: "sqlite".into(), sources: vec!["live sqlite (read-only)".into()], tables: tables.into_values().collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_password() {
        let url = "postgres://app:s3cretpw@db.example.com:5432/app";
        let msg = format!("error connecting to {url}: password s3cretpw rejected");
        let r = redact(&msg, url);
        assert!(!r.contains("s3cretpw"), "{r}");
    }

    #[test]
    fn sqlite_read_only_introspection() {
        let dir = std::env::temp_dir().join(format!("repomap-db-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("app.db");
        let _ = std::fs::remove_file(&path);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let opts = sqlx::sqlite::SqliteConnectOptions::new().filename(&path).create_if_missing(true);
            let mut c = sqlx::SqliteConnection::connect_with(&opts).await.unwrap();
            sqlx::raw_sql("create table users (id integer primary key, email text not null unique); create table posts (id integer primary key, user_id integer references users(id), title text); create index posts_user on posts(user_id);").execute(&mut c).await.unwrap();
            c.close().await.unwrap();
        });
        let s = introspect(path.to_str().unwrap()).unwrap();
        let posts = s.table("posts").unwrap();
        assert_eq!(posts.foreign_keys[0].ref_table, "users");
        assert!(posts.indexes.iter().any(|i| i.name == "posts_user"));
        assert!(s.table("users").unwrap().columns.iter().any(|c| c.name == "id" && c.primary_key));
        // The connection is query-only: nothing can have written a journal/WAL.
        assert!(!dir.join("app.db-wal").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
