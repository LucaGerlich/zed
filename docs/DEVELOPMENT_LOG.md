# PgBlade Development Log

## Project History

PgBlade started as a standalone Rust/GPUI application (pgblade-standalone), then pivoted to a Zed editor fork for access to Zed's mature editor, git, search, themes, and UI infrastructure.

### Phase 1: Standalone App (pgblade-standalone, archived)
- M1: GPUI window shell, 5-crate workspace, domain types
- M2: PostgreSQL engine (tokio-postgres, async driver, integration tests)
- M3: Query pad (TextInput, connection modal, result grid)
- M4: Workbench MVP (schema browser, saved connections, keychain, SQL classifier)
- M5: Public alpha polish (command palette, tabs, syntax highlighting)
- M6: Editor overhaul (ropey buffer, selection model, undo, mouse support)
- Decision: forked Zed instead of continuing standalone

### Phase 2: Zed Fork (current, 45 commits)

#### Foundation (commits 1-4)
- Forked zed-industries/zed
- Copied pgblade-core, pgblade-postgres, pgblade-security into Zed workspace
- Created database_panel crate with ConnectionPanel (left dock) and ResultPanel (bottom dock)
- Registered in Zed's main.rs init

#### Core Database IDE (commits 5-10)
- Connection form with Zed Editor inputs (host, port, db, user, password)
- Execute Query (Cmd+Enter) reading from active editor buffer
- Result grid with column headers, row numbers, timing
- DBeaver-style schema tree (Database > Schemas > Tables/Views/Functions/Sequences)
- Per-table sub-nodes: Columns, Constraints, Foreign Keys, Indexes, Triggers
- Table preview on double-click

#### Export & Analysis (commits 11-15)
- CSV, JSON, INSERT, UPDATE, DELETE export (clipboard + file)
- EXPLAIN ANALYZE with plan tree visualization
- View Sessions (pg_stat_activity), Kill Backend
- SQL formatting via sqlformat crate
- Generate SELECT/INSERT/UPDATE from schema tree

#### DBA Tooling (commits 16-25)
- 30+ system query actions: Database Info, Table Sizes, Index Usage, Unused/Missing/Duplicate Indexes
- Table Bloat, Vacuum Progress, Autovacuum Status, Maintenance Recommendations
- View Locks, Slow Queries, Active Queries, Cache Hit Ratio
- WAL Status, Replication, Tablespaces, Roles, Privileges, Settings
- Connection Stats, Connection Limits, pg_stat_statements, Table Access Stats
- Column Stats, View Dependencies, Partitions, Foreign Tables, Extensions

#### Editor Integration (commits 26-30)
- Inline SQL auto-completion via CompletionProvider trait (merges with LSP)
- Schema-aware completions: tables, columns, functions, keywords
- Execute selected text only (highlight + Cmd+Enter)
- Column sorting (click header), row selection + edit/delete generation
- Multiple result tabs with smart labels

#### UI Polish (commits 31-38)
- Zed native components: ListItem, Icon, Label, Button, IconButton throughout
- Schema tree with disclosure triangles and proper icons
- Quick action toolbar (Sessions, Locks, Sizes, Slow, Bloat)
- Load More pagination (500 rows, expandable)
- Column filtering (type to filter results)
- Right-click context menu on connections (Connect/Edit/Delete)
- Single-click expand, double-click preview
- Connection test button, uptime display

#### Features & Templates (commits 39-43)
- SSH tunnel support (system ssh binary, agent + key file auth)
- SSL mode selector (Disable/Prefer/Require)
- Query bookmarks (save/view from command palette)
- SQL snippets: CTE, Window Function, Upsert, Pivot, Recursive CTE, JSON, LATERAL JOIN
- CREATE FUNCTION/TRIGGER templates, ALTER TABLE templates, GRANT/REVOKE
- ER Diagram (Mermaid), Schema DDL Dump, CSV Import & Execute
- Database Health Check (11 metrics in one query)

#### Stabilization (commits 44-45)
- Audit: 13 panic fixes (unchecked indexing), 8 SQL injection hardenings
- Unicode-safe truncation, large cell value handling
- Clippy clean (zero warnings)
- Status bar connection indicator (database@host + uptime)
- PGBLADE.md documentation, icon suggestion

---

## Current Stats (as of commit 45)
- **45 commits** on the Zed fork
- **~21K lines** of database-specific code
- **65+ command palette actions**
- **4 crates**: database_panel (~3K), pgblade-core (~1.5K), pgblade-postgres (~500), pgblade-security (~70)
- **Zero clippy warnings**, all builds clean

---

## What's Built Next (Phase 3)

### Batch 1: Smart Integrations
- [ ] `.env` file integration — auto-detect DATABASE_URL from project .env files
- [ ] Connection health ping — background check every 30s, yellow warning on drop
- [ ] Query timeout configuration — set statement_timeout per connection
- [ ] Schema change detection — detect new/dropped objects, offer refresh

### Batch 2: AI & Intelligence
- [ ] AI SQL assistant — explain query, suggest optimizations via Zed's Claude
- [ ] SQL linting — warn about SELECT *, missing WHERE on DELETE, etc.
- [ ] Query plan comparison — diff EXPLAIN before/after changes

### Batch 3: Multi-Connection
- [ ] Multiple simultaneous connections — switch between dev/staging/prod
- [ ] Custom color per connection — red for production, green for dev
- [ ] Query result diffing — same query on two connections, show differences

### Batch 4: Unique Zed Features
- [ ] SQL in any file — detect SQL strings in .py, .ts, .rs, execute inline
- [ ] Migration file support — detect migration files, run up/down
- [ ] Git-aware schema tracking — show tables changed in current git diff
- [ ] Data masking — auto-mask sensitive columns on production connections

### Batch 5: Monitoring
- [ ] Slow query alerts — background monitor, notification on threshold
- [ ] Result set pinning — keep a result tab from being replaced
- [ ] Saved query folders — organize bookmarks by project
