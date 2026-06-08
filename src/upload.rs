// Upload benchmark results to paste.rs and return the URL.

pub fn upload(json: &str) -> Result<String, String> {
    let resp = ureq::post("https://paste.rs/")
        .set("Content-Type", "application/json")
        .send_bytes(json.as_bytes())
        .map_err(|e| e.to_string())?;
    resp.into_string()
        .map(|s| s.trim().to_string())
        .map_err(|e| e.to_string())
}
