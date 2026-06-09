// Sharing benchmark results.
//
// Two targets:
//   * `submit`  — POST the full JSON to the CRUCIBLE score server (default
//                 https://crux.mmzsigmond.me) so it can be ranked/compared.
//   * `upload`  — POST to paste.rs (used as a fallback and for plain text).
//
// The server base URL can be overridden with the `CRUX_SERVER` env var so you
// can point the CLI/GUI at your own deployment without rebuilding.

/// Default CRUCIBLE score server. Override with `CRUX_SERVER`.
pub const DEFAULT_SERVER: &str = "https://crux.mmzsigmond.me";

/// Where a result ended up after sharing.
pub struct Shared {
    /// Human-facing URL to view the result.
    pub url: String,
    /// Which backend accepted it ("crux" or "paste.rs").
    pub backend: &'static str,
}

/// The configured server base URL (env override or the default), without a
/// trailing slash.
pub fn server_base() -> String {
    let raw = std::env::var("CRUX_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_string());
    raw.trim_end_matches('/').to_string()
}

/// Submit a full results JSON to the CRUCIBLE score server.
///
/// Expects the server to accept `POST {base}/api/results` with a JSON body and
/// reply with `{"id": "...", "url": "..."}` (see `web.md` for the contract).
/// Falls back to the `Location` header or a constructed `/r/{id}` URL.
pub fn submit(json: &str) -> Result<String, String> {
    let base = server_base();
    let endpoint = format!("{}/api/results", base);
    let resp = ureq::post(&endpoint)
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(20))
        .send_bytes(json.as_bytes())
        .map_err(|e| e.to_string())?;

    let body = resp.into_string().map_err(|e| e.to_string())?;
    // Try to parse the JSON envelope; fall back to using the raw body.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(url) = v.get("url").and_then(|u| u.as_str()) {
            return Ok(url.to_string());
        }
        if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
            return Ok(format!("{}/r/{}", base, id));
        }
    }
    let trimmed = body.trim();
    if trimmed.starts_with("http") {
        return Ok(trimmed.to_string());
    }
    Err(format!("server returned an unexpected response: {}", trimmed))
}

/// Share results: try the CRUCIBLE server first, fall back to paste.rs.
/// Returns the URL plus which backend accepted it.
pub fn share(json: &str) -> Result<Shared, String> {
    match submit(json) {
        Ok(url) => Ok(Shared { url, backend: "crux" }),
        Err(server_err) => match upload(json) {
            Ok(url) => Ok(Shared { url, backend: "paste.rs" }),
            Err(paste_err) => Err(format!(
                "server failed ({}); paste.rs fallback also failed ({})",
                server_err, paste_err
            )),
        },
    }
}

/// Upload arbitrary text/JSON to paste.rs and return the URL.
pub fn upload(json: &str) -> Result<String, String> {
    let resp = ureq::post("https://paste.rs/")
        .set("Content-Type", "application/json")
        .send_bytes(json.as_bytes())
        .map_err(|e| e.to_string())?;
    resp.into_string()
        .map(|s| s.trim().to_string())
        .map_err(|e| e.to_string())
}
