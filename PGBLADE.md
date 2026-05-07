# PgBlade -- PostgreSQL IDE built on Zed

PgBlade is a fork of [Zed](https://zed.dev) that adds a comprehensive PostgreSQL database IDE directly into the editor. Get Zed's speed and editing power with DBeaver-level database features.

## Features

### Connection Management
- Save, edit, delete connection profiles
- SSH tunnel support (agent and key file auth)
- SSL mode configuration (Disable/Prefer/Require)
- Passwords stored locally (base64 encoded in SQLite)
- Test connection before saving
- Right-click context menu on saved connections
- Auto-open database panel when saved connections exist

### Schema Browser
- DBeaver-style hierarchical tree: Database > Schemas > Tables/Views/Functions/Sequences
- Per-table details: Columns, Constraints, Foreign Keys, Indexes, Triggers
- Estimated row counts on tables
- Quick action toolbar: Sessions, Locks, Sizes, Slow Queries, Bloat
- Table actions: SELECT, INSERT, UPDATE, INDEX, DDL, COUNT, TRUNCATE, VACUUM
- View definitions (DEF), function/trigger source (SRC)
- Materialized view refresh (REF)
- Single-click to expand, double-click to preview data
- Schema object count summary in header

### Query Execution
- Execute SQL from any editor: **Cmd+Enter**
- Execute selected text only (highlight + Cmd+Enter)
- EXPLAIN ANALYZE with plan visualization: **Cmd+Alt+X**
  - Warning indicators: SLOW, BAD ESTIMATE, LARGE SEQ SCAN
  - Sort method, buffer stats, parallel worker info
- SQL auto-completion: **Ctrl+Space** (or automatic while typing)
  - Tables, columns, functions, sequences, SQL keywords
  - Merges with Zed's LSP completions
- SQL formatting: command palette "Format SQL"
- Load More pagination (500 rows default, expandable)

### Result Grid
- Built on Zed's native Table component
- Column sorting (click header, toggle asc/desc)
- Inline row filtering (type to filter when focused)
- Row selection + Edit/Delete SQL generation
- Type-based cell coloring (numbers, booleans, NULLs)
- Multiple result tabs with smart labels
- Refresh, Clear buttons

### Data Export
- CSV to clipboard + file (~Downloads/)
- JSON to clipboard + file
- SQL INSERT statements
- SQL UPDATE statements
- SQL DELETE statements
- Copy all results as TSV (Cmd+C)

### SQL Snippets
- CTE, Window Function, Upsert, Pivot
- Recursive CTE, JSON Query, LATERAL JOIN
- SELECT with JOIN (auto-generated from FK relationships)
- GROUP BY Summary template

### Query Bookmarks
- Save frequently used queries
- View bookmarks from command palette

### Status Bar Indicator
- Always-visible connection status in Zed's status bar
- Shows connected database@host with uptime when connected
- Shows muted "No DB" indicator when disconnected
- Automatic updates on connect/disconnect

### 50+ DBA/Admin Actions
All available via command palette (Cmd+Shift+P):

**Performance:** pg_stat_statements, Table Access Stats, Active Queries, Slow Queries, Cache Hit Ratio
**Indexes:** Usage, Unused, Missing, Duplicate
**Maintenance:** Bloat, Vacuum Progress, Autovacuum, Recommendations
**Admin:** Sessions, Locks, Kill Backend, DB Info, Table Sizes, Connection Stats/Limits, WAL Status
**Security:** Privileges, Roles
**Schema:** Comments, Extensions, Definitions, Dependencies, Partitions, Foreign Tables, Tablespaces, Replication, Settings, Sequences, Column Stats, Structure
**Development:** Schema Compare, DDL Dump, ER Diagram (Mermaid), CSV Import, Create Function/Trigger, GRANT/REVOKE, ALTER TABLE templates
**Health:** Database Health Check (11 metrics in one query)

### Keyboard Shortcuts
| Shortcut | Action |
|----------|--------|
| Cmd+Enter | Execute query |
| Cmd+Alt+X | EXPLAIN ANALYZE |
| Ctrl+Space | SQL auto-completion |
| Ctrl+Shift+D | Toggle database panel |
| Cmd+Shift+P | Command palette (all actions) |

### From Zed (all features preserved)
- Multi-cursor editing, vim mode, LSP support
- Git panel, blame, diff
- Global search, file finder
- 50+ themes, settings, terminal
- Extensions, collaboration

## Building

```bash
# Clone the fork
git clone https://github.com/LucaGerlich/zed.git pgblade
cd pgblade

# Build (requires Rust 1.85+)
cargo build --release

# Run
cargo run --release

# Run tests (requires Docker for integration tests)
./scripts/test-db.sh start  # if test script exists
cargo test -p pgblade-core -p pgblade-postgres
```

## Architecture

PgBlade adds 4 crates to Zed's workspace:

| Crate | Lines | Purpose |
|-------|-------|---------|
| `database_panel` | ~3,000 | Zed panels: ConnectionPanel (left), ResultPanel (bottom), DatabaseStatusItem (status bar) |
| `pgblade-core` | ~1,500 | Domain types, SQLite storage, SQL classifier, schema types |
| `pgblade-postgres` | ~500 | Async PostgreSQL driver, introspection, error mapping |
| `pgblade-security` | ~70 | macOS Keychain integration |

Total: ~5,000 lines of database-specific code on top of Zed's 2M+ codebase.

## License

PgBlade is a fork of Zed. The fork inherits Zed's GPL-3.0-or-later license.
The pgblade-core, pgblade-postgres, and pgblade-security crates are Apache-2.0 licensed.
