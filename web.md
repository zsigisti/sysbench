# CRUCIBLE Score Server — build runbook (for an AI coding agent)

You are an autonomous coding agent (e.g. Claude Code). Your job: stand up the
**CRUCIBLE score server** that the `crux` CLI and `crux-gui` submit benchmark
results to, so machines can be ranked and compared at
**https://crux.mmzsigmond.me**.

Build it exactly to the **API contract** below — the clients are already shipping
and depend on it. Stack is fixed: **Rust + axum + SQLite**, reverse-proxied by
**nginx**. TLS / public exposure is handled by **Cloudflare Zero Trust** by the
operator — you only need nginx listening on `127.0.0.1:8787` (or a local port)
and serving plain HTTP; do **not** configure certificates.

When you finish, the operator should be able to run `crux` on any machine and see
it appear on the leaderboard.

---

## 0. Context you need

The client posts the **full results JSON** produced by `crux`. Its shape (only
the fields you must read are shown; ignore unknown fields, and treat every
metric as optional — partial runs happen):

```jsonc
{
  "sysinfo": { "cpu_model": "…", "logical_cores": 16, "ram_mib": 32000,
               "kernel": "6.0", "os": "Arch Linux",
               "machine_id": "a1b2c3d4e5f6" },
  "cpu":  { "composite_st": 1300, "composite_mt": 12000, "speedup": 8.9 },
  "mem":  { "triad_gbs": 45.0 },
  "net":  { "download_mbps": {"Ok": 300.0}, "upload_mbps": {"Ok": 55.0},
            "latency": {"Ok": {"avg_ms": 9.8}} },
  "disk": { "seq_write_mbs": 3000, "seq_read_mbs": 3500 }
}
```

Note `net.*` values are serde-`Result`-encoded: read `["net"]["download_mbps"]["Ok"]`.

The leaderboard ranks by **`cpu.composite_mt`** (multi-threaded composite score),
descending.

### One row per machine (`sysinfo.machine_id`)

`sysinfo.machine_id` is a **stable, anonymized 12-hex identifier** that is the
same on every submission from a given machine. **Use it as the result's primary
key** so a machine that benchmarks repeatedly updates its single row instead of
spawning a new one each time:

- Derive the public `id` deterministically from it: `id = machine_id` (it is
  already a short, opaque hash — do not re-hash). This makes each machine's
  result URL (`/r/<id>`) stable and shareable.
- On `POST`, **upsert and merge**: insert if new, otherwise update the row —
  identity fields (`cpu_model`, `os`, …), `raw`, and `created_at` take the latest
  submission, while each metric keeps its prior value when the new submission
  didn't measure it (so partial runs accumulate; see the `COALESCE` SQL in §2).
- Fallback: if `machine_id` is absent (older client), generate a random 12-hex
  `id` as before so the insert still succeeds.

This is also why partial runs look fragmented today: a mem-only and a net-only
submission from the same box became two separate rows. With machine-id upsert, a
full `crux submit` collapses them into one complete, ranked row.

---

## 1. API contract (do not deviate)

| Method | Path | Body | Response |
|--------|------|------|----------|
| `POST` | `/api/results` | the full results JSON | `201` + `{"id":"<id>","url":"https://crux.mmzsigmond.me/r/<id>"}` |
| `GET`  | `/api/results?limit=50&sort=mt` | — | `200` + JSON array of leaderboard rows |
| `GET`  | `/r/:id` | — | `200` HTML page for one result |
| `GET`  | `/` | — | `200` HTML leaderboard page |
| `GET`  | `/healthz` | — | `200` `ok` |

Rules:
- `id` is a short, URL-safe string derived from `sysinfo.machine_id` (12 hex
  chars); fall back to a random 12-hex string only when `machine_id` is absent.
- `POST` is **idempotent per machine**: re-submitting from the same machine
  updates its existing row (same `id`/`url`), it does not create a duplicate.
- `POST` must store the **raw JSON** and the extracted summary columns.
- The `url` you return MUST use the public base (`PUBLIC_BASE` env, default
  `https://crux.mmzsigmond.me`), not the bind address.
- Be liberal in what you accept: missing metrics → store `NULL`, never `500`.
- Respond `400` only on malformed JSON.

