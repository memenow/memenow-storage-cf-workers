use worker::*;

pub fn handle_cors_preflight() -> Result<Response> {
    let mut headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS, DELETE")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;
    Ok(Response::ok("").unwrap().with_headers(headers))
}

pub fn add_cors_headers(mut res: Response) -> Result<Response> {
    res.headers_mut().set("Access-Control-Allow-Origin", "*")?;
    res.headers_mut().set("Access-Control-Allow-Methods", "GET, POST, OPTIONS, DELETE")?;
    res.headers_mut().set("Access-Control-Allow-Headers", "Content-Type")?;
    Ok(res)
}