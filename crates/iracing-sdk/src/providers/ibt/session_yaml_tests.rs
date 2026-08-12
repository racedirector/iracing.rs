use anyhow::{Context, Result};

use super::IbtProvider;
use crate::{provider::Provider, schema::SessionInfo, test_utils::load_fixture_manifest};

#[tokio::test]
async fn every_manifest_fixture_runs_through_the_session_yaml_pipeline() -> Result<()> {
    let manifest = load_fixture_manifest().context("loading deterministic fixture manifest")?;

    for fixture in manifest.fixtures {
        let ibt_path = fixture.fixture_path()?;
        let yaml_path = fixture.session_yaml_file()?;
        let companion_yaml = std::fs::read_to_string(&yaml_path)
            .with_context(|| format!("reading {}", yaml_path.display()))?;

        let mut provider = IbtProvider::open(&ibt_path)
            .with_context(|| format!("opening {}", ibt_path.display()))?;
        let sanitized = provider
            .session_yaml(0)
            .await?
            .with_context(|| format!("fixture {} must contain session YAML", fixture.name))?;

        assert_eq!(
            sanitized.as_str(),
            companion_yaml,
            "embedded YAML differs for fixture {}",
            fixture.name
        );

        let embedded_session = SessionInfo::parse_sanitized(&sanitized)
            .with_context(|| format!("parsing embedded YAML for fixture {}", fixture.name))?;
        let companion_session = SessionInfo::parse(&companion_yaml)
            .with_context(|| format!("parsing companion YAML for fixture {}", fixture.name))?;

        assert_eq!(embedded_session, companion_session);
        assert!(!embedded_session.weekend_info.track_name.is_empty());
        assert!(!embedded_session.session_info.sessions.is_empty());
    }

    Ok(())
}
