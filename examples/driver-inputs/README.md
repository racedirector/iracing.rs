# driver-inputs

`driver-inputs` reads an iRacing `.ibt` replay file, extracts driver input telemetry, and writes the result to CSV.

## Prerequisites

- Run `git lfs pull` in the repository if you have not already fetched the telemetry fixtures.
- Use an `.ibt` file on disk. Paths with spaces must be quoted.

## Run

From the workspace root:

```bash
cargo run -p driver-inputs -- \
  --ibt-path "test-data/stockcars2 mustang2019_bristol fullpit 2026-04-07 13-11-05.ibt" \
  --csv-output-path "test-data/output.csv"
```

You can also inspect the CLI help:

```bash
cargo run -p driver-inputs -- --help
```

## Flags

- `--ibt-path <PATH>`: input `.ibt` telemetry file to read.
- `--csv-output-path <PATH>`: destination CSV file to create.

## Output

The command writes one CSV row per decoded frame. If the output file already exists, it will be overwritten.
