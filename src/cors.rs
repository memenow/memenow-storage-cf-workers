use worker::*;

pub fn handle_cors_preflight() -> Result<Response> {
    let mut headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;
    headers.set("Access-Control-Max-Age", "86400")?;
    Ok(Response::ok("").unwrap().with_headers(headers))
}

pub fn add_cors_headers(res: Response) -> Result<Response> {
    let mut headers = res.headers().clone();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;

    Ok(Response::from_body(res.body().clone())?.with_status(res.status_code()).with_headers(headers))
}