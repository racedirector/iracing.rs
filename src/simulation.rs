use std::io::{Read, Write};
use std::net::TcpStream;

///
/// Simulation instance.
///
/// Represents a simulation instance on a host.
///
/// # Examples
///
/// ```
/// use iracing::simuation::Simulation
///
/// let local = Simulation { host: "127.0.0.1".to_string() }
/// let remote = Simulation { host: "192.168.5.125".to_string() }
/// ```
#[derive(Debug, Clone)]
pub struct Simulation {
    pub host: String,
}

impl Simulation {
    /// The default port the iRacing simulation runs on.
    pub const PORT: u16 = 32034;

    /// The default path to retrieve sim status
    pub const SIM_STATUS_PATH: &str = "/get_sim_status?object=simStatus";

    pub fn host_uri(&self) -> String {
        format!("{}:{}", self.host, Self::PORT)
    }

    pub fn is_connected(&self) -> bool {
        self.check_status()
    }

    ///
    /// Checks if the sim is running
    ///
    /// Makes a request to {self.host}:{PORT}/{SIM_STATUS_PATH} to retrieve
    /// the sim status and returns true if connected, false otherwise.
    pub fn check_status(&self) -> bool {
        let mut stream = match TcpStream::connect(self.host_uri()) {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to connect to iRacing sim client: {}", e);
                return false;
            }
        };

        // Raw HTTP request string
        let http_request = format!(
            "{} {} {}\r\nHost: {}\r\nConnection: close\r\n\r\n",
            "GET",
            Simulation::SIM_STATUS_PATH,
            "HTTP/1.1",
            self.host
        );

        // Write the request to the stream
        if let Err(e) = stream.write_all(http_request.as_bytes()) {
            println!("Failed to send request: {}", e);
            return false;
        }

        let mut response = String::new();
        if let Err(e) = stream.read_to_string(&mut response) {
            println!("Failed to read response: {}", e);
            return false;
        }

        response.contains("running:1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status() {
        let sim = Simulation {
            host: "127.0.0.1".to_string(),
        };

        assert!(sim.check_status())
    }
}
