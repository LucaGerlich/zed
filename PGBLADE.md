# PgBlade -- PostgreSQL IDE built on Zed

PgBlade is a fork of [Zed](https://zed.dev) that adds a comprehensive PostgreSQL database IDE directly into the editor. Get Zed's speed and editing power with DBeaver-level database features.

**53 commits | 102 actions | ~15K lines of database code | Zero clippy warnings**

## Features

### Connection Management
- Save, edit, delete connection profiles with right-click context menu
- SSH tunnel support (agent and key file authentication)
- SSL mode configuration (Disable / Prefer / Require)
- Passwords stored locally (base64 encoded in SQLite, keychain fallback)
- Test connection before saving
- Auto-detect DATABASE_URL from project `.env` files
- Connection health ping (SELECT 1 every 30s, auto-disconnect on failure)
- Auto-open database panel when saved connections exist
- Connection color indicator in status bar (green=dev, yellow=staging, red=production)
- Connection uptime display
- Generate connection strings in 8 formats (URI, JDBC, .NET, Python, Node.js, Go, Rust, env var)

### Schema Browser
- DBeaver-style hierarchical tree: Database > Schemas > Tables/Views/Functions/Sequences
- Per-table details: Columns (with PK/nullable indicators), Constraints, Foreign Keys, Indexes, Triggers
- Estimated row counts on tables (~1.2K, ~3.5M format)
- Table action buttons: SEL | INS | UPD | IDX | DDL | CNT | TRC | VAC
- View definitions (DEF), function/trigger source (SRC), materialized view refresh (REF)
- Single-click to expand, double-click to preview data
- Schema object count summary in header
- Auto-refresh schema tree after DDL queries (CREATE/ALTER/DROP)
- SERVER admin section: Roles, Sessions, Settings, Extensions, Tablespaces, Locks, Activity, Health

### Quick Action Toolbar
Two rows of icon buttons when connected:
- **Row 1:** Active Sessions | Lock Monitor | Table Sizes | Slow Queries | Table Bloat
- **Row 2:** Health Check | Roles & Users | Server Settings

### Query Execution
- Execute SQL from any editor: **Cmd+Enter**
- Execute selected text only (highlight + Cmd+Enter)
- SQL-in-any-file: strips Python triple quotes, JS template literals, Rust raw strings
- EXPLAIN ANALYZE with plan visualization: **Cmd+Alt+X**
  - Warning indicators: SLOW (>100ms), BAD ESTIMATE (10x off), LARGE SEQ SCAN
  - Sort method, buffer stats, parallel worker info, rows removed by filter
- Query plan comparison: write two queries separated by `---`, compare plans side-by-side
- Query performance analysis: runs query 3x, shows min/max/avg/median timing with recommendations
- Query timeout configuration via SetTimeout action

### SQL Auto-Completion
- Native Zed CompletionProvider (inline popup, not a picker)
- Schema-aware: tables, columns, functions, sequences, schemas
- 50+ SQL keywords
- Merges with Zed's LSP completions (composite/decorator pattern)
- Updates when schema loads, clears on disconnect
- Also available as Ctrl+Space picker for browsing

### Result Grid
- Built on Zed's native `ui::Table` component
- Column sorting (click header, toggle asc/desc)
- Inline row filtering (type to filter when panel is focused, Escape to clear)
- Row selection with Edit Row / Delete Row SQL generation
- Type-based cell coloring (numbers=accent, booleans=warning, NULLs=disabled)
- Multiple result tabs with smart labels ("SELECT users" not raw SQL)
- Tab pinning (pinned tabs survive close and clear)
- Load More pagination (500 rows default, "Load 500 more" / "Load All" buttons)
- Refresh button re-executes last query
- Data masking toggle (masks password/token/email/phone columns)
- Large cell values truncated to 500 chars

### Data Export
- **To clipboard:** CSV, JSON, INSERT, UPDATE, DELETE, TSV (Cmd+C)
- **To file:** CSV and JSON saved to ~/Downloads/ with timestamp filenames
- Copy all results or generate per-row SQL statements

### SQL Tools
- **SQL Formatting:** uppercase keywords, 4-space indent via sqlformat
- **SQL Linting:** 10 anti-pattern checks (SELECT *, missing WHERE, leading LIKE wildcards, NOT IN with subquery, Cartesian joins, deep nesting, etc.) with severity levels
- **AI Context Builder:** prepares query + full schema context for Zed's AI assistant (Cmd+;)

### Code Generation (Schema-Aware)
All generators use actual column names/types when connected:
- **CRUD Generator:** SELECT/INSERT/UPDATE/DELETE/UPSERT with PK detection and $N placeholders
- **Index Patterns:** B-tree, GIN, GiST, BRIN, partial, expression, covering, concurrent
- **ALTER TABLE Templates:** add/modify/drop column, constraints, FK, rename
- **GRANT/REVOKE Templates:** SELECT, INSERT/UPDATE/DELETE, ALL, WITH GRANT OPTION
- **CREATE FUNCTION/TRIGGER Templates:** PL/pgSQL function and trigger boilerplate
- **RLS Template:** Row Level Security policies with user isolation pattern
- **Partition Template:** range/list partition creation, detach/attach
- **Data Profile:** null percentages, distinct counts, random sample per column
- **Migration Template:** timestamped UP/DOWN migration with BEGIN/COMMIT

### SQL Snippets (Command Palette)
- CTE (WITH ... AS)
- Window Function (ROW_NUMBER, SUM OVER)
- Upsert (INSERT ON CONFLICT DO UPDATE)
- Pivot / Crosstab (COUNT FILTER)
- Recursive CTE (tree traversal)
- JSON Query (->>, @>, jsonb_array_length)
- LATERAL JOIN
- SELECT with JOIN (auto-generated from foreign keys)
- GROUP BY Summary

### Query Bookmarks
- Save selected SQL as a named bookmark (persisted to SQLite)
- View all bookmarks from command palette

### Query History
- Every executed query saved with timestamp, classification, database
- View recent 100 queries from command palette

### DBA Performance Analysis (22 actions)
- pg_stat_statements (top queries by time)
- Table Access Stats (seq vs index scans, DML counts)
- Active Queries (currently running, with duration)
- Slow Queries (non-idle, ordered by duration)
- Cache Hit Ratio (index, table, total)
- Index Usage / Unused Indexes / Missing Indexes / Duplicate Indexes
- Table Bloat (dead tuples, bloat percentage)
- Vacuum Progress / Autovacuum Status
- Maintenance Recommendations (auto-detect tables needing VACUUM/ANALYZE)
- IO Stats (heap/index cache hit ratios per table)
- Checkpoint Stats (write/sync times, buffer stats)
- Connection Age (long-lived connections)
- Database Health Check (11 key metrics in one query)
- Query Performance Analysis (3-iteration benchmark with recommendations)

### Server Administration (17 actions)
- View/Kill Sessions (pg_stat_activity, pg_terminate_backend)
- View Locks (blocked/blocking query pairs)
- Database Info / Table Sizes / Connection Stats / Connection Limits
- WAL Status / Replication Status
- View Settings (non-default pg_settings)
- View HBA Rules (pg_hba_file_rules)
- Config File Locations
- Database Activity (commits, rollbacks, cache, DML per database)

### User & Role Management (6 actions)
- Create Role / Alter Role templates
- Role Memberships (pg_auth_members hierarchy)
- Role Permissions (per-role privileges)
- Transfer Ownership (REASSIGN OWNED BY)

### Security (4 actions)
- View Privileges / View Policies (RLS)
- Security Audit (SSL, logging, timeout settings)
- RLS Template (Row Level Security boilerplate)

### Schema Management
- Create Schema / Create Database templates
- Object Ownership viewer
- Schema Comparison (column/index/FK counts)
- Schema DDL Dump (full CREATE TABLE + INDEX export)
- ER Diagram (Mermaid erDiagram format)
- View Definitions / Dependencies / Comments / Extensions
- Table Structure / Column Stats / Sequence Values
- Event Triggers / Publications / Subscriptions
- Partitions / Foreign Tables / Tablespaces

### Data Import
- Import CSV from clipboard (generates INSERT statements)
- Import CSV and Execute (generates + runs INSERT statements)
- Detect migration files in project (8 common directory patterns)

### Status Bar
- Connection indicator at bottom of window
- Shows database@host with uptime (5s, 3m, 1h)
- Color-coded by environment (green/yellow/red)
- Tooltip with full connection details

### From Zed (all features preserved)
- Multi-cursor editing, vim mode, LSP support
- Git panel, blame, diff
- Global search (Cmd+Shift+F), file finder (Cmd+P)
- 50+ themes, settings system
- Integrated terminal
- Extensions, collaboration
- Tree-sitter syntax highlighting for all languages

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+Enter | Execute query (selected text or full buffer) |
| Cmd+Alt+X | EXPLAIN ANALYZE |
| Ctrl+Space | SQL auto-completion picker |
| Ctrl+Shift+D | Toggle database panel |
| Cmd+Shift+P | Command palette (all 102 actions searchable) |

## Building

```bash
# Clone the fork
git clone https://github.com/LucaGerlich/zed.git pgblade
cd pgblade

# Build (requires Rust 1.85+, macOS)
cargo build --release

# Run
cargo run --release

# Run database-specific tests (requires Docker)
cargo test -p pgblade-core
cargo test -p pgblade-postgres  # needs: docker run -d --name pgblade-test -p 15432:5432 -e POSTGRES_USER=pgblade_test -e POSTGRES_PASSWORD=pgblade_test -e POSTGRES_DB=pgblade_test postgres:16-alpine
```

## Architecture

PgBlade adds 4 crates to Zed's 220+ crate workspace:

| Crate | Purpose |
|-------|---------|
| `database_panel` | Zed panels (ConnectionPanel, ResultPanel, DatabaseStatusItem), 102 workspace actions, SQL completion provider |
| `pgblade-core` | Domain types, SQLite storage (connections, history, bookmarks), SQL classifier, schema types, SSH tunnel |
| `pgblade-postgres` | Async PostgreSQL driver (tokio-postgres), schema introspection, error mapping, row type conversion |
| `pgblade-security` | macOS Keychain credential storage (keyring crate) |

### Key Design Decisions
- **Zed's Table component** for result grid (virtualized, column resize, built-in scrolling)
- **CompletionProvider trait** for inline auto-completion (merges with LSP, not a separate picker)
- **SQLite-first password storage** with keychain as fallback (Zed fork binary lacks keychain entitlements)
- **System SSH binary** for tunnels (leverages user's ~/.ssh/config, no Rust SSH library needed)
- **base64 password encoding** in SQLite (obfuscation, same level as DBeaver default)
- **Async tokio runtime** in ConnectionPanel for non-blocking query execution
- **EventEmitter pattern** for schema refresh after DDL queries
- **WeakEntity references** for cross-panel communication (ConnectionPanel <-> ResultPanel)

## License

PgBlade is a fork of Zed. The fork inherits Zed's GPL-3.0-or-later license.
The pgblade-core, pgblade-postgres, and pgblade-security crates are Apache-2.0 licensed.

## Comparison with DBeaver

| Feature | PgBlade | DBeaver Free |
|---------|---------|--------------|
| Editor quality | Zed (multi-cursor, vim, LSP) | Basic SQL editor |
| Startup time | ~2 seconds | 10-30 seconds |
| Memory usage | ~100MB | 500MB+ |
| Git integration | Full (blame, diff, search) | None |
| Terminal | Integrated | None |
| Themes | 50+ | ~5 |
| Schema browser | Full hierarchy + admin | Full hierarchy |
| Query execution | Selected text + full buffer | Selected text + full buffer |
| Auto-completion | Schema-aware + LSP | Schema-aware |
| Result grid | Zed Table (virtualized) | Custom grid |
| Data export | 5 formats + file | Multiple formats |
| SQL formatting | Yes | Yes |
| EXPLAIN visualization | Text tree with warnings | Graphical |
| DBA tools | 22 performance actions | Session manager |
| Admin tools | 17 server actions | Limited in free |
| Code generation | Schema-aware CRUD, indexes | Templates |
| SQL linting | 10 anti-patterns | None in free |
| AI integration | Context builder for Zed AI | None in free |
| SSH tunnels | Yes | Yes |
| Data masking | Yes (toggle) | Enterprise only |
| Connection health | Auto-ping | None |
| .env detection | Yes | None |
| SQL-in-any-file | Yes (strips quotes) | None |