A leaderboard row (`GET /api/results`) is:

```json
{ "id": "ab12cd34ef56", "when": "2026-06-09T10:21:00Z", "cpu_model": "…",
  "cores": 16, "ram_mib": 32000, "os": "…",
  "composite_mt": 12000, "composite_st": 1300, "speedup": 8.9,
  "mem_triad_gbs": 45.0, "net_down_mbps": 300.0, "disk_seq_write_mbs": 3000 }
```

---

## 2. Database schema (`schema.sql`)

```sql
CREATE TABLE IF NOT EXISTS results (
  id                  TEXT PRIMARY KEY,        -- = sysinfo.machine_id (stable per machine)
  created_at          TEXT NOT NULL,           -- ISO-8601 UTC, of the latest submission
  raw                 TEXT NOT NULL,           -- the full posted JSON
  cpu_model           TEXT,
  cores               INTEGER,
  ram_mib             INTEGER,
  os                  TEXT,
  kernel              TEXT,
  composite_mt        REAL,
  composite_st        REAL,
  speedup             REAL,
  mem_triad_gbs       REAL,
  net_down_mbps       REAL,
  net_up_mbps         REAL,
  net_latency_ms      REAL,
  disk_seq_write_mbs  REAL,
  disk_seq_read_mbs   REAL
);
CREATE INDEX IF NOT EXISTS idx_results_mt ON results(composite_mt DESC);
```

Insert with an upsert that **merges** metrics so a machine ends up with one
complete row even when it submits one suite at a time. Identity fields take the
latest value; each metric keeps its existing value when the new submission omits
it (`COALESCE(excluded.x, x)`) — a mem-only run then a net-only run accumulate
into a single row instead of overwriting each other:

```sql
INSERT INTO results (id, created_at, raw, cpu_model, cores, ram_mib, os, kernel,
  composite_mt, composite_st, speedup, mem_triad_gbs, net_down_mbps, net_up_mbps,
  net_latency_ms, disk_seq_write_mbs, disk_seq_read_mbs)
VALUES (?1, ?2, ?3, ?4, …)
ON CONFLICT(id) DO UPDATE SET
  -- identity / freshness: always take the latest submission
  created_at = excluded.created_at,
  raw        = excluded.raw,
  cpu_model  = excluded.cpu_model,
  cores      = excluded.cores,
  ram_mib    = excluded.ram_mib,
  os         = excluded.os,
  kernel     = excluded.kernel,
  -- metrics: keep the prior value when this submission didn't measure it
  composite_mt       = COALESCE(excluded.composite_mt, composite_mt),
  composite_st       = COALESCE(excluded.composite_st, composite_st),
  speedup            = COALESCE(excluded.speedup, speedup),
  mem_triad_gbs      = COALESCE(excluded.mem_triad_gbs, mem_triad_gbs),
  net_down_mbps      = COALESCE(excluded.net_down_mbps, net_down_mbps),
  net_up_mbps        = COALESCE(excluded.net_up_mbps, net_up_mbps),
  net_latency_ms     = COALESCE(excluded.net_latency_ms, net_latency_ms),
  disk_seq_write_mbs = COALESCE(excluded.disk_seq_write_mbs, disk_seq_write_mbs),
  disk_seq_read_mbs  = COALESCE(excluded.disk_seq_read_mbs, disk_seq_read_mbs);
```

(A full `crux submit` measures everything at once, so it fills the whole row in
one go; the COALESCE merge just makes partial runs behave sanely too.)

---

## 3. Rust project

Create `crux-server/` with this `Cargo.toml`:

```toml
[package]
name = "crux-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
tower-http = { version = "0.5", features = ["trace"] }
rusqlite = { version = "0.31", features = ["bundled"] }   # bundled = no system sqlite
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
time = { version = "0.3", features = ["formatting", "macros"] }
askama = "0.12"          # or build HTML by hand; templates live in templates/
```

Implement (`src/main.rs`, split into modules as it grows):

1. **State**: an `Arc<Mutex<rusqlite::Connection>>` (SQLite + low write volume →
   a single guarded connection is fine). On boot, run `schema.sql`.
