/// A highlighted range in a SQL string.
#[derive(Debug, Clone)]
pub struct HighlightRange {
    pub start: usize, // byte offset
    pub end: usize,   // byte offset
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    String,
    Number,
    Comment,
    Operator,
    Identifier,
}

/// Tokenize SQL text for syntax highlighting.
/// Returns ranges sorted by start position, non-overlapping.
pub fn highlight_sql(sql: &str) -> Vec<HighlightRange> {
    let mut ranges = Vec::new();
    let mut i = 0;
    let bytes = sql.as_bytes();

    while i < bytes.len() {
        // Skip whitespace
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Single-line comment: -- ...
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            ranges.push(HighlightRange {
                start,
                end: i,
                kind: TokenKind::Comment,
            });
            continue;
        }

        // Block comment: /* ... */
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2;
            }
            ranges.push(HighlightRange {
                start,
                end: i,
                kind: TokenKind::Comment,
            });
            continue;
        }

        // String literal: '...'
        if bytes[i] == b'\'' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\'' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                        i += 2; // escaped quote ''
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            ranges.push(HighlightRange {
                start,
                end: i,
                kind: TokenKind::String,
            });
            continue;
        }

        // Number
        if bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            ranges.push(HighlightRange {
                start,
                end: i,
                kind: TokenKind::Number,
            });
            continue;
        }

        // Operators
        if b"=<>!+-*/%&|^~".contains(&bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && b"=<>!".contains(&bytes[i]) {
                i += 1;
            }
            ranges.push(HighlightRange {
                start,
                end: i,
                kind: TokenKind::Operator,
            });
            continue;
        }

        // Parentheses, commas, semicolons -- skip (default color)
        if b"(),;".contains(&bytes[i]) {
            i += 1;
            continue;
        }

        // Word (identifier or keyword)
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &sql[start..i];
            let kind = if is_sql_keyword(word) {
                TokenKind::Keyword
            } else {
                TokenKind::Identifier
            };
            ranges.push(HighlightRange {
                start,
                end: i,
                kind,
            });
            continue;
        }

        // Quoted identifier: "..."
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            ranges.push(HighlightRange {
                start,
                end: i,
                kind: TokenKind::Identifier,
            });
            continue;
        }

        // Dollar-quoted string: $$...$$ or $tag$...$tag$
        if bytes[i] == b'$' {
            let start = i;
            let tag_start = i;
            i += 1;
            // Find the tag (between $ and next $)
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'$' {
                let tag = &sql[tag_start..=i];
                i += 1;
                // Find closing tag
                if let Some(pos) = sql[i..].find(tag) {
                    i += pos + tag.len();
                } else {
                    i = bytes.len();
                }
                ranges.push(HighlightRange {
                    start,
                    end: i,
                    kind: TokenKind::String,
                });
            }
            // If no matching $, just skip the single $ character
            continue;
        }

        i += 1;
    }

    ranges
}

