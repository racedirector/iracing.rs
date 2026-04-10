use std::sync::Arc;

use iracing_sdk::{Result, VariableSchema, WindowsConnection, yaml_utils};
use tracing::{debug, info};

use crate::Provider;

pub struct LiveProvider {
    connection: WindowsConnection,
    schema: Arc<VariableSchema>,
}

impl LiveProvider {
    pub fn new() -> Result<Self> {
        let connection = WindowsConnection::try_connect()?;

        Self::with_connection(connection)
    }

    pub fn with_connection(connection: WindowsConnection) -> Result<Self> {
        let header = connection.header();
        let variables = connection.get_variables();
        let mut variable_map = std::collections::HashMap::new();

        for var_info in variables {
            variable_map.insert(var_info.name.clone(), var_info);
        }

        let frame_size = header.buf_len as usize;
        let schema = Arc::new(VariableSchema::new(variable_map, frame_size)?);

        Ok(Self { connection, schema })
    }

    /// Get the variable schema
    pub fn schema(&self) -> Arc<VariableSchema> {
        Arc::clone(&self.schema)
    }
}

impl Provider for LiveProvider {
    fn next_frame(&mut self) -> Result<Option<crate::FramePacket>> {
        Ok(None)
    }

    fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
        debug!("Fetching session YAML from shared memory");

        // Get raw YAML from shared memory
        let raw_yaml = match self.connection.session_info() {
            Some(yaml) => yaml,
            None => {
                debug!("No session info available");
                return Ok(None);
            }
        };

        // Return None if empty
        if raw_yaml.trim().is_empty() {
            return Ok(None);
        }

        // Preprocess to fix iRacing's YAML issues
        let cleaned_yaml = yaml_utils::preprocess_iracing_yaml(raw_yaml)?;

        info!("Extracted session YAML ({} bytes)", cleaned_yaml.len());

        Ok(Some(cleaned_yaml))
    }
}
