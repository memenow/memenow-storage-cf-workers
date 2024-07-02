use worker::*;

pub fn handle_cors_preflight() -> Result<Response> {
    let mut headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;
    headers.set("Access-Control-Max-Age", "86400")?;
    Ok(Response::ok("").unwrap().with_headers(headers))
}

pub fn add_cors_headers(mut res: Response) -> Response {
    res.headers_mut().set("Access-Control-Allow-Origin", "*").unwrap();
    res.headers_mut().set("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS").unwrap();
    res.headers_mut().set("Access-Control-Allow-Headers", "Content-Type").unwrap();
    res
}