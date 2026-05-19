#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Command } from "commander";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

const protoFile = "./docs/specs/broadcast.proto";
const protoIncludeDir = "./docs/specs";

const binPath = (name) => {
  const executable = process.platform === "win32" ? `${name}.cmd` : name;
  const candidate = path.join(rootDir, "node_modules", ".bin", executable);

  if (!existsSync(candidate)) {
    throw new Error(
      `Missing ${candidate}. Run pnpm install before generating protobuf files.`,
    );
  }

  return candidate;
};

const program = new Command("iracing-broadcast-grpc")
  .description("Generate gRPC-Web protobuf client files for iracing-status.")
  .requiredOption("-o, --output <path>", "Output path")
  .action(({ output }) => {
    rmSync(output, { recursive: true, force: true });
    mkdirSync(output, { recursive: true });

    const protocCommand = process.env.PROTOC || "protoc";
    const protocArgs = [
      `-I=${protoIncludeDir}`,
      protoFile,
      `--plugin=protoc-gen-js=${binPath("protoc-gen-js")}`,
      `--plugin=protoc-gen-grpc-web=${binPath("protoc-gen-grpc-web")}`,
      `--js_out=import_style=commonjs,binary:${output}`,
      `--grpc-web_out=import_style=typescript,mode=grpcwebtext:${output}`,
    ];

    const result = spawnSync(protocCommand, protocArgs, {
      cwd: rootDir,
      stdio: "inherit",
      shell: false,
    });

    if (result.error) {
      if (result.error.code === "ENOENT") {
        throw new Error(
          `Unable to find ${command}. Install it or set PROTOC to the protoc executable path.`,
        );
      }

      throw result.error;
    }

    if (result.status !== 0) {
      process.exit(result.status ?? 1);
    }

  });

program.parse();
