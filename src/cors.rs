use worker::*;

pub fn handle_cors_preflight() -> Result<Response> {
    let mut headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS, PUT, DELETE")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type, X-Chunk-Index, Authorization")?;
    headers.set("Access-Control-Max-Age", "86400")?;
    Ok(Response::ok("").unwrap().with_headers(headers))
}

pub fn add_cors_headers(res: Response) -> Result<Response> {
    let mut res = res;
    res.headers_mut().set("Access-Control-Allow-Origin", "*")?;
    res.headers_mut().set("Access-Control-Allow-Methods", "GET, POST, OPTIONS, PUT, DELETE")?;
    res.headers_mut().set("Access-Control-Allow-Headers", "Content-Type, X-Chunk-Index, Authorization")?;
    Ok(res)
}