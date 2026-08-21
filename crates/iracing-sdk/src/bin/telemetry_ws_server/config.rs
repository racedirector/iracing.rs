use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::{
    env,
    fmt::{self, Display},
    net::SocketAddr,
    str::FromStr,
    time::Duration,
};

const ENVIRONMENT_ENV: &str = "TELEMETRY_WS_ENV";
const BIND_ADDR_ENV: &str = "TELEMETRY_WS_BIND_ADDR";
const SHUTDOWN_TIMEOUT_SECS_ENV: &str = "TELEMETRY_WS_SHUTDOWN_TIMEOUT_SECS";

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3000";
const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 10;

#[derive(Parser, Debug)]
#[command(version, about = "HTTP/WebSocket telemetry server", long_about = None)]
pub(crate) struct Cli {
    /// Runtime environment name. Overrides TELEMETRY_WS_ENV.
    #[arg(long)]
    pub(crate) environment: Option<Environment>,

    /// Address the HTTP/WebSocket server should bind to. Overrides TELEMETRY_WS_BIND_ADDR.
    #[arg(long)]
    pub(crate) bind_addr: Option<SocketAddr>,

    /// Graceful shutdown timeout in seconds. Overrides TELEMETRY_WS_SHUTDOWN_TIMEOUT_SECS.
    #[arg(long)]
    pub(crate) shutdown_timeout_secs: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Environment {
    #[default]
    Development,
    Test,
    Staging,
    Production,
}

impl Environment {
    pub(crate) fn default_bind_addr(self) -> &'static str {
        match self {
            Self::Development | Self::Test => DEFAULT_BIND_ADDR,
            Self::Staging | Self::Production => "0.0.0.0:3000",
        }
    }
}

impl Display for Environment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Staging => "staging",
            Self::Production => "production",
        };

        formatter.write_str(value)
    }
}

impl FromStr for Environment {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dev" | "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "stage" | "staging" => Ok(Self::Staging),
            "prod" | "production" => Ok(Self::Production),
            other => Err(anyhow!(
                "unsupported environment `{other}`; expected development, test, staging, or production"
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ServerConfig {
    pub(crate) environment: Environment,
    pub(crate) bind_addr: SocketAddr,
    pub(crate) shutdown_timeout: Duration,
}

impl ServerConfig {
    pub(crate) fn from_cli(cli: Cli) -> Result<Self> {
        let environment = match cli.environment {
            Some(environment) => environment,
            None => read_value(ENVIRONMENT_ENV)?
                .map(|value| value.parse())
                .transpose()
                .context("invalid TELEMETRY_WS_ENV")?
                .unwrap_or_default(),
        };

        let bind_addr = match cli.bind_addr {
            Some(addr) => addr,
            None => read_value(BIND_ADDR_ENV)?
                .map(|value| value.parse())
                .transpose()
                .with_context(|| format!("invalid {BIND_ADDR_ENV}"))?
                .unwrap_or_else(|| {
                    environment
                        .default_bind_addr()
                        .parse()
                        .expect("default bind address must be valid")
                }),
        };

        let shutdown_timeout_secs = cli
            .shutdown_timeout_secs
            .or(read_value(SHUTDOWN_TIMEOUT_SECS_ENV)?
                .map(|value| value.parse())
                .transpose()
                .with_context(|| format!("invalid {SHUTDOWN_TIMEOUT_SECS_ENV}"))?)
            .unwrap_or(DEFAULT_SHUTDOWN_TIMEOUT_SECS);

        Ok(Self {
            environment,
            bind_addr,
            shutdown_timeout: Duration::from_secs(shutdown_timeout_secs),
        })
    }
}

fn read_value(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_accepts_common_aliases() {
        assert_eq!(
            "dev".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "prod".parse::<Environment>().unwrap(),
            Environment::Production
        );
    }

    #[test]
    fn development_defaults_to_loopback() {
        let addr: SocketAddr = Environment::Development
            .default_bind_addr()
            .parse()
            .unwrap();

        assert_eq!(addr, SocketAddr::from(([127, 0, 0, 1], 3000)));
    }

    #[test]
    fn production_defaults_to_all_interfaces() {
        let addr: SocketAddr = Environment::Production.default_bind_addr().parse().unwrap();

        assert_eq!(addr, SocketAddr::from(([0, 0, 0, 0], 3000)));
    }
}
