use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub has_system_proxy: bool,
    pub signal_strength: i32,
    pub network_type: String,
    pub dns_error: bool,
    pub check_error: Option<String>,
}

pub async fn check_network(native: &dyn crate::Native) -> NetworkInfo {
    match native.check_network().await {
        Ok(info) => info,
        Err(e) => NetworkInfo {
            has_system_proxy: false,
            signal_strength: -999,
            network_type: "unknown".to_string(),
            dns_error: true,
            check_error: Some(e.to_string()),
        }
    }
}

pub async fn check_dns(native: &dyn crate::Native, domain: &str) -> bool {
    match native.check_dns(domain).await {
        Ok(true) => true,
        _ => false
    }
} 