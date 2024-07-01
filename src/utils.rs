use worker::*;

pub use console_error_panic_hook::set_once as set_panic_hook;

pub fn json_response<T: serde::Serialize>(data: &T) -> Result<Response> {
    Response::from_json(data)
}

pub fn parse_query_string(url: &Url) -> std::collections::HashMap<String, String> {
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

pub fn generate_unique_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:x}", rng.gen::<u128>())
}