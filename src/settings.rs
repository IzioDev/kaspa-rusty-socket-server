use kaspa_wrpc_client::prelude::NetworkId;

pub struct AppSettings {
    pub ws_host: String,
    pub ws_port: u16,
    pub ws_path: String,
    pub network_id: NetworkId,
    pub max_clients: usize,
}

fn adapt_network_name_to_network_id(network_name: String) -> NetworkId {
    return match network_name.as_str() {
        "mainnet" => NetworkId::new(kaspa_wrpc_client::prelude::NetworkType::Mainnet),
        "tn11" => NetworkId::with_suffix(kaspa_wrpc_client::prelude::NetworkType::Testnet, 11),
        "tn10" => NetworkId::with_suffix(kaspa_wrpc_client::prelude::NetworkType::Testnet, 10),
        _ => NetworkId::new(kaspa_wrpc_client::prelude::NetworkType::Mainnet),
    };
}

impl AppSettings {
    pub fn new() -> Self {
        Self {
            ws_host: std::env::var("WS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            ws_port: std::env::var("WS_PORT")
                .unwrap_or_else(|_| "3002".to_string())
                .parse()
                .expect("WS_PORT must be a number"),
            ws_path: std::env::var("WS_PATH").unwrap_or_else(|_| "/ws".to_string()),
            network_id: adapt_network_name_to_network_id(
                std::env::var("NETWORK_NAME").unwrap_or_else(|_| "mainnet".to_string()),
            ),
            max_clients: std::env::var("MAX_CLIENTS")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .expect("MAX_CLIENTS must be a number"),
        }
    }
}
