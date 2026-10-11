use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub app_home: String,
    pub log_level: String,
    pub local_api_listen: String,
    pub local_api_secret: String,
    pub pid_file: String,
    pub database_url: String,
    pub web_ws_url: String,
    pub web_client_id: String,
    pub web_client_secret: String,
    pub core_type: String,
    pub core_bin_path: String,
}

pub fn load() -> Config {
    if cfg!(debug_assertions) {
        dotenvy::dotenv().ok();
        println!("Loaded development environment .env configuration");
    }

    envy::from_env().expect("Failed to parse environment variables!")
}
