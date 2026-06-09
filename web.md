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
               "kernel": "6.0", "os": "Arch Linux" },
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
- `id` is a short, URL-safe, unguessable string (e.g. 12 hex chars or a UUIDv4).
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
  id                  TEXT PRIMARY KEY,
  created_at          TEXT NOT NULL,           -- ISO-8601 UTC
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
4. **Handlers** per the contract. For `POST`: parse JSON → `id = uuid`/short hex
   → insert raw + summary → return `{id,url}`. For the HTML pages, render a clean
   leaderboard table (rank, CPU, cores, OS, MT score, ST score, Triad, when) and a
   single-result page that also shows a "compare on the leaderboard" link.
5. **Logging**: `tower_http::trace::TraceLayer`.

Keep the HTML minimal, dark-themed, and dependency-light (one inline `<style>`).
Match the CRUCIBLE ember palette (`#e0552b` accent on a `#0e1014` background).

### Acceptance test (the agent must run this)

```sh
cargo run &                                  # starts on 127.0.0.1:8787
curl -s localhost:8787/healthz               # -> ok
curl -s -X POST localhost:8787/api/results \
     -H 'content-type: application/json' \
     --data '{"sysinfo":{"cpu_model":"Demo CPU","logical_cores":8,"ram_mib":16000,"kernel":"6.0","os":"Arch"},"cpu":{"composite_st":1200,"composite_mt":7000,"speedup":5.8},"mem":{"triad_gbs":30.5},"net":{"download_mbps":{"Ok":300.0}},"disk":{"seq_write_mbs":1500}}'
# -> {"id":"…","url":"https://crux.mmzsigmond.me/r/…"}
curl -s 'localhost:8787/api/results?limit=10'   # -> array containing the row
curl -s localhost:8787/ | head                  # -> leaderboard HTML
```

All four must pass before you consider the server done.

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
- [ ] `GET /` renders a leaderboard ranked by `composite_mt` desc.
- [ ] `GET /r/:id` renders a single result.
- [ ] Service survives reboot (systemd) and is reachable through nginx on `:80`.
- [ ] Running `crux` on a real machine makes it appear on the leaderboard
      (end-to-end smoke test).

Optional niceties (only after the above): per-CPU-model best-of view, a
client-supplied nonce to dedupe re-submits, and a JSON download link on each
result page.
