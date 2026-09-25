//! Database map: tables, columns, foreign keys and indexes, from a live
//! database (introspected by the binary) or from schema sources in the repo:
//! SQL migrations, Prisma, Drizzle, SQLAlchemy and Diesel. Tables are linked
//! to the code that queries them.

use crate::index::Index;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ForeignKey {
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexDef {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CodeRef {
    pub file: String,
    pub line: u32,
    pub symbol: Option<String>,
    pub via: &'static str,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Table {
    /// Non-default schema (`public`/`main`/database default are omitted).
    pub schema: Option<String>,
    pub name: String,
    pub kind: String,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<IndexDef>,
    /// Where the definition came from: `file:line`, or the live database kind.
    pub source: String,
    /// ORM identifiers that stand for this table in code (model, variable, module).
    pub aliases: Vec<String>,
    pub used_by: Vec<CodeRef>,
}

impl Table {
    pub fn key(&self) -> String {
        match &self.schema {
            Some(s) => format!("{s}.{}", self.name),
            None => self.name.clone(),
        }
    }
    fn col_mut(&mut self, name: &str) -> Option<&mut Column> {
        self.columns.iter_mut().find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DbSchema {
    /// `postgres`, `mysql`, `sqlite`, or `repo` for static sources.
    pub origin: String,
    pub sources: Vec<String>,
    pub tables: Vec<Table>,
}

impl DbSchema {
    pub fn table(&self, name: &str) -> Option<&Table> {
        let n = name.trim().trim_matches('"');
        self.tables
            .iter()
            .find(|t| t.key().eq_ignore_ascii_case(n))
            .or_else(|| self.tables.iter().find(|t| t.name.eq_ignore_ascii_case(n)))
            .or_else(|| self.tables.iter().find(|t| t.aliases.iter().any(|a| a.eq_ignore_ascii_case(n))))
    }

    fn upsert(&mut self, t: Table) -> usize {
        if let Some(i) = self.tables.iter().position(|x| x.key().eq_ignore_ascii_case(&t.key())) {
            let old = &mut self.tables[i];
            // Prefer the ORM definition as the source: code links resolve through it.
            if !t.source.to_ascii_lowercase().contains(".sql:") && old.source.to_ascii_lowercase().contains(".sql:") {
                old.source = t.source.clone();
            }
            for c in t.columns {
                if old.col_mut(&c.name).is_none() {
                    old.columns.push(c);
                }
            }
            old.foreign_keys.extend(t.foreign_keys);
            old.indexes.extend(t.indexes);
            for a in t.aliases {
                if !old.aliases.contains(&a) {
                    old.aliases.push(a);
                }
            }
            i
        } else {
            self.tables.push(t);
            self.tables.len() - 1
        }
    }

    fn find_mut(&mut self, name: &str) -> Option<&mut Table> {
        let (schema, n) = split_name(name);
        self.tables
            .iter_mut()
            .find(|t| t.name.eq_ignore_ascii_case(&n) && (schema.is_none() || t.schema.as_deref().map(|s| s.eq_ignore_ascii_case(schema.as_deref().unwrap())).unwrap_or(false)))
    }
}

// ---------------------------------------------------------------- identifiers

fn unquote(s: &str) -> String {
    s.trim().trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']' || c == '\'').to_string()
}

/// `"public"."users"` -> (None, users); `auth.users` -> (Some(auth), users).
pub fn split_name(raw: &str) -> (Option<String>, String) {
    let parts: Vec<String> = raw.split('.').map(unquote).filter(|p| !p.is_empty()).collect();
    match parts.len() {
        0 => (None, String::new()),
        1 => (None, parts[0].clone()),
        _ => {
            let schema = parts[parts.len() - 2].clone();
            let name = parts[parts.len() - 1].clone();
            if matches!(schema.to_ascii_lowercase().as_str(), "public" | "main" | "dbo") {
                (None, name)
            } else {
                (Some(schema), name)
            }
        }
    }
}

// ---------------------------------------------------------------- SQL DDL

fn strip_sql_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            out.push(c as char);
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == b'\'' || c == b'"' || c == b'`' {
            quote = Some(c);
            out.push(c as char);
            i += 1;
        } else if c == b'-' && i + 1 < b.len() && b[i + 1] == b'-' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(c as char);
            i += 1;
        }
    }
    out
}

/// Split on `sep` at paren depth 0, outside quotes and `$$` bodies.
fn split_top(s: &str, sep: char) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut dollar = false;
    let mut start = 0;
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut k = 0;
    while k < chars.len() {
        let (i, c) = chars[k];
        if dollar {
            if c == '$' && k + 1 < chars.len() && chars[k + 1].1 == '$' {
                dollar = false;
                k += 2;
                continue;
            }
            k += 1;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            k += 1;
            continue;
        }
        match c {
            '\'' | '"' | '`' => quote = Some(c),
            '$' if k + 1 < chars.len() && chars[k + 1].1 == '$' => {
                dollar = true;
                k += 2;
                continue;
            }
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if c == sep && depth == 0 => {
                out.push((start, s[start..i].to_string()));
                start = i + c.len_utf8();
            }
            _ => {}
        }
        k += 1;
    }
    if start < s.len() {
        out.push((start, s[start..].to_string()));
    }
    out
}

/// Words, quoted identifiers and parenthesised groups as tokens.
fn sql_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || c == ',' {
            i += 1;
        } else if c == '(' {
            let mut depth = 0;
            let start = i;
            while i < chars.len() {
                if chars[i] == '(' {
                    depth += 1;
                } else if chars[i] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            out.push(chars[start..i].iter().collect());
        } else if c == '"' || c == '`' || c == '[' || c == '\'' {
            let close = if c == '[' { ']' } else { c };
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != close {
                i += 1;
            }
            i += 1;
            let mut tok: String = chars[start..i.min(chars.len())].iter().collect();
            // Glue a following `.name` (schema-qualified identifiers).
            while i < chars.len() && chars[i] == '.' {
                let s2 = i;
                i += 1;
                while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' && chars[i] != ',' {
                    i += 1;
                }
                tok.push_str(&chars[s2..i].iter().collect::<String>());
            }
            out.push(tok);
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' && chars[i] != ',' {
                if chars[i] == '"' || chars[i] == '`' {
                    // "schema"."table" style continues through quotes.
                    let q = chars[i];
                    i += 1;
                    while i < chars.len() && chars[i] != q {
                        i += 1;
                    }
                }
                i += 1;
            }
            out.push(chars[start..i.min(chars.len())].iter().collect());
        }
    }
    out
}

