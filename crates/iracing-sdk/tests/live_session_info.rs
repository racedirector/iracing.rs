#![cfg(windows)]

use iracing_sdk::{schema::SessionInfo, windows::Connection};

#[test]
#[ignore = "requires iRacing running in an active session"]
fn parses_live_iracing_session_info() {
    let connection = Connection::try_connect().expect("connect to running iRacing");
    let buffer = connection
        .session_info_buffer()
        .expect("active session should expose a session buffer");
    let session = SessionInfo::try_from(buffer).expect("live session should deserialize");
    assert!(!session.weekend_info.track_name.is_empty());
    assert!(!session.weekend_info.track_display_name.is_empty());
    assert!(!session.session_info.sessions.is_empty());
}
