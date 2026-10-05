//! Responsibilities: restrict SQL accepted by the facade before engine preparation.
//! Not responsible for: parsing application query semantics or constructing SQL.
//! Failure modes: TEMP, VACUUM, attachment, transaction/pragma or metadata mutation
//! bypasses ownership; comments/quotes hide restricted SQL; ordinary literals fail.
//! Tests: restrictions and sql tests.
use crate::{Error, Result};

// A lexical allowlist, not a second SQL parser. Turso still parses every accepted
// statement. Only one statement is allowed; migrations use one call per statement.
// Identifiers are decoded, comments/string contents are skipped, and reserved
// namespaces are excluded throughout (including trigger bodies and CTEs).
pub(crate) fn validate(sql: &str, read_only: bool) -> Result<bool> {
    let words = tokens(sql)?;
    let Some(first) = words.first() else { return Err(Error::RestrictedSql("empty statement")) };
    let reading = matches!(first.as_str(), "SELECT" | "WITH" | "EXPLAIN");
    if read_only && !reading {
        return Err(Error::RestrictedSql("reader SQL must be a query"));
    }
    if !matches!(
        first.as_str(),
        "SELECT"
            | "WITH"
            | "EXPLAIN"
            | "INSERT"
            | "REPLACE"
            | "UPDATE"
            | "DELETE"
            | "CREATE"
            | "ALTER"
            | "DROP"
            | "ANALYZE"
    ) {
        return Err(Error::RestrictedSql(
            "connection and transaction control belongs to vault-store",
        ));
    }
    for word in &words {
        if matches!(
            word.as_str(),
            "TEMP" | "TEMPORARY" | "VACUUM" | "ATTACH" | "DETACH" | "PRAGMA" | "__PK_MIGRATIONS"
        ) || word.starts_with("PRAGMA_")
        {
            return Err(Error::RestrictedSql(
                "TEMP, VACUUM, connection controls and storage metadata are private",
            ));
        }
    }
    // Attached scratch has its own writer; read it here, write via Db::Scratch.
    if words.iter().any(|w| w == "SCRATCH")
        && words.iter().any(|w| {
            matches!(
                w.as_str(),
                "INSERT" | "REPLACE" | "UPDATE" | "DELETE" | "CREATE" | "ALTER" | "DROP"
            )
        })
    {
        return Err(Error::RestrictedSql("scratch writes belong to Db::Scratch"));
    }
    Ok(words.iter().any(|word| word == "SCRATCH"))
}
fn tokens(sql: &str) -> Result<Vec<String>> {
    let b = sql.as_bytes();
    let mut i = 0;
    let mut result: Vec<String> = Vec::new();
    let mut ended = false;
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"--") {
            while i < b.len() && b[i] != b'\n' {
                i += 1
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i += 2;
            while i + 1 < b.len() && !b[i..].starts_with(b"*/") {
                i += 1
            }
            i = (i + 2).min(b.len());
            continue;
        }
        if ended {
            return Err(Error::RestrictedSql("one SQL statement per call"));
        }
        if b[i] == b';' {
            ended = true;
            i += 1;
            continue;
        }
        if matches!(b[i], b'\'' | b'"' | b'`' | b'[') {
            let quote_start = i;
            let identifier_context = result.last().is_some_and(|word| {
                matches!(
                    word.as_str(),
                    "FROM"
                        | "JOIN"
                        | "UPDATE"
                        | "INTO"
                        | "TABLE"
                        | "EXISTS"
                        | "ON"
                        | "REFERENCES"
                        | "VIEW"
                        | "TRIGGER"
                        | "INDEX"
                        | "TO"
                        | "ABORT"
                        | "FAIL"
                        | "IGNORE"
                        | "ROLLBACK"
                        | "REPLACE"
                        | "."
                )
            }) || sql[..quote_start].trim_end().ends_with('.');
            let opener = b[i];
            let closer = if opener == b'[' { b']' } else { opener };
            i += 1;
            let mut value = Vec::new();
            while i < b.len() {
                if b[i] == closer {
                    i += 1;
                    if i < b.len() && b[i] == closer && opener != b'[' {
                        value.push(closer);
                        i += 1;
                    } else {
                        break;
                    }
                } else {
                    value.push(b[i]);
                    i += 1
                }
            }
            if opener != b'\'' || identifier_context || sql[i..].trim_start().starts_with('.') {
                result.push(String::from_utf8_lossy(&value).to_uppercase());
            }
        } else if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1
            }
            result.push(sql[start..i].to_ascii_uppercase());
        } else {
            if b[i] == b'.' {
                result.push(".".into());
            }
            i += 1
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_and_quoted_identifiers_do_not_hide_restrictions() {
        for sql in [
            "CREATE /*x*/ TEMP TABLE a(id)",
            "CREATE TABLE [temp].a(id)",
            "CREATE TABLE `temp`.a(id)",
            "SELECT 1; VACUUM INTO 'a'",
            "-- x\nCOMMIT",
        ] {
            assert!(validate(sql, false).is_err(), "{sql}");
        }
        assert!(validate("SELECT 'it''s TEMP; VACUUM', 1 -- TEMP\n", true).is_ok());
    }
}
