//! Downstream tooling can inspect complete geometry using only the public layout.

use iracing_sdk::provider::VariableHeadersProvider;
use iracing_sdk::{
    IbtLayout,
    ibt::IbtReader,
    irsdk::{DiskSubHeader, Header},
    test_utils::require_smallest_ibt_fixture,
};
use zerocopy::IntoBytes;

#[test]
fn public_layout_describes_owned_and_mapped_recordings() -> anyhow::Result<()> {
    let path = require_smallest_ibt_fixture()?;
    let bytes = std::fs::read(&path)?;
    let mapped = IbtReader::open(&path)?;
    let owned = IbtReader::from_bytes(bytes.clone())?;

    for reader in [mapped, owned] {
        // A probe uses the reader's layout without recomputing geometry from headers.
        let layout: IbtLayout = reader.layout().clone();
        assert_eq!(layout.source_len(), bytes.len());
        assert_eq!(
            &bytes[layout.header_region().as_range()],
            reader.header().as_bytes()
        );
        assert_eq!(
            &bytes[layout.disk_header_region().as_range()],
            reader.disk_header().as_bytes()
        );
        assert_eq!(
            layout.preamble_region().len(),
            size_of::<Header>() + size_of::<DiskSubHeader>()
        );

        let session = layout
            .metadata()
            .session_info()
            .expect("fixture has session metadata");
        let variables = layout
            .metadata()
            .variable_headers()
            .expect("fixture has variable headers");
        assert!(session.offset() >= layout.preamble_region().end());
        assert!(variables.offset() >= layout.preamble_region().end());
        assert!(!session.as_region().overlaps(variables.as_region()));
        assert_eq!(
            layout.frame_data_start(),
            session.end().max(variables.end())
        );
        assert_eq!(layout.frames().end(), layout.source_len());

        // Metadata reads may move the source cursor; indexed frame reads still use the layout.
        assert!(
            iracing_sdk::provider::SessionInformationBytesProvider::session_info_snapshot(&reader)?
                .is_some()
        );
        assert!(!reader.variable_headers()?.is_empty());
        for index in [layout.frame_count() - 1, 0] {
            let region = layout.frame(index)?;
            assert_eq!(region.len(), layout.frame_size());
            assert_eq!(reader.frame(index)?, bytes[region.as_region().as_range()]);
        }
        assert!(layout.frame(layout.frame_count()).is_err());
        assert!(reader.frame(layout.frame_count()).is_err());
        drop(reader);
        // The physical description is reusable independently of the reader/source lifetime.
        assert_eq!(layout.source_len(), bytes.len());
    }
    Ok(())
}
