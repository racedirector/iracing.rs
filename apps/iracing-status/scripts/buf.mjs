#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");
const executable = process.platform === "win32" ? "buf.cmd" : "buf";
const bufPath = path.join(rootDir, "node_modules", ".bin", executable);

if (!existsSync(bufPath)) {
  throw new Error(`Missing ${bufPath}. Run pnpm install before generating protobuf files.`);
}

const env = {
  ...process.env,
  BUF_CACHE_DIR: process.env.BUF_CACHE_DIR ?? path.join(rootDir, ".buf-cache"),
};

const result = spawnSync(bufPath, process.argv.slice(2), {
  cwd: rootDir,
  env,
  stdio: "inherit",
  shell: process.platform === "win32",
});

if (result.error) {
  throw result.error;
}

process.exit(result.status ?? 1);
