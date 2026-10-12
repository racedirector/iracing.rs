//! Downstream tooling can inspect complete geometry using only the public layout.

use iracing_sdk::{
    IbtFile, IbtLayout,
    irsdk::{DiskSubHeader, Header},
    test_utils::require_smallest_ibt_fixture,
};
use zerocopy::IntoBytes;

#[test]
fn public_layout_describes_owned_and_mapped_recordings() -> anyhow::Result<()> {
    let path = require_smallest_ibt_fixture()?;
    let bytes = std::fs::read(&path)?;
    let mapped = IbtFile::open(&path)?;
    let owned = IbtFile::from_bytes(bytes.clone())?;

    for file in [mapped, owned] {
        // A probe uses the file's layout without recomputing geometry from headers.
        let layout: IbtLayout = file.physical_layout().clone();
        assert_eq!(layout.source_len(), bytes.len());
        assert_eq!(
            &bytes[layout.header_region().as_range()],
            file.header().as_bytes()
        );
        assert_eq!(
            &bytes[layout.disk_header_region().as_range()],
            file.disk_header().as_bytes()
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

        // Immutable metadata inspection and frame access share the validated file layout.
        assert!(file.session_info_bytes().is_some());
        assert!(!file.variable_headers().is_empty());
        for index in [layout.frame_count() - 1, 0] {
            let region = layout.frame(index)?;
            assert_eq!(region.len(), layout.frame_size());
            assert_eq!(
                file.frame(index)?.bytes(),
                &bytes[region.as_region().as_range()]
            );
        }
        assert!(layout.frame(layout.frame_count()).is_err());
        assert!(file.frame(layout.frame_count()).is_err());
        drop(file);
        // The physical description is reusable independently of the file/source lifetime.
        assert_eq!(layout.source_len(), bytes.len());
    }
    Ok(())
}