fn is_sql_keyword(word: &str) -> bool {
    matches!(
        word.to_uppercase().as_str(),
        "SELECT"
            | "FROM"
            | "WHERE"
            | "AND"
            | "OR"
            | "NOT"
            | "IN"
            | "IS"
            | "NULL"
            | "INSERT"
            | "INTO"
            | "VALUES"
            | "UPDATE"
            | "SET"
            | "DELETE"
            | "CREATE"
            | "DROP"
            | "ALTER"
            | "TABLE"
            | "VIEW"
            | "INDEX"
            | "SCHEMA"
            | "DATABASE"
            | "JOIN"
            | "LEFT"
            | "RIGHT"
            | "INNER"
            | "OUTER"
            | "CROSS"
            | "FULL"
            | "ON"
            | "GROUP"
            | "BY"
            | "ORDER"
            | "ASC"
            | "DESC"
            | "HAVING"
            | "LIMIT"
            | "OFFSET"
            | "AS"
            | "DISTINCT"
            | "ALL"
            | "EXISTS"
            | "BETWEEN"
            | "LIKE"
            | "ILIKE"
            | "CASE"
            | "WHEN"
            | "THEN"
            | "ELSE"
            | "END"
            | "BEGIN"
            | "COMMIT"
            | "ROLLBACK"
            | "SAVEPOINT"
            | "TRANSACTION"
            | "TRUE"
            | "FALSE"
            | "IF"
            | "ELSIF"
            | "LOOP"
            | "WHILE"
            | "FOR"
            | "RETURN"
            | "WITH"
            | "RECURSIVE"
            | "UNION"
            | "INTERSECT"
            | "EXCEPT"
            | "PRIMARY"
            | "KEY"
            | "FOREIGN"
            | "REFERENCES"
            | "UNIQUE"
            | "CHECK"
            | "CONSTRAINT"
            | "DEFAULT"
            | "SERIAL"
            | "BIGSERIAL"
            | "INTEGER"
            | "INT"
            | "BIGINT"
            | "SMALLINT"
            | "TEXT"
            | "VARCHAR"
            | "CHAR"
            | "BOOLEAN"
            | "BOOL"
            | "TIMESTAMP"
            | "DATE"
            | "TIME"
            | "INTERVAL"
            | "UUID"
            | "NUMERIC"
            | "DECIMAL"
            | "REAL"
            | "FLOAT"
            | "DOUBLE"
            | "JSON"
            | "JSONB"
            | "ARRAY"
            | "BYTEA"
            | "EXPLAIN"
            | "ANALYZE"
            | "VERBOSE"
            | "GRANT"
            | "REVOKE"
            | "TRUNCATE"
            | "CASCADE"
            | "RESTRICT"
            | "RETURNING"
            | "CONFLICT"
            | "DO"
            | "NOTHING"
            | "EXCLUDED"
            | "MATERIALIZED"
            | "REFRESH"
            | "CONCURRENTLY"
            | "COALESCE"
            | "NULLIF"
            | "GREATEST"
            | "LEAST"
            | "CAST"
            | "COUNT"
            | "SUM"
            | "AVG"
            | "MIN"
            | "MAX"
            | "OVER"
            | "PARTITION"
            | "WINDOW"
            | "ROWS"
            | "RANGE"
            | "PRECEDING"
            | "FOLLOWING"
            | "LATERAL"
            | "UNNEST"
            | "GENERATE_SERIES"
            | "COPY"
            | "TO"
            | "STDOUT"
            | "STDIN"
            | "CSV"
            | "HEADER"
            | "DELIMITER"
            | "PERFORM"
            | "RAISE"
            | "NOTICE"
            | "EXCEPTION"
            | "FOUND"
            | "FETCH"
            | "NEXT"
            | "PRIOR"
            | "FIRST"
            | "LAST"
            | "ABSOLUTE"
            | "RELATIVE"
            | "DECLARE"
            | "CURSOR"
            | "CLOSE"
            | "OPEN"
            | "TYPE"
            | "ENUM"
            | "DOMAIN"
            | "SEQUENCE"
            | "OWNED"
            | "NONE"
            | "TRIGGER"
            | "FUNCTION"
            | "PROCEDURE"
            | "LANGUAGE"
            | "PLPGSQL"
            | "SQL"
            | "SECURITY"
            | "DEFINER"
            | "INVOKER"
            | "VOLATILE"
            | "STABLE"
            | "IMMUTABLE"
            | "EXTENSION"
            | "COMMENT"
            | "OWNER"
            | "RENAME"
            | "COLUMN"
            | "ADD"
            | "ENABLE"
            | "DISABLE"
            | "ALWAYS"
            | "REPLICA"
            | "IDENTITY"
            | "TEMP"
            | "TEMPORARY"
            | "UNLOGGED"
            | "INHERITS"
            | "INCLUDING"
            | "VACUUM"
            | "REINDEX"
            | "CLUSTER"
            | "DISCARD"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_keywords() {
        let ranges = highlight_sql("SELECT * FROM users");
        let keywords: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::Keyword)
            .collect();
        assert_eq!(keywords.len(), 2); // SELECT, FROM
    }

    #[test]
    fn highlights_strings() {
        let ranges = highlight_sql("SELECT 'hello world'");
        let strings: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::String)
            .collect();
        assert_eq!(strings.len(), 1);
    }

    #[test]
    fn highlights_comments() {
        let ranges = highlight_sql("-- this is a comment\nSELECT 1");
        let comments: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn highlights_numbers() {
        let ranges = highlight_sql("SELECT 42, 3.14");
        let numbers: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::Number)
            .collect();
        assert_eq!(numbers.len(), 2);
    }

    #[test]
    fn highlights_block_comment() {
        let ranges = highlight_sql("/* block */ SELECT 1");
        let comments: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
        assert_eq!(
            &"/* block */ SELECT 1"[comments[0].start..comments[0].end],
            "/* block */"
        );
    }

    #[test]
    fn highlights_escaped_string() {
        let ranges = highlight_sql("SELECT 'it''s'");
        let strings: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::String)
            .collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(
            &"SELECT 'it''s'"[strings[0].start..strings[0].end],
            "'it''s'"
        );
    }

    #[test]
    fn highlights_quoted_identifier() {
        let ranges = highlight_sql("SELECT \"my column\" FROM t");
        let identifiers: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::Identifier)
            .collect();
        // "my column" and t
        assert_eq!(identifiers.len(), 2);
    }

    #[test]
    fn highlights_dollar_quoted_string() {
        let ranges = highlight_sql("$$hello$$");
        let strings: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::String)
            .collect();
        assert_eq!(strings.len(), 1);
    }

    #[test]
    fn keywords_are_case_insensitive() {
        let ranges = highlight_sql("select from WHERE");
        let keywords: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::Keyword)
            .collect();
        assert_eq!(keywords.len(), 3);
    }

    #[test]
    fn empty_input_returns_no_ranges() {
        let ranges = highlight_sql("");
        assert!(ranges.is_empty());
    }

    #[test]
    fn operators_detected() {
        let ranges = highlight_sql("a >= b");
        let ops: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == TokenKind::Operator)
            .collect();
        assert_eq!(ops.len(), 1);
    }
}
