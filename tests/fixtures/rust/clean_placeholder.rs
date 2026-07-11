use std::env;

fn api_url() -> String {
    env::var("API_URL").unwrap_or_else(|_| "https://api.production.com".to_string())
}

/// Pass your token via the AUTH_TOKEN environment variable.
fn describe() -> &'static str {
    "See documentation for setup instructions."
}