fn paren_list(tok: &str) -> Vec<String> {
    let inner = tok.trim().trim_start_matches('(').trim_end_matches(')');
    split_top(inner, ',')
        .into_iter()
        .map(|(_, s)| {
            let s = s.trim();
            // `col ASC`, `lower(col)`, `col(10)` -> the identifier part.
            let first = s.split_whitespace().next().unwrap_or("");
            unquote(first.split('(').next().unwrap_or(first))
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn upper(t: &str) -> String {
    t.to_ascii_uppercase()
}

const COL_STOP: &[&str] = &[
    "NOT", "NULL", "PRIMARY", "UNIQUE", "REFERENCES", "DEFAULT", "CONSTRAINT", "CHECK", "GENERATED", "COLLATE", "AUTO_INCREMENT", "AUTOINCREMENT", "COMMENT", "ON", "IDENTITY", "AS",
];

fn parse_column(def: &str) -> Option<(Column, Option<ForeignKey>, bool)> {
    let toks = sql_tokens(def);
    let name = unquote(toks.first()?);
    if name.is_empty() {
        return None;
    }
    let mut ty = String::new();
    let mut i = 1;
    while i < toks.len() && !COL_STOP.contains(&upper(&toks[i]).as_str()) {
        if !ty.is_empty() && !toks[i].starts_with('(') {
            ty.push(' ');
        }
        ty.push_str(&toks[i]);
        i += 1;
    }
    let mut col = Column { name, data_type: ty.to_ascii_lowercase(), nullable: true, primary_key: false };
    let mut fk = None;
    let mut unique = false;
    while i < toks.len() {
        match upper(&toks[i]).as_str() {
            "NOT" if i + 1 < toks.len() && upper(&toks[i + 1]) == "NULL" => {
                col.nullable = false;
                i += 1;
            }
            "PRIMARY" => {
                col.primary_key = true;
                col.nullable = false;
            }
            "UNIQUE" => unique = true,
            "REFERENCES" if i + 1 < toks.len() => {
                let (s, t) = split_name(&toks[i + 1]);
                let ref_table = match s {
                    Some(s) => format!("{s}.{t}"),
                    None => t,
                };
                let ref_columns = if i + 2 < toks.len() && toks[i + 2].starts_with('(') { paren_list(&toks[i + 2]) } else { Vec::new() };
                fk = Some(ForeignKey { columns: vec![col.name.clone()], ref_table, ref_columns });
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    Some((col, fk, unique))
}

/// Table-level constraint or column definition inside CREATE TABLE / ALTER TABLE ADD.
fn apply_table_item(t: &mut Table, item: &str) {
    let toks = sql_tokens(item);
    if toks.is_empty() {
        return;
    }
    let mut k = 0;
    if upper(&toks[0]) == "CONSTRAINT" {
        k = 2;
    }
    if k >= toks.len() {
        return;
    }
    let head = upper(&toks[k]);
    match head.as_str() {
        "PRIMARY" => {
            if let Some(list) = toks.iter().skip(k).find(|x| x.starts_with('(')) {
                for c in paren_list(list) {
                    if let Some(col) = t.col_mut(&c) {
                        col.primary_key = true;
                        col.nullable = false;
                    }
                }
            }
        }
        "FOREIGN" => {
            let lists: Vec<&String> = toks.iter().skip(k).filter(|x| x.starts_with('(')).collect();
            if let Some(r) = toks.iter().position(|x| upper(x) == "REFERENCES") {
                if r + 1 < toks.len() && !lists.is_empty() {
                    let (s, n) = split_name(&toks[r + 1]);
                    t.foreign_keys.push(ForeignKey {
                        columns: paren_list(lists[0]),
                        ref_table: match s {
                            Some(s) => format!("{s}.{n}"),
                            None => n,
                        },
                        ref_columns: if r + 2 < toks.len() && toks[r + 2].starts_with('(') { paren_list(&toks[r + 2]) } else { Vec::new() },
                    });
                }
            }
        }
        "UNIQUE" | "KEY" | "INDEX" => {
            if let Some(list) = toks.iter().skip(k).find(|x| x.starts_with('(')) {
                let name = if k + 1 < toks.len() && !toks[k + 1].starts_with('(') && upper(&toks[k + 1]) != "KEY" { unquote(&toks[k + 1]) } else { String::new() };
                t.indexes.push(IndexDef { name, columns: paren_list(list), unique: head == "UNIQUE" });
            }
        }
        "CHECK" | "EXCLUDE" | "FULLTEXT" | "SPATIAL" => {}
        _ => {
            if let Some((col, fk, unique)) = parse_column(item) {
                if unique {
                    t.indexes.push(IndexDef { name: String::new(), columns: vec![col.name.clone()], unique: true });
                }
                if let Some(fk) = fk {
                    t.foreign_keys.push(fk);
                }
                if let Some(existing) = t.col_mut(&col.name) {
                    *existing = col;
                } else {
                    t.columns.push(col);
                }
            }
        }
    }
}

fn line_at(src: &str, byte: usize) -> u32 {
    src[..byte.min(src.len())].bytes().filter(|b| *b == b'\n').count() as u32 + 1
}

pub fn parse_sql(schema: &mut DbSchema, path: &str, src: &str) {
    let cleaned = strip_sql_comments(src);
    let re_create = Regex::new(r"(?is)^\s*create\s+(?:or\s+replace\s+)?(?:(?:global\s+|local\s+)?(?:temp|temporary)\s+|unlogged\s+|virtual\s+)?(table|view|materialized\s+view)\s+(?:if\s+not\s+exists\s+)?([^\s(]+)").unwrap();
    let re_index = Regex::new(r"(?is)^\s*create\s+(unique\s+)?index\s+(?:concurrently\s+)?(?:if\s+not\s+exists\s+)?([^\s(]*)\s*on\s+(?:only\s+)?([^\s(]+)(?:\s+using\s+\w+)?\s*(\(.*\))").unwrap();
    let re_alter = Regex::new(r"(?is)^\s*alter\s+table\s+(?:if\s+exists\s+)?(?:only\s+)?([^\s]+)\s+(.*)$").unwrap();
    let re_drop = Regex::new(r"(?is)^\s*drop\s+(?:table|view)\s+(?:if\s+exists\s+)?([^;]+?)(?:\s+cascade|\s+restrict)?\s*$").unwrap();
    for (off, stmt) in split_top(&cleaned, ';') {
        let line = line_at(&cleaned, off + stmt.len() - stmt.trim_start().len());
        let st = stmt.trim();
        if st.is_empty() {
            continue;
        }
        if let Some(c) = re_create.captures(st) {
            let kind = if c[1].to_ascii_lowercase().contains("view") { "view" } else { "table" };
            let (s, n) = split_name(&c[2]);
            let mut t = Table { schema: s, name: n, kind: kind.into(), source: format!("{path}:{line}"), ..Default::default() };
            if kind == "table" {
                let rest = &st[c.get(0).unwrap().end()..];
                if let Some(open) = rest.find('(') {
                    // Body = the balanced group after the name.
                    let body_tok = sql_tokens(&rest[open..]).into_iter().next().unwrap_or_default();
                    let body = body_tok.trim_start_matches('(').trim_end_matches(')');
                    for (_, item) in split_top(body, ',') {
                        apply_table_item(&mut t, item.trim());
                    }
                }
            }
            if let Some(existing) = schema.find_mut(&c[2]) {
                // CREATE TABLE after a DROP in a later migration: replace.
                *existing = t;
            } else {
                schema.tables.push(t);
            }
        } else if let Some(c) = re_index.captures(st) {
            let unique = c.get(1).is_some();
            let name = unquote(&c[2]);
            let cols = paren_list(sql_tokens(&c[4]).first().map(|s| s.as_str()).unwrap_or(""));
            if let Some(t) = schema.find_mut(&c[3]) {
                t.indexes.push(IndexDef { name, columns: cols, unique });
            }
        } else if let Some(c) = re_alter.captures(st) {
            let target = c[1].to_string();
            let actions = c[2].to_string();
            for (_, act) in split_top(&actions, ',') {
                let a = act.trim();
                let toks = sql_tokens(a);
                if toks.is_empty() {
                    continue;
                }
                let verb = upper(&toks[0]);
                let Some(t) = schema.find_mut(&target) else { break };
                match verb.as_str() {
                    "ADD" => {
                        let mut rest = a[3..].trim_start();
                        let lower = rest.to_ascii_lowercase();
                        if lower.starts_with("column ") {
                            rest = rest[7..].trim_start();
                        }
                        let lower = rest.to_ascii_lowercase();
                        if lower.starts_with("if not exists ") {
                            rest = rest[14..].trim_start();
                        }
                        apply_table_item(t, rest);
                    }
                    "DROP" => {
                        let mut k = 1;
                        if k < toks.len() && upper(&toks[k]) == "COLUMN" {
                            k += 1;
                        }
                        if k + 1 < toks.len() && upper(&toks[k]) == "IF" {
                            k += 2;
                        }
                        if k < toks.len() && upper(&toks[k]) != "CONSTRAINT" {
                            let col = unquote(&toks[k]);
                            t.columns.retain(|c| !c.name.eq_ignore_ascii_case(&col));
                            t.foreign_keys.retain(|f| !f.columns.iter().any(|c| c.eq_ignore_ascii_case(&col)));
                        }
                    }
                    "RENAME" => {
                        if toks.len() >= 3 && upper(&toks[1]) == "TO" {
                            let (_, n) = split_name(&toks[2]);
                            t.name = n;
                        } else {
                            let k = if upper(&toks.get(1).map(|s| s.as_str()).unwrap_or("")) == "COLUMN" { 2 } else { 1 };
                            if toks.len() > k + 2 && upper(&toks[k + 1]) == "TO" {
                                let (from, to) = (unquote(&toks[k]), unquote(&toks[k + 2]));
                                if let Some(col) = t.col_mut(&from) {
                                    col.name = to;
                                }
                            }
                        }
                    }
                    "ALTER" | "MODIFY" => {
                        let k = if toks.len() > 1 && upper(&toks[1]) == "COLUMN" { 2 } else { 1 };
                        if let Some(colname) = toks.get(k).map(|s| unquote(s)) {
                            let rest: Vec<String> = toks.iter().skip(k + 1).map(|s| upper(s)).collect();
                            if let Some(col) = t.col_mut(&colname) {
                                if rest.windows(2).any(|w| w[0] == "SET" && w[1] == "NOT") {
                                    col.nullable = false;
                                } else if rest.windows(2).any(|w| w[0] == "DROP" && w[1] == "NOT") {
                                    col.nullable = true;
                                } else if let Some(p) = rest.iter().position(|x| x == "TYPE") {
                                    if let Some(ty) = toks.get(k + 1 + p + 1) {
                                        col.data_type = ty.to_ascii_lowercase();
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else if let Some(c) = re_drop.captures(st) {
            for (_, name) in split_top(&c[1], ',') {
                let (s, n) = split_name(name.trim());
                schema.tables.retain(|t| !(t.name.eq_ignore_ascii_case(&n) && (s.is_none() || t.schema == s)));
            }
        }
    }
}

// ---------------------------------------------------------------- Prisma

fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_lowercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

fn bracket_list(s: &str) -> Vec<String> {
    s.trim().trim_start_matches('[').trim_end_matches(']').split(',').map(|x| x.split('(').next().unwrap_or(x).trim().trim_matches('"').to_string()).filter(|x| !x.is_empty()).collect()
}

pub fn parse_prisma(schema: &mut DbSchema, path: &str, src: &str) {
    let re_block = Regex::new(r"(?m)^\s*(model|view)\s+(\w+)\s*\{").unwrap();
    let re_map = Regex::new(r#"@@map\(\s*(?:name:\s*)?"([^"]+)"\s*\)"#).unwrap();
    let re_fmap = Regex::new(r#"@map\(\s*(?:name:\s*)?"([^"]+)"\s*\)"#).unwrap();
    let re_rel = Regex::new(r"@relation\(([^)]*)\)").unwrap();
    let re_fields = Regex::new(r"fields:\s*(\[[^\]]*\])").unwrap();
    let re_refs = Regex::new(r"references:\s*(\[[^\]]*\])").unwrap();
    let re_attr_list = Regex::new(r"@@(index|unique|id)\(\s*(?:fields:\s*)?(\[[^\]]*\])").unwrap();
    let models: HashSet<String> = re_block.captures_iter(src).map(|c| c[2].to_string()).collect();
    // model name -> table name (after @@map), resolved in a first pass.
    let mut table_of: HashMap<String, String> = HashMap::new();
    let mut blocks = Vec::new();
    for c in re_block.captures_iter(src) {
        let start = c.get(0).unwrap().end();
        let end = src[start..].find("\n}").map(|e| start + e).unwrap_or(src.len());
        let body = &src[start..end];
        let table = re_map.captures(body).map(|m| m[1].to_string()).unwrap_or_else(|| c[2].to_string());
        table_of.insert(c[2].to_string(), table.clone());
        blocks.push((c[1].to_string(), c[2].to_string(), table, body.to_string(), line_at(src, c.get(0).unwrap().start())));
    }
    for (kind, model, table, body, line) in blocks {
        let mut t = Table { name: table, kind: if kind == "view" { "view".into() } else { "table".into() }, source: format!("{path}:{line}"), aliases: vec![model.clone(), lower_first(&model)], ..Default::default() };
        let mut field_col: HashMap<String, String> = HashMap::new();
        for raw in body.lines() {
            let l = raw.trim();
            if l.is_empty() || l.starts_with("//") {
                continue;
            }
            if l.starts_with("@@") {
                if let Some(c) = re_attr_list.captures(l) {
                    let cols: Vec<String> = bracket_list(&c[2]).into_iter().map(|f| field_col.get(&f).cloned().unwrap_or(f)).collect();
                    match &c[1] {
                        "id" => {
                            for f in &cols {
                                if let Some(col) = t.col_mut(f) {
                                    col.primary_key = true;
                                }
                            }
                        }
                        k => t.indexes.push(IndexDef { name: String::new(), columns: cols, unique: k == "unique" }),
                    }
                }
                continue;
            }
            let mut parts = l.split_whitespace();
            let (Some(field), Some(ty)) = (parts.next(), parts.next()) else { continue };
            let base = ty.trim_end_matches('?').trim_end_matches("[]");
            if let Some(rel) = re_rel.captures(l) {
                if let (Some(f), Some(r)) = (re_fields.captures(&rel[1]), re_refs.captures(&rel[1])) {
                    let cols: Vec<String> = bracket_list(&f[1]).into_iter().map(|x| field_col.get(&x).cloned().unwrap_or(x)).collect();
                    t.foreign_keys.push(ForeignKey { columns: cols, ref_table: table_of.get(base).cloned().unwrap_or_else(|| base.to_string()), ref_columns: bracket_list(&r[1]) });
                }
                continue;
            }
            if models.contains(base) || ty.ends_with("[]") && models.contains(base) {
                continue; // relation field without a column
            }
            let colname = re_fmap.captures(l).map(|m| m[1].to_string()).unwrap_or_else(|| field.to_string());
            field_col.insert(field.to_string(), colname.clone());
            t.columns.push(Column { name: colname.clone(), data_type: base.to_ascii_lowercase(), nullable: ty.ends_with('?'), primary_key: l.contains("@id") });
            if l.contains("@unique") {
                t.indexes.push(IndexDef { name: String::new(), columns: vec![colname], unique: true });
            }
        }
        schema.upsert(t);
    }
}

// ---------------------------------------------------------------- Drizzle

fn balanced(src: &str, open_at: usize) -> Option<(usize, &str)> {
    let b = src.as_bytes();
    let (open, close) = match b.get(open_at)? {
        b'{' => (b'{', b'}'),
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        _ => return None,
    };
    let mut depth = 0;
    let mut quote: Option<u8> = None;
    for i in open_at..b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == q && b[i - 1] != b'\\' {
                quote = None;
            }
            continue;
        }
        match c {
            b'"' | b'\'' | b'`' => quote = Some(c),
            _ if c == open => depth += 1,
            _ if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some((i, &src[open_at + 1..i]));
                }
            }
            _ => {}
        }
    }
    None
}

pub fn parse_drizzle(schema: &mut DbSchema, path: &str, src: &str) {
    let re_table = Regex::new(r#"(?:export\s+)?const\s+(\w+)\s*=\s*(?:\w+\.)?(pgTable|mysqlTable|sqliteTable|table)\s*\(\s*["'`]([^"'`]+)["'`]\s*,\s*"#).unwrap();
    let re_type = Regex::new(r#"^\s*(\w+)\s*\(\s*(?:["'`]([^"'`]+)["'`])?"#).unwrap();
    let re_ref = Regex::new(r"\.references\(\s*\(\)\s*(?::\s*\w+\s*)?=>\s*(\w+)\.(\w+)").unwrap();
    let mut var_table: HashMap<String, String> = HashMap::new();
    for c in re_table.captures_iter(src) {
        var_table.insert(c[1].to_string(), c[3].to_string());
    }
    for c in re_table.captures_iter(src) {
        let m = c.get(0).unwrap();
        let mut at = m.end();
        // `pgTable("x", (t) => ({ ... }))` form: skip to the object literal.
        while at < src.len() && !matches!(src.as_bytes()[at], b'{') {
            if src.as_bytes()[at] == b')' {
                break;
            }
            at += 1;
        }
        let Some((_, body)) = balanced(src, at) else { continue };
        let mut t = Table { name: c[3].to_string(), kind: "table".into(), source: format!("{path}:{}", line_at(src, m.start())), aliases: vec![c[1].to_string()], ..Default::default() };
        for (_, prop) in split_top(body, ',') {
            let prop = prop.trim();
            let Some((key, val)) = prop.split_once(':') else { continue };
            let key = key.trim().trim_matches(|x| x == '"' || x == '\'');
            if key.is_empty() || key.contains(' ') {
                continue;
            }
            let Some(tc) = re_type.captures(val) else { continue };
            let colname = tc.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| key.to_string());
            let col = Column { name: colname.clone(), data_type: tc[1].to_ascii_lowercase(), nullable: !val.contains(".notNull()") && !val.contains(".primaryKey()"), primary_key: val.contains(".primaryKey()") };
            if val.contains(".unique()") {
                t.indexes.push(IndexDef { name: String::new(), columns: vec![colname.clone()], unique: true });
            }
            if let Some(r) = re_ref.captures(val) {
                let ref_table = var_table.get(&r[1]).cloned().unwrap_or_else(|| r[1].to_string());
                t.foreign_keys.push(ForeignKey { columns: vec![colname.clone()], ref_table, ref_columns: vec![to_snake(&r[2])] });
            }
            t.columns.push(col);
        }
        schema.upsert(t);
    }
}

// ---------------------------------------------------------------- SQLAlchemy

pub fn parse_sqlalchemy(schema: &mut DbSchema, path: &str, src: &str) {
    let re_class = Regex::new(r"(?m)^([ \t]*)class\s+(\w+)\s*(?:\([^)]*\))?\s*:").unwrap();
    let re_tn = Regex::new(r#"__tablename__\s*=\s*["']([^"']+)["']"#).unwrap();
    let re_col = Regex::new(r"^(\w+)\s*(?::\s*([^=]+))?=\s*(?:\w+\.)?(Column|mapped_column)\s*\((.*)$").unwrap();
    let re_fk = Regex::new(r#"ForeignKey\(\s*["']([^"'.]+(?:\.[^"'.]+)?)\.([^"'.]+)["']"#).unwrap();
    let re_str = Regex::new(r#"^\s*["']([^"']+)["']"#).unwrap();
    let re_fk_attr = Regex::new(r"ForeignKey\(\s*([A-Za-z_]\w*)\.(\w+)\s*[,)]").unwrap();
    let re_type = Regex::new(r"(?:^|,)\s*(?:sa\.|db\.)?([A-Z]\w*)").unwrap();
    let re_mapped = Regex::new(r"Mapped\[\s*(?:Optional\[)?\s*([\w.]+)").unwrap();
    let lines: Vec<&str> = src.lines().collect();
    for c in re_class.captures_iter(src) {
        let indent = c[1].len();
        let start_line = line_at(src, c.get(0).unwrap().start()) as usize;
        let mut body: Vec<&str> = Vec::new();
        for l in lines.iter().skip(start_line) {
            if !l.trim().is_empty() && l.len() - l.trim_start().len() <= indent {
                break;
            }
            body.push(l);
        }
        let text = body.join("\n");
        // Flask-SQLAlchemy / declarative models without __tablename__ use the snake-cased class name.
        let header = c.get(0).unwrap().as_str();
        let name = match re_tn.captures(&text) {
            Some(tn) => tn[1].to_string(),
            None if (header.contains("Model") || header.contains("Base")) && (text.contains("Column(") || text.contains("mapped_column(")) => to_snake(&c[2]),
            None => continue,
        };
        let mut t = Table { name, kind: "table".into(), source: format!("{path}:{start_line}"), aliases: vec![c[2].to_string()], ..Default::default() };
        // Join continuation lines of multi-line Column(...) calls.
        let mut stmts: Vec<String> = Vec::new();
        let mut depth = 0i32;
        for l in &body {
            let trimmed = l.trim();
            if depth > 0 {
                if let Some(last) = stmts.last_mut() {
                    last.push(' ');
                    last.push_str(trimmed);
                }
            } else {
                stmts.push(trimmed.to_string());
            }
            depth += trimmed.matches('(').count() as i32 - trimmed.matches(')').count() as i32;
            depth = depth.max(0);
        }
        for s in stmts {
            let Some(m) = re_col.captures(&s) else { continue };
            let attr = m[1].to_string();
            let args = &m[4];
            let name = re_str.captures(args).map(|x| x[1].to_string()).unwrap_or(attr);
            let ty = m.get(2).and_then(|a| re_mapped.captures(a.as_str()).map(|x| x[1].to_string())).or_else(|| re_type.captures(args).map(|x| x[1].to_string())).unwrap_or_default();
            let pk = args.contains("primary_key=True");
            let nullable = !(args.contains("nullable=False") || pk) && !m.get(2).map_or(false, |a| !a.as_str().contains("Optional") && !a.as_str().contains("None"));
            if let Some(fk) = re_fk.captures(args) {
                t.foreign_keys.push(ForeignKey { columns: vec![name.clone()], ref_table: fk[1].to_string(), ref_columns: vec![fk[2].to_string()] });
            } else if let Some(fk) = re_fk_attr.captures(args) {
                // ForeignKey(User.id): the class name resolves through table aliases.
                t.foreign_keys.push(ForeignKey { columns: vec![name.clone()], ref_table: fk[1].to_string(), ref_columns: vec![fk[2].to_string()] });
            }
            if args.contains("unique=True") {
                t.indexes.push(IndexDef { name: String::new(), columns: vec![name.clone()], unique: true });
            }
            t.columns.push(Column { name, data_type: ty.to_ascii_lowercase(), nullable, primary_key: pk });
        }
        schema.upsert(t);
    }
}

// ---------------------------------------------------------------- Diesel

pub fn parse_diesel(schema: &mut DbSchema, path: &str, src: &str) {
    let re_table = Regex::new(r"(?s)table!\s*\{\s*(?:(?:use\s[^;]*;|//[^\n]*|#\[[^\]]*\])\s*)*(?:(\w+)\.)?(\w+)\s*(?:\(([^)]*)\))?\s*\{([^}]*)\}").unwrap();
    let re_col = Regex::new(r"(?m)^\s*(?:#\[[^\]]*\]\s*)*(\w+)\s*->\s*([^,\n]+),?").unwrap();
    let re_join = Regex::new(r"joinable!\s*\(\s*(\w+)\s*->\s*(\w+)\s*\(\s*(\w+)\s*\)\s*\)").unwrap();
    for c in re_table.captures_iter(src) {
        let pks: Vec<String> = c.get(3).map(|p| p.as_str().split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_else(|| vec!["id".into()]);
        let mut t = Table {
            schema: c.get(1).map(|s| s.as_str().to_string()).filter(|s| s != "public"),
            name: c[2].to_string(),
            kind: "table".into(),
            source: format!("{path}:{}", line_at(src, c.get(0).unwrap().start())),
            aliases: vec![c[2].to_string()],
            ..Default::default()
        };
        for col in re_col.captures_iter(&c[4]) {
            let ty = col[2].trim().to_string();
            t.columns.push(Column { name: col[1].to_string(), nullable: ty.starts_with("Nullable<"), data_type: ty.to_ascii_lowercase(), primary_key: pks.contains(&col[1].to_string()) });
        }
        schema.upsert(t);
    }
    for j in re_join.captures_iter(src) {
        let child = j[1].to_string();
        let fk = ForeignKey { columns: vec![j[3].to_string()], ref_table: j[2].to_string(), ref_columns: Vec::new() };
        if let Some(t) = schema.find_mut(&child) {
            t.foreign_keys.push(fk);
        }
    }
}

// ---------------------------------------------------------------- from repo

/// Static schema from the repository's schema sources.
pub fn from_repo(index: &Index) -> DbSchema {
    let mut schema = DbSchema { origin: "repo".into(), ..Default::default() };
    let mut sql_files: Vec<&str> = Vec::new();
    for f in &index.files {
        let p = f.path.as_str();
        let lower = p.to_ascii_lowercase();
        let name = lower.rsplit('/').next().unwrap_or(&lower);
        // Down / rollback migrations undo schema; seeds and fixtures hold data.
        let rollback = name == "down.sql" || name.ends_with(".down.sql") || name.contains("rollback") || name.starts_with("down");
        if lower.ends_with(".sql") && !f.is_test && !rollback && !lower.contains("seed") && !lower.contains("fixture") {
            sql_files.push(p);
        }
    }
    // Migrations apply in path order (timestamps/sequence numbers sort).
    sql_files.sort();
    let read = |p: &str| std::fs::read_to_string(index.root.join(p)).ok();
    let mut sources: Vec<String> = Vec::new();
    for p in &sql_files {
        if let Some(src) = read(p) {
            let before = schema.tables.len();
            parse_sql(&mut schema, p, &src);
            if schema.tables.len() != before || src.to_ascii_lowercase().contains("alter table") {
                sources.push(p.to_string());
            }
        }
    }
    // ORM sources override/extend SQL (they carry model names for linking).
    for f in &index.files {
        let p = f.path.as_str();
        let lower = p.to_ascii_lowercase();
        if f.is_test {
            continue;
        }
        let parsed = if lower.ends_with(".prisma") {
            read(p).map(|s| parse_prisma(&mut schema, p, &s)).is_some()
        } else if lower.ends_with(".ts") || lower.ends_with(".js") || lower.ends_with(".mts") {
            match read(p) {
                Some(s) if s.contains("drizzle-orm") && (s.contains("pgTable(") || s.contains("mysqlTable(") || s.contains("sqliteTable(")) => {
                    parse_drizzle(&mut schema, p, &s);
                    true
                }
                _ => false,
            }
        } else if lower.ends_with(".py") {
            match read(p) {
                // Alembic revisions describe changes, not models.
                Some(s) if !lower.contains("/versions/") && !s.contains("from alembic import op") && (s.contains("__tablename__") || (s.contains("sqlalchemy") || s.contains("db.Model")) && (s.contains("Column(") || s.contains("mapped_column("))) => {
                    parse_sqlalchemy(&mut schema, p, &s);
                    true
                }
                _ => false,
            }
        } else if lower.ends_with(".rs") {
            match read(p) {
                Some(s) if s.contains("table!") && s.contains("->") => {
                    parse_diesel(&mut schema, p, &s);
                    true
                }
                _ => false,
            }
        } else {
            false
        };
        if parsed {
            sources.push(p.to_string());
        }
    }
    sources.dedup();
    schema.sources = sources;
    schema.tables.sort_by(|a, b| a.key().cmp(&b.key()));
    schema
}

// ---------------------------------------------------------------- code links

/// Link each table to the code that queries it: raw SQL (`FROM users`), Prisma
/// (`prisma.user.findMany`), Diesel (`users::table`) and ORM models or
/// variables used by files that import their definition.
pub fn link_code(index: &Index, schema: &mut DbSchema) {
    if schema.tables.is_empty() {
        return;
    }
    let esc = |s: &str| regex::escape(s);
    let names: Vec<String> = schema.tables.iter().map(|t| t.name.clone()).collect();
    let alt = names.iter().map(|n| esc(n)).collect::<Vec<_>>().join("|");
    let Ok(re_sql) = Regex::new(&format!(r#"(?i)\b(?:from|join|into|update|table|exists)\s+["`\[]?(?:\w+["`\]]?\.["`\[]?)?({alt})\b"#)) else { return };
    let prisma: Vec<(usize, String)> = schema.tables.iter().enumerate().flat_map(|(i, t)| t.aliases.iter().filter(|a| a.chars().next().map_or(false, |c| c.is_ascii_lowercase())).map(move |a| (i, a.clone()))).collect();
    let re_prisma = if prisma.is_empty() { None } else { Regex::new(&format!(r"\b(?:prisma|db|tx|client)\.({})\s*\.\s*(?:find|create|update|delete|upsert|count|aggregate|groupBy)", prisma.iter().map(|(_, a)| esc(a)).collect::<Vec<_>>().join("|"))).ok() };
    let re_diesel = Regex::new(&format!(r"\b({alt})::(?:table|dsl|columns)\b")).ok();

    // Candidate files: those whose index mentions a table name or alias.
    let mut candidates: HashSet<u32> = HashSet::new();
    for t in &schema.tables {
        for term in std::iter::once(&t.name).chain(t.aliases.iter()) {
            candidates.extend(index.bm25.files_with(&term.to_ascii_lowercase()));
        }
    }
    let defining: HashSet<String> = schema.tables.iter().filter_map(|t| t.source.rsplit_once(':').map(|(f, _)| f.to_string())).collect();
    // ORM identifiers (class / variable names) resolve only in files that import the defining file.
    let mut alias_home: HashMap<String, (usize, u32)> = HashMap::new();
    for (i, t) in schema.tables.iter().enumerate() {
        if let Some((f, _)) = t.source.rsplit_once(':') {
            if let Some(&fid) = index.path_ix.get(f) {
                for a in &t.aliases {
                    if a.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) && a.len() >= 3 {
                        alias_home.insert(a.clone(), (i, fid));
                    }
                }
            }
        }
    }
    let re_alias = if alias_home.is_empty() { None } else { Regex::new(&format!(r"\b({})\b", alias_home.keys().map(|a| esc(a)).collect::<Vec<_>>().join("|"))).ok() };

    let by_name: HashMap<String, usize> = names.iter().enumerate().map(|(i, n)| (n.to_ascii_lowercase(), i)).collect();
    let mut refs: Vec<Vec<CodeRef>> = vec![Vec::new(); schema.tables.len()];
    let mut ids: Vec<u32> = candidates.into_iter().collect();
    ids.sort();
    for fid in ids {
        let f = &index.files[fid as usize];
        if f.lang.is_none() || defining.contains(&f.path) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(index.root.join(&f.path)) else { continue };
        let imports: HashSet<u32> = index.file_out[fid as usize].iter().map(|&e| index.file_edges[e as usize].to).collect();
        let line_starts: Vec<usize> = std::iter::once(0).chain(src.match_indices('\n').map(|(i, _)| i + 1)).collect();
        let mut push = |ti: usize, byte: usize, via: &'static str| {
            let line = line_at(&src, byte);
            // Import / use lines name the model without querying it.
            let ls = line_starts[(line as usize - 1).min(line_starts.len() - 1)];
            let text = src[ls..].lines().next().unwrap_or("").trim_start();
            if text.starts_with("import ") || text.starts_with("from ") || text.starts_with("use ") || text.starts_with("pub use ") || text.starts_with("export {") || text.starts_with("} from") {
                return;
            }
            if refs[ti].iter().any(|r: &CodeRef| r.file == f.path && r.line == line) || refs[ti].len() >= 60 {
                return;
            }
            let symbol = index.symbol_at(fid, line).map(|s| index.symbols[s as usize].qualified());
            refs[ti].push(CodeRef { file: f.path.clone(), line, symbol, via });
        };
        for m in re_sql.captures_iter(&src) {
            if let Some(&ti) = by_name.get(&m[1].to_ascii_lowercase()) {
                push(ti, m.get(0).unwrap().start(), "sql");
            }
        }
        if let Some(re) = &re_prisma {
            for m in re.captures_iter(&src) {
                if let Some((ti, _)) = prisma.iter().find(|(_, a)| a == &m[1]) {
                    push(*ti, m.get(0).unwrap().start(), "prisma");
                }
            }
        }
        if let Some(re) = &re_diesel {
            for m in re.captures_iter(&src) {
                if let Some(&ti) = by_name.get(&m[1].to_ascii_lowercase()) {
                    push(ti, m.get(0).unwrap().start(), "diesel");
                }
            }
        }
        if let Some(re) = &re_alias {
            for m in re.captures_iter(&src) {
                if let Some(&(ti, home)) = alias_home.get(&m[1]) {
                    if imports.contains(&home) {
                        push(ti, m.get(0).unwrap().start(), "orm");
                    }
                }
            }
        }
    }
    for (t, r) in schema.tables.iter_mut().zip(refs) {
        t.used_by = r;
    }
}

// ---------------------------------------------------------------- output

fn fk_edges(schema: &DbSchema) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    for (i, t) in schema.tables.iter().enumerate() {
        for (k, fk) in t.foreign_keys.iter().enumerate() {
            let (s, n) = split_name(&fk.ref_table);
            let hit = schema.tables.iter().position(|x| x.name.eq_ignore_ascii_case(&n) && (s.is_none() || x.schema == s)).or_else(|| schema.tables.iter().position(|x| x.aliases.iter().any(|a| a == &n)));
            if let Some(j) = hit {
                out.push((i, j, k));
            }
        }
    }
    out
}

fn col_summary(t: &Table, c: &Column) -> String {
    let mut s = format!("{} {}", c.name, if c.data_type.is_empty() { "?" } else { &c.data_type });
    if c.primary_key {
        s.push_str(" PK");
    } else if !c.nullable {
        s.push_str(" NOT NULL");
    }
    if let Some(fk) = t.foreign_keys.iter().find(|f| f.columns.len() == 1 && f.columns[0].eq_ignore_ascii_case(&c.name)) {
        s.push_str(&format!(" → {}{}", fk.ref_table, fk.ref_columns.first().map(|c| format!(".{c}")).unwrap_or_default()));
    }
    s
}

impl DbSchema {
    pub fn text(&self, focus: Option<&str>) -> String {
        let mut o = String::new();
        if self.tables.is_empty() {
            let _ = writeln!(o, "No database schema found. repomap reads SQL migrations, Prisma, Drizzle, SQLAlchemy and Diesel schema files, or a live database with --url-env / url_env (Postgres, MySQL, SQLite; read-only).");
            return o;
        }
        let edges = fk_edges(self);
        let mut referenced_by: HashMap<usize, Vec<String>> = HashMap::new();
        for &(i, j, k) in &edges {
            let fk = &self.tables[i].foreign_keys[k];
            referenced_by.entry(j).or_default().push(format!("{}.{}", self.tables[i].key(), fk.columns.join(",")));
        }
        if let Some(name) = focus {
            let Some(t) = self.table(name) else {
                let _ = writeln!(o, "No table `{name}`. Tables: {}", self.tables.iter().map(|t| t.key()).collect::<Vec<_>>().join(", "));
                return o;
            };
            let i = self.tables.iter().position(|x| x.key() == t.key()).unwrap();
            let _ = writeln!(o, "# {} {} ({})", t.kind, t.key(), t.source);
            if t.aliases.len() > 0 {
                let _ = writeln!(o, "Code names: {}", t.aliases.join(", "));
            }
            let _ = writeln!(o, "\n## Columns ({})", t.columns.len());
            for c in &t.columns {
                let _ = writeln!(o, "- {}", col_summary(t, c));
            }
            if !t.indexes.is_empty() {
                let _ = writeln!(o, "\n## Indexes");
                for ix in &t.indexes {
                    let _ = writeln!(o, "- {}{} ({})", if ix.unique { "UNIQUE " } else { "" }, if ix.name.is_empty() { "-" } else { &ix.name }, ix.columns.join(", "));
                }
            }
            if let Some(rb) = referenced_by.get(&i) {
                let _ = writeln!(o, "\n## Referenced by ({})", rb.len());
                for r in rb {
                    let _ = writeln!(o, "- {r}");
                }
            }
            if !t.used_by.is_empty() {
                let _ = writeln!(o, "\n## Queried from ({})", t.used_by.len());
                for r in &t.used_by {
                    let _ = writeln!(o, "- {}:{}{} [{}]", r.file, r.line, r.symbol.as_ref().map(|s| format!(" {s}")).unwrap_or_default(), r.via);
                }
            }
            return o;
        }
        let fk_total: usize = self.tables.iter().map(|t| t.foreign_keys.len()).sum();
        let origin = if self.origin == "repo" {
            let sql = self.sources.iter().filter(|s| s.to_ascii_lowercase().ends_with(".sql")).count();
            let other: Vec<&String> = self.sources.iter().filter(|s| !s.to_ascii_lowercase().ends_with(".sql")).collect();
            let mut parts: Vec<String> = Vec::new();
            if sql > 3 {
                parts.push(format!("{sql} SQL migrations"));
            } else {
                parts.extend(self.sources.iter().filter(|s| s.to_ascii_lowercase().ends_with(".sql")).cloned());
            }
            parts.extend(other.iter().take(6).map(|s| s.to_string()));
            format!("from {}", parts.join(", "))
        } else {
            format!("live {}, read-only", self.origin)
        };
        let _ = writeln!(o, "# Database map ({origin})");
        let _ = writeln!(o, "{} tables, {} foreign keys, {} indexes, {} code references.", self.tables.len(), fk_total, self.tables.iter().map(|t| t.indexes.len()).sum::<usize>(), self.tables.iter().map(|t| t.used_by.len()).sum::<usize>());
        for (i, t) in self.tables.iter().enumerate() {
            let _ = writeln!(o, "\n## {}{} — {} columns ({})", t.key(), if t.kind == "view" { " (view)" } else { "" }, t.columns.len(), t.source);
            let cols: Vec<String> = t.columns.iter().take(14).map(|c| col_summary(t, c)).collect();
            if !cols.is_empty() {
                let _ = writeln!(o, "  {}{}", cols.join(" · "), if t.columns.len() > 14 { " · …" } else { "" });
            }
            if let Some(rb) = referenced_by.get(&i) {
                let _ = writeln!(o, "  referenced by: {}", rb.join(", "));
            }
            if !t.used_by.is_empty() {
                let top: Vec<String> = t.used_by.iter().take(5).map(|r| format!("{}:{}{}", r.file, r.line, r.symbol.as_ref().map(|s| format!(" ({s})")).unwrap_or_default())).collect();
                let _ = writeln!(o, "  queried from: {}{}", top.join(", "), if t.used_by.len() > 5 { format!(" … ({} total)", t.used_by.len()) } else { String::new() });
            }
        }
        o
    }

    /// Graph payload in the same shape as the code map, for the web UI.
    pub fn graph_json(&self, repo: &str, version: &str, web: Option<&str>, commit: Option<&str>) -> Value {
        let n = self.tables.len();
        let edges = fk_edges(self);
        let mut und: BTreeMap<(u32, u32), f32> = BTreeMap::new();
        let mut fe: Vec<crate::index::FileEdge> = Vec::new();
        for &(i, j, _) in &edges {
            if i == j {
                continue;
            }
            let key = if i < j { (i as u32, j as u32) } else { (j as u32, i as u32) };
            *und.entry(key).or_insert(0.0) += 1.0;
            fe.push(crate::index::FileEdge { from: i as u32, to: j as u32, imports: 1, calls: 0 });
        }
        let rank = crate::graph::pagerank(n, &fe);
        let und: Vec<(u32, u32, f32)> = und.into_iter().map(|((a, b), w)| (a, b, w)).collect();
        let raw = crate::graph::louvain(n, &und);
        // Communities: FK clusters; isolated tables share one group.
        let degree: Vec<usize> = (0..n).map(|i| und.iter().filter(|e| e.0 as usize == i || e.1 as usize == i).count()).collect();
        let mut groups: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for i in 0..n {
            let key = if degree[i] == 0 { u32::MAX } else { raw[i] };
            groups.entry(key).or_default().push(i);
        }
        let mut ordered: Vec<(u32, Vec<usize>)> = groups.into_iter().collect();
        ordered.sort_by(|a, b| (a.0 == u32::MAX).cmp(&(b.0 == u32::MAX)).then(b.1.len().cmp(&a.1.len())));
        let mut comm = vec![0u32; n];
        let mut communities = Vec::new();
        for (cid, (key, members)) in ordered.iter().enumerate() {
            for &m in members {
                comm[m] = cid as u32;
            }
            let name = if *key == u32::MAX {
                "standalone tables".to_string()
            } else {
                let top = members.iter().max_by(|a, b| rank[**a].partial_cmp(&rank[**b]).unwrap()).copied().unwrap_or(0);
                format!("{} group", self.tables[top].name)
            };
            communities.push(json!({"id": cid, "name": name, "size": members.len(), "kind": "core"}));
        }
        let nodes: Vec<Value> = self
            .tables
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let cols: Vec<Value> = t
                    .columns
                    .iter()
                    .map(|c| {
                        let is_fk = t.foreign_keys.iter().any(|f| f.columns.iter().any(|x| x.eq_ignore_ascii_case(&c.name)));
                        json!([format!("{}: {}", c.name, c.data_type), if c.primary_key { "pk" } else if is_fk { "fk" } else { "column" }, 0, 0, 0])
                    })
                    .collect();
                let (src_file, src_line) = t.source.rsplit_once(':').map(|(f, l)| (f.to_string(), l.parse::<u32>().unwrap_or(0))).unwrap_or((String::new(), 0));
                json!({
                    "p": t.key(),
                    "c": comm[i],
                    "r": (rank.get(i).copied().unwrap_or(0.0) * 10000.0).round() / 10000.0,
                    "l": t.columns.len(),
                    "g": t.kind,
                    "t": false,
                    "s": cols,
                    "q": t.used_by.iter().map(|r| json!([r.file, r.line, r.symbol, r.via])).collect::<Vec<_>>(),
                    "src": [src_file, src_line],
                    "ix": t.indexes.iter().map(|ix| json!([ix.name, ix.columns, ix.unique])).collect::<Vec<_>>(),
                })
            })
            .collect();
        let edge_json: Vec<Value> = edges.iter().filter(|(i, j, _)| i != j).map(|&(i, j, _)| json!([i, j, 1, 0])).collect();
        json!({
            "mode": "db",
            "version": version,
            "repo": repo,
            "web": web,
            "commit": commit,
            "origin": self.origin,
            "stats": {
                "files": n, "code_files": n,
                "symbols": self.tables.iter().map(|t| t.columns.len()).sum::<usize>(),
                "call_edges": self.tables.iter().map(|t| t.used_by.len()).sum::<usize>(),
                "file_edges": edge_json.len(),
                "index_ms": 0,
            },
            "communities": communities,
            "nodes": nodes,
            "edges": edge_json,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_migrations() {
        let mut s = DbSchema::default();
        parse_sql(&mut s, "m/001.sql", r#"
-- users
CREATE TABLE IF NOT EXISTS "public"."users" (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  email varchar(255) NOT NULL UNIQUE,
  org_id integer REFERENCES orgs(id) ON DELETE CASCADE,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE orgs (id serial, name text, PRIMARY KEY (id));
CREATE TABLE posts (
  id bigint NOT NULL,
  author_id uuid NOT NULL,
  body text,
  CONSTRAINT posts_pk PRIMARY KEY (id),
  CONSTRAINT posts_author_fk FOREIGN KEY (author_id) REFERENCES users (id)
);
CREATE UNIQUE INDEX posts_author_idx ON posts USING btree (author_id, id);
CREATE FUNCTION f() RETURNS trigger AS $$ BEGIN; select 1; END; $$ LANGUAGE plpgsql;
"#);
        parse_sql(&mut s, "m/002.sql", "ALTER TABLE posts ADD COLUMN title text NOT NULL, DROP COLUMN body; ALTER TABLE users RENAME COLUMN email TO email_address; DROP TABLE IF EXISTS orgs CASCADE;");
        let users = s.table("users").unwrap();
        assert!(users.columns.iter().any(|c| c.name == "id" && c.primary_key));
        assert!(users.columns.iter().any(|c| c.name == "email_address" && !c.nullable && c.data_type == "varchar(255)"), "{:?}", users.columns);
        assert_eq!(users.foreign_keys[0].ref_table, "orgs");
        let posts = s.table("posts").unwrap();
        assert!(posts.columns.iter().any(|c| c.name == "id" && c.primary_key));
        assert!(posts.columns.iter().any(|c| c.name == "title"));
        assert!(!posts.columns.iter().any(|c| c.name == "body"));
        assert_eq!(posts.foreign_keys[0].columns, vec!["author_id"]);
        assert!(posts.indexes.iter().any(|i| i.unique && i.columns == vec!["author_id", "id"]));
        assert!(s.table("orgs").is_none());
    }

    #[test]
    fn prisma() {
        let mut s = DbSchema::default();
        parse_prisma(&mut s, "prisma/schema.prisma", r#"
model User {
  id        String   @id @default(cuid())
  email     String   @unique
  posts     Post[]
  createdAt DateTime @default(now()) @map("created_at")
  @@map("users")
}

model Post {
  id       Int    @id @default(autoincrement())
  title    String
  author   User   @relation(fields: [authorId], references: [id])
  authorId String @map("author_id")
  @@index([authorId])
}
"#);
        let u = s.table("users").unwrap();
        assert!(u.columns.iter().any(|c| c.name == "created_at"));
        assert!(!u.columns.iter().any(|c| c.name == "posts"));
        assert!(u.aliases.contains(&"user".to_string()));
        let p = s.table("Post").unwrap();
        assert_eq!(p.foreign_keys[0].ref_table, "users");
        assert_eq!(p.foreign_keys[0].columns, vec!["authorId"]);
    }

    #[test]
    fn drizzle_sqlalchemy_diesel() {
        let mut s = DbSchema::default();
        parse_drizzle(&mut s, "src/db/schema.ts", r#"
import { pgTable, serial, text, integer } from "drizzle-orm/pg-core";
export const users = pgTable("users", {
  id: serial("id").primaryKey(),
  name: text("name").notNull(),
});
export const posts = pgTable("posts", {
  id: serial("id").primaryKey(),
  authorId: integer("author_id").references(() => users.id),
});
"#);
        let p = s.table("posts").unwrap();
        assert_eq!(p.foreign_keys[0].ref_table, "users");
        assert_eq!(p.foreign_keys[0].columns, vec!["author_id"]);
        parse_sqlalchemy(&mut s, "app/models.py", r#"
class Order(Base):
    __tablename__ = "orders"
    id = Column(Integer, primary_key=True)
    user_id = Column(
        Integer, ForeignKey("users.id"), nullable=False
    )
    total: Mapped[int] = mapped_column()
"#);
        let o = s.table("orders").unwrap();
        assert_eq!(o.foreign_keys[0].ref_table, "users");
        assert!(o.columns.iter().any(|c| c.name == "total" && c.data_type == "int"));
        parse_diesel(&mut s, "src/schema.rs", "diesel::table! {\n    /// Representation of the `comments` table.\n    ///\n    /// (Automatically generated by Diesel.)\n    comments (id) {\n        id -> Int4,\n        post_id -> Int4,\n        body -> Nullable<Text>,\n    }\n}\ndiesel::joinable!(comments -> posts (post_id));\n");
        let c = s.table("comments").unwrap();
        assert_eq!(c.foreign_keys[0].ref_table, "posts");
        assert!(c.columns.iter().any(|x| x.name == "body" && x.nullable));
    }
}