2. **Config from env**: `CRUX_DB` (default `crux.db`), `CRUX_BIND`
   (default `127.0.0.1:8787`), `PUBLIC_BASE` (default
   `https://crux.mmzsigmond.me`).
3. **Extraction helper**: a `fn summarize(v: &serde_json::Value) -> Summary`
   that pulls the columns above using the JSON paths in §0 (mirror the client's
   `summary.rs` logic — note the `["Ok"]` nesting for net values).
4. **Handlers** per the contract. For `POST`: parse JSON → `id =
   sysinfo.machine_id` (random 12-hex fallback) → **upsert** raw + summary →
   return `{id,url}`. For the HTML pages, render the leaderboard and single-result
   pages to the **UI spec in §3a** below.
5. **Logging**: `tower_http::trace::TraceLayer`.

Keep the HTML dependency-light (one inline `<style>`, no JS frameworks; a few
lines of vanilla JS for the copy-link button is fine). Dark, dependency-light,
and on the CRUCIBLE ember palette (`#e0552b` accent on a `#0e1014` background).

### Acceptance test (the agent must run this)

```sh
cargo run &                                  # starts on 127.0.0.1:8787
curl -s localhost:8787/healthz               # -> ok
# Submit twice with the SAME machine_id — must NOT create two rows:
curl -s -X POST localhost:8787/api/results -H 'content-type: application/json' \
     --data '{"sysinfo":{"cpu_model":"Demo CPU","logical_cores":8,"ram_mib":16000,"kernel":"6.0","os":"Arch","machine_id":"deadbeef0001"},"cpu":{"composite_st":1200,"composite_mt":7000,"speedup":5.8},"mem":{"triad_gbs":30.5},"net":{"download_mbps":{"Ok":300.0}},"disk":{"seq_write_mbs":1500}}'
# -> {"id":"deadbeef0001","url":"https://crux.mmzsigmond.me/r/deadbeef0001"}
curl -s -X POST localhost:8787/api/results -H 'content-type: application/json' \
     --data '{"sysinfo":{"cpu_model":"Demo CPU","logical_cores":8,"ram_mib":16000,"kernel":"6.0","os":"Arch","machine_id":"deadbeef0001"},"cpu":{"composite_st":1300,"composite_mt":7500,"speedup":5.8},"mem":{"triad_gbs":31.0}}'
# -> same id deadbeef0001 (row UPDATED, mt now 7500)
curl -s 'localhost:8787/api/results?limit=10'   # -> array with exactly ONE row, composite_mt=7500
curl -s localhost:8787/ | head                  # -> leaderboard HTML
```

All four must pass — in particular the array must contain **one** row, not two,
and its `composite_mt` must be the updated value.

---

## 3a. Leaderboard UI (make it look good)

The leaderboard is the public face of CRUCIBLE — invest in it. Render server-side
HTML (no SPA). Requirements:

