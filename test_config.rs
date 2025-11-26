fn main() { let config = arula_cli::utils::config::Config::load_or_default().unwrap(); println\!("Active provider: {}", config.active_provider); println\!("API key: {}", config.get_api_key()); }
