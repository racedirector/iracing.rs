# Configuration domain

Read `Cargo.toml`, every relevant feature/cfg, dist metadata and
`docs/architecture/platform-and-features.md`. Inventory supported compiler,
endianness, pointer width and platform assumptions separately from tested cells.
Identify conditional producers/consumers on Windows and portable stubs elsewhere.
Do not silently restrict an all-platform claim to one runner or one constructor.

State which evidence comes from source reasoning, const layout assertions,
ordinary tests, Miri, native Windows integration or an external specification.
Mocked memory is useful for algorithms, not evidence that real mapping extent,
hardware ordering, external updates or event lifetime satisfy the model.
