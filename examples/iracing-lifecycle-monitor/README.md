# iracing-lifecycle-monitor

Workspace example that supervises the iRacing process lifecycle across:

- `iracing-simulation` process detection
- `iracing-simulation` HTTP sim-status checks
- `iracing-sdk` live shared-memory telemetry connectivity

Run on Windows:

```bash
cargo run -p iracing-lifecycle-monitor -- --help
```