**Layout & chrome**
- Centered column, `max-width: 1100px`, generous padding, ember accent on dark.
- Header: `🔥 CRUCIBLE` wordmark, a one-line tagline ("Trial by fire — rank your
  machine"), and a small stat strip: total machines ranked, fastest MT score.
- Sticky table header so columns stay labelled when scrolling.
- Mobile: the table scrolls horizontally inside a rounded container; never let
  columns squish into unreadable wraps (`overflow-x:auto`, `white-space:nowrap`).

**The table**
- Columns: **Rank · Machine · Cores · RAM · OS · MT · ST · Speedup · Triad ·
  Disk W · Net ↓ · When**.
- **Rank**: top 3 get medals (🥇🥈🥉) and a subtly highlighted row; others show
  `#4`, `#5`, … in muted grey.
- **Machine**: CPU model in full-weight text; if it's long, allow it to be the
  one column that wraps. Whole row is a link to `/r/<id>`.
- **MT** is the hero column: large, bold, ember. Render a thin horizontal bar
  behind/under the number sized as `value / top_mt` (a pure-CSS mini bar, e.g. a
  `linear-gradient` background or a nested div) so ranking is visible at a glance.
- Missing metrics render as a muted `—`, never blank or `null`.
- Format numbers: scores as integers with thousands separators; GB/s and Mbps to
  one decimal; relative time for "When" ("2h ago", "3d ago") with the absolute
  date in a `title=` tooltip.
- Zebra/hover row shading for readability.

**Empty state**
- When there are no rows, show a friendly card: "No machines ranked yet — be the
  first: `curl -fsSL https://crux.mmzsigmond.me/install | sh` then `crux submit`."
  (Use the project's real install one-liner if known; otherwise just `crux submit`.)

**Single-result page (`/r/:id`)**
- A big score card at top: CPU model, MT score (hero), ST, speedup.
- A clean key/value grid of every metric (CPU composite ST/MT, memory triad,
  disk read/write, net down/up/latency, kernel, OS, RAM, cores).
- This machine's **current rank** ("#3 of 27") and a "← back to leaderboard" link.
- A "Copy result link" button (tiny inline JS) and a "Download JSON" link that
  serves the stored `raw`.
- `<title>` and OpenGraph tags ("CRUCIBLE — <cpu> — MT <score>") so shared links
  preview nicely.

**Polish**
- Use `font-variant-numeric: tabular-nums` so score columns align.
- One cohesive palette: bg `#0e1014`, surface `#13161c`, border `#2a2d33`, text
  `#c9cdd4`, muted `#6b7280`, accent `#e0552b`. Round corners (8px), subtle
  borders, no harsh shadows.

---

## 3b. Clearing submissions (operator reset)

The database currently holds throwaway test rows (fragmented partial runs). Wipe
them so the public board starts clean. Provide the operator these steps and run
the wipe yourself as part of bring-up:

```sh
# Stop the service so the DB isn't being written during the wipe
sudo systemctl stop crux-server
# Option A — clear rows, keep the DB/schema:
sudo sqlite3 /var/lib/crux/crux.db 'DELETE FROM results; VACUUM;'
# Option B — nuke the DB entirely (schema is recreated on boot):
sudo rm -f /var/lib/crux/crux.db
sudo systemctl start crux-server
curl -s -H 'Host: crux.mmzsigmond.me' localhost/api/results   # -> []
```

Do **not** expose a public "delete all" HTTP endpoint. Resetting is an operator
action over SSH only.

---

## 4. systemd unit (`/etc/systemd/system/crux-server.service`)

```ini
[Unit]
Description=CRUCIBLE score server
After=network.target

[Service]
Environment=CRUX_BIND=127.0.0.1:8787
Environment=CRUX_DB=/var/lib/crux/crux.db
Environment=PUBLIC_BASE=https://crux.mmzsigmond.me
ExecStart=/usr/local/bin/crux-server
Restart=on-failure
DynamicUser=yes
StateDirectory=crux

[Install]
WantedBy=multi-user.target
```

Build release, install the binary, enable the service:

```sh
cargo build --release
sudo install -m755 target/release/crux-server /usr/local/bin/crux-server
sudo systemctl enable --now crux-server
```

---

## 5. nginx (plain HTTP — Cloudflare terminates TLS)

`/etc/nginx/sites-available/crux.conf`:

```nginx
server {
    listen 80;
    server_name crux.mmzsigmond.me;

    location / {
        proxy_pass http://127.0.0.1:8787;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

```sh
sudo ln -s /etc/nginx/sites-available/crux.conf /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

The operator points Cloudflare Zero Trust (tunnel) at this nginx vhost; you do
**not** manage DNS or certificates.

---

## 6. Done criteria

- [ ] `POST /api/results` stores raw + summary and returns `{id,url}` with the
      public base URL.
- [ ] **`id` is derived from `sysinfo.machine_id` and `POST` upserts** — two
      submissions from the same machine yield one row, not two (see §3a test).
- [ ] `GET /` renders the leaderboard to the **§3a UI spec** (medals, hero MT
      column with relative bar, mobile scroll, empty state), ranked by
      `composite_mt` desc.
- [ ] `GET /r/:id` renders a single result with the metric grid, current rank,
      copy-link, and JSON download.
- [ ] Existing throwaway rows have been **cleared** (§3b) so the board starts clean.
- [ ] Service survives reboot (systemd) and is reachable through nginx on `:80`.
- [ ] Running `crux` on a real machine makes it appear on the leaderboard
      (end-to-end smoke test).

Optional niceties (only after the above): per-CPU-model best-of view, search /
filter by CPU, and an OpenGraph image per result.
