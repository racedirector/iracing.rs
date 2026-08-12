#![cfg(windows)]

use iracing_sdk::{provider::Provider, providers::live::LiveProvider, schema::SessionInfo};

#[tokio::test]
#[ignore = "iracing_required"]
async fn parses_live_iracing_session_info() -> iracing_sdk::Result<()> {
    let mut provider = LiveProvider::new()?;
    let yaml = provider
        .session_yaml(0)
        .await?
        .expect("an active iRacing session should expose session YAML");
    let session = SessionInfo::parse_sanitized(&yaml)?;

    assert!(!session.weekend_info.track_name.is_empty());
    assert!(!session.weekend_info.track_display_name.is_empty());
    assert!(!session.session_info.sessions.is_empty());

    Ok(())
}
