use std::io::Write;
use std::net::TcpStream;

const PATH: &str = "/get_sim_status?object=simStatus";

pub fn check_simulation_status(host: &str) -> bool {
    let host_uri = format!("{}:32034", host);
    println!("{}", host_uri);

    let mut stream = match TcpStream::connect(host_uri) {
        Ok(s) => s,
        Err(e) => {
            println!("Failed to connect to iRacing sim client: {}", e);
            return false;
        }
    };

    // Raw HTTP request string
    let http_request = format!(
        "{} {} {}\r\nHost: {}\r\nConnection: close\r\n\r\n",
        "GET", PATH, "HTTP/1.1", host
    );

    // Write the request to the stream
    if let Err(e) = stream.write_all(http_request.as_bytes()) {
        println!("Failed to send request: {}", e);
        return false;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status() {
        assert!(check_simulation_status("192.168.5.126"));
    }
}
