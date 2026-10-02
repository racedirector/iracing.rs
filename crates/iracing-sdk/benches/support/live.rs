//! Shared report format for simulator-backed diagnostics. No telemetry payload is serialized.

use anyhow::{Context, Result, anyhow, bail};
use iracing_sdk::VariableSchema;
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct Options {
    pub output: Option<PathBuf>,
    pub profile: String,
    pub warmup_frames: usize,
    pub target_frames: usize,
    pub timeout: Duration,
}

impl Options {
    pub fn parse() -> Result<Self> {
        let mut options = Self {
            output: None,
            profile: String::new(),
            warmup_frames: 120,
            target_frames: 600,
            timeout: Duration::from_secs(60),
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| anyhow!("missing value for {arg}"))?;
            match arg.as_str() {
                "--output" => options.output = Some(PathBuf::from(value)),
                "--profile" => options.profile = value,
                "--warmup-frames" => options.warmup_frames = value.parse()?,
                "--target-frames" => options.target_frames = value.parse()?,
                "--timeout-seconds" => options.timeout = Duration::from_secs(value.parse()?),
                _ => bail!("unknown argument {arg}"),
            }
        }
        if options.profile.is_empty()
            || options.profile.len() > 48
            || !options
                .profile
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            bail!("--profile must be a non-sensitive label using 1-48 letters, digits, - or _");
        }
        if options.target_frames < 600 {
            bail!("--target-frames must be at least 600");
        }
        if options.timeout.is_zero() {
            bail!("--timeout-seconds must be positive");
        }
        Ok(options)
    }
}

#[derive(Serialize)]
pub struct Environment {
    pub os_version: String,
    pub cpu_model: String,
    pub logical_cores: usize,
    pub rustc: String,
    pub target: String,
    pub build_profile: &'static str,
    pub power_profile: String,
}

#[derive(Serialize)]
pub struct Scenario {
    pub source: &'static str,
    pub declared_tick_hz: i32,
    pub frame_size: usize,
    pub schema_fingerprint: String,
    pub warmup_frames: usize,
    pub target_frames: usize,
    pub workload_version: u32,
}

#[derive(Serialize)]
pub struct Case {
    pub id: &'static str,
    pub experiment_version: u32,
    pub status: &'static str,
    pub samples: usize,
    pub parameters: Value,
    pub metrics: Value,
}

#[derive(Serialize)]
pub struct Run {
    pub format_version: u32,
    pub run_id: String,
    pub git_sha: String,
    pub git_dirty: bool,
    pub profile: String,
    pub environment: Environment,
    pub scenario: Scenario,
    pub cases: Vec<Case>,
}

fn command(program: &str, args: &[&str], cwd: &Path) -> Result<String> {
    let output = Command::new(program).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        bail!("{program} {} failed", args.join(" "));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn fingerprint(schema: &VariableSchema) -> Result<String> {
    let mut variables: Vec<_> = schema.variables.values().collect();
    variables.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    let bytes = serde_json::to_vec(&(schema.frame_size, variables))?;
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    Ok(format!("fnv1a64-{hash:016x}"))
}

pub fn new_run(options: &Options, schema: &VariableSchema, tick_hz: i32) -> Result<Run> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let git_sha = command("git", &["rev-parse", "HEAD"], &workspace)?;
    let git_dirty = !command("git", &["status", "--porcelain"], &workspace)?.is_empty();
    let power = command("powercfg", &["/getactivescheme"], &workspace)
        .unwrap_or_else(|_| "unknown".to_owned());
    // Keep only the human-readable plan name; the GUID is unnecessary in a shared report.
    let power_profile = power
        .split_once('(')
        .and_then(|(_, tail)| tail.split_once(')'))
        .map_or("unknown", |(name, _)| name)
        .to_owned();
    Ok(Run {
        format_version: 1,
        run_id: format!("{}-{}", now.as_secs(), now.subsec_nanos()),
        git_sha,
        git_dirty,
        profile: options.profile.clone(),
        environment: Environment {
            os_version: command("cmd", &["/c", "ver"], &workspace)
                .unwrap_or_else(|_| "unknown".to_owned()),
            cpu_model: std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown".into()),
            logical_cores: std::thread::available_parallelism().map_or(1, usize::from),
            rustc: command("rustc", &["-Vv"], &workspace)?,
            target: command("rustc", &["-vV"], &workspace)?
                .lines()
                .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
                .context("rustc did not report a target")?,
            build_profile: "bench",
            power_profile,
        },
        scenario: Scenario {
            source: "active-iracing",
            declared_tick_hz: tick_hz,
            frame_size: schema.frame_size,
            schema_fingerprint: fingerprint(schema)?,
            warmup_frames: options.warmup_frames,
            target_frames: options.target_frames,
            workload_version: 1,
        },
        cases: Vec::new(),
    })
}

pub fn write_run(run: &Run, options: &Options) -> Result<PathBuf> {
    let output = options.output.clone().unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target/live-benchmarks/runs")
            .join(format!("{}.json", run.run_id))
    });
    let parent = output.parent().context("output needs a parent directory")?;
    fs::create_dir_all(parent)?;
    let payload = serde_json::to_vec_pretty(run)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .with_context(|| format!("refusing to replace {}", output.display()))?;
    use std::io::Write;
    file.write_all(&payload)?;
    file.write_all(b"\n")?;
    Ok(output)
}

pub fn percentile(samples: &mut [u64], quantile: f64) -> f64 {
    samples.sort_unstable();
    let index = ((samples.len() - 1) as f64 * quantile).round() as usize;
    samples[index] as f64 / 1_000.0
}
