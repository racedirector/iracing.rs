use serde::{Deserialize, Serialize};

/// Complete server settings edited by the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    pub general: ServerGeneralSettings,
    pub http: TransportSettings,
    pub websocket: TransportSettings,
    pub grpc: TransportSettings,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            general: ServerGeneralSettings::default(),
            http: TransportSettings {
                host: "127.0.0.1".to_string(),
                port: 32080,
            },
            websocket: TransportSettings {
                host: "127.0.0.1".to_string(),
                port: 32081,
            },
            grpc: TransportSettings {
                host: "127.0.0.1".to_string(),
                port: 32082,
            },
        }
    }
}

impl ServerSettings {
    pub(super) fn validate(&self) -> Result<(), String> {
        tracing::debug!(settings = ?self, "validating server settings");
        self.http.validate("HTTP")?;
        self.websocket.validate("WebSocket")?;
        self.grpc.validate("gRPC")?;
        tracing::debug!("server settings validation succeeded");
        Ok(())
    }
}

/// Feature flags for each local transport.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerGeneralSettings {
    pub http_enabled: bool,
    pub websocket_enabled: bool,
    pub grpc_enabled: bool,
}

/// Bind settings shared by all transports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportSettings {
    pub host: String,
    pub port: u16,
}

impl TransportSettings {
    pub(super) fn validate(&self, label: &str) -> Result<(), String> {
        if self.host.trim().is_empty() {
            tracing::debug!(
                transport = label,
                "transport validation failed: host is required"
            );
            return Err(format!("{label} host is required."));
        }

        if self.port == 0 {
            tracing::debug!(
                transport = label,
                "transport validation failed: port is zero"
            );
            return Err(format!("{label} port must be between 1 and 65535."));
        }

        tracing::debug!(
            transport = label,
            host = %self.host,
            port = self.port,
            "transport settings validation succeeded"
        );
        Ok(())
    }
}

/// Settings plus live runtime status for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerState {
    pub settings: ServerSettings,
    pub status: ServerRuntimeStatus,
}

/// Runtime status for every transport.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRuntimeStatus {
    pub http: TransportRuntimeStatus,
    pub websocket: TransportRuntimeStatus,
    pub grpc: TransportRuntimeStatus,
}

/// UI-friendly transport status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TransportRuntimeStatus {
    Disabled,
    Running { endpoint: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_keep_all_transports_disabled() {
        let settings = ServerSettings::default();

        assert!(!settings.general.http_enabled);
        assert!(!settings.general.websocket_enabled);
        assert!(!settings.general.grpc_enabled);
        assert_eq!(settings.http.port, 32080);
        assert_eq!(settings.websocket.port, 32081);
        assert_eq!(settings.grpc.port, 32082);
    }

    #[test]
    fn transport_settings_require_a_host_and_non_zero_port() {
        let missing_host = TransportSettings {
            host: String::new(),
            port: 32080,
        };
        let missing_port = TransportSettings {
            host: "127.0.0.1".to_string(),
            port: 0,
        };

        assert!(missing_host.validate("HTTP").is_err());
        assert!(missing_port.validate("HTTP").is_err());
    }
}
