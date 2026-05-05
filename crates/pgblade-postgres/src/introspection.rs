use pgblade_core::error::QueryError;
use pgblade_core::schema::{
    ColumnInfo, ConstraintInfo, ConstraintKind, ForeignKeyInfo, FunctionInfo, IndexInfo,
    SchemaInfo, SequenceInfo, TableInfo, TableKind, TriggerInfo,
};

use crate::error_map::map_query_error;

pub async fn fetch_schemas(client: &tokio_postgres::Client) -> Result<Vec<SchemaInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT schema_name, \
                    CASE WHEN schema_name = current_schema() THEN true ELSE false END as is_default \
             FROM information_schema.schemata \
             WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast') \
             ORDER BY schema_name",
            &[],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| SchemaInfo {
            name: row.get(0),
            is_default: row.get(1),
        })
        .collect())
}

pub async fn fetch_tables(
    client: &tokio_postgres::Client,
    schema: &str,
) -> Result<Vec<TableInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT c.relname, c.relkind::text, c.reltuples::bigint \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 \
                 AND c.relkind IN ('r', 'v', 'm', 'p') \
             ORDER BY c.relkind, c.relname",
            &[&schema],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| {
            let relkind: String = row.get(1);
            let kind = match relkind.as_str() {
                "v" => TableKind::View,
                "m" => TableKind::MaterializedView,
                _ => TableKind::Table,
            };
            TableInfo {
                schema: schema.to_string(),
                name: row.get(0),
                kind,
                row_estimate: row.get(2),
            }
        })
        .collect())
}

pub async fn fetch_columns(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default, \
                    c.ordinal_position::int, \
                    EXISTS( \
                        SELECT 1 FROM information_schema.table_constraints tc \
                        JOIN information_schema.key_column_usage kcu \
                            ON tc.constraint_name = kcu.constraint_name \
                            AND tc.table_schema = kcu.table_schema \
                        WHERE tc.constraint_type = 'PRIMARY KEY' \
                            AND tc.table_schema = $1 \
                            AND tc.table_name = $2 \
                            AND kcu.column_name = c.column_name \
                    ) as is_pk \
             FROM information_schema.columns c \
             WHERE c.table_schema = $1 AND c.table_name = $2 \
             ORDER BY c.ordinal_position",
            &[&schema, &table],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| {
            let nullable_str: String = row.get(2);
            ColumnInfo {
                name: row.get(0),
                data_type: row.get(1),
                nullable: nullable_str == "YES",
                is_primary_key: row.get(5),
                default_value: row.get(3),
                ordinal_position: row.get(4),
            }
        })
        .collect())
}

pub async fn fetch_constraints(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<Vec<ConstraintInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT con.conname, con.contype::text, \
                    array_agg(att.attname ORDER BY u.ord)::text[] \
             FROM pg_constraint con \
             JOIN pg_class rel ON rel.oid = con.conrelid \
             JOIN pg_namespace nsp ON nsp.oid = rel.relnamespace \
             CROSS JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS u(attnum, ord) \
             JOIN pg_attribute att ON att.attrelid = rel.oid AND att.attnum = u.attnum \
             WHERE nsp.nspname = $1 AND rel.relname = $2 \
             GROUP BY con.conname, con.contype \
             ORDER BY con.conname",
            &[&schema, &table],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| {
            let contype: String = row.get(1);
            let kind = match contype.as_str() {
                "p" => ConstraintKind::PrimaryKey,
                "f" => ConstraintKind::ForeignKey,
                "u" => ConstraintKind::Unique,
                "c" => ConstraintKind::Check,
                "x" => ConstraintKind::Exclusion,
                _ => ConstraintKind::Check,
            };
            let columns: Vec<String> = row.get(2);
            ConstraintInfo {
                name: row.get(0),
                kind,
                columns,
            }
        })
        .collect())
}

pub async fn fetch_foreign_keys(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT tc.constraint_name, \
                    array_agg(kcu.column_name ORDER BY kcu.ordinal_position)::text[] as columns, \
                    ccu.table_name AS referenced_table, \
                    array_agg(ccu.column_name ORDER BY kcu.ordinal_position)::text[] as referenced_columns \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
                 ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
             JOIN information_schema.constraint_column_usage ccu \
                 ON tc.constraint_name = ccu.constraint_name AND tc.table_schema = ccu.table_schema \
             WHERE tc.constraint_type = 'FOREIGN KEY' \
                 AND tc.table_schema = $1 AND tc.table_name = $2 \
             GROUP BY tc.constraint_name, ccu.table_name \
             ORDER BY tc.constraint_name",
            &[&schema, &table],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| ForeignKeyInfo {
            name: row.get(0),
            columns: row.get(1),
            referenced_table: row.get(2),
            referenced_columns: row.get(3),
        })
        .collect())
}

pub async fn fetch_indexes(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT i.relname, ix.indisunique, am.amname, \
                    array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum))::text[] \
             FROM pg_index ix \
             JOIN pg_class t ON t.oid = ix.indrelid \
             JOIN pg_class i ON i.oid = ix.indexrelid \
             JOIN pg_namespace n ON n.oid = t.relnamespace \
             JOIN pg_am am ON am.oid = i.relam \
             JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey) \
             WHERE n.nspname = $1 AND t.relname = $2 \
             GROUP BY i.relname, ix.indisunique, am.amname \
             ORDER BY i.relname",
            &[&schema, &table],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| IndexInfo {
            name: row.get(0),
            is_unique: row.get(1),
            index_type: row.get(2),
            columns: row.get(3),
        })
        .collect())
}

pub async fn fetch_triggers(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Result<Vec<TriggerInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT trigger_name, action_timing, event_manipulation \
             FROM information_schema.triggers \
             WHERE trigger_schema = $1 AND event_object_table = $2 \
             ORDER BY trigger_name",
            &[&schema, &table],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| TriggerInfo {
            name: row.get(0),
            timing: row.get(1),
            event: row.get(2),
        })
        .collect())
}

pub async fn fetch_functions(
    client: &tokio_postgres::Client,
    schema: &str,
) -> Result<Vec<FunctionInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT p.proname, n.nspname, \
                    pg_get_function_arguments(p.oid) as args, \
                    pg_get_function_result(p.oid) as return_type, \
                    l.lanname \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             JOIN pg_language l ON l.oid = p.prolang \
             WHERE n.nspname = $1 \
                 AND p.prokind IN ('f', 'p') \
             ORDER BY p.proname",
            &[&schema],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| FunctionInfo {
            name: row.get(0),
            schema: row.get(1),
            arguments: row.get(2),
            return_type: row.get(3),
            language: row.get(4),
        })
        .collect())
}

pub async fn fetch_sequences(
    client: &tokio_postgres::Client,
    schema: &str,
) -> Result<Vec<SequenceInfo>, QueryError> {
    let rows = client
        .query(
            "SELECT sequencename, data_type \
             FROM pg_sequences \
             WHERE schemaname = $1 \
             ORDER BY sequencename",
            &[&schema],
        )
        .await
        .map_err(map_query_error)?;

    Ok(rows
        .iter()
        .map(|row| SequenceInfo {
            name: row.get(0),
            schema: schema.to_string(),
            data_type: row.get(1),
        })
        .collect())
}
