#!/usr/bin/env node
/**
 * Compile crates/kim-protocol/proto/pkt.proto to a protobufjs JSON descriptor.
 * No protoc: protobufjs parses .proto itself.
 */
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import protobuf from "protobufjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const sdkRoot = path.resolve(here, "..");
const protoPath = path.resolve(
  sdkRoot,
  "../../crates/kim-protocol/proto/pkt.proto",
);
const outDir = path.join(sdkRoot, "src/proto");
const outFile = path.join(outDir, "pkt.json");

const root = await protobuf.load(protoPath);
await mkdir(outDir, { recursive: true });
await writeFile(outFile, `${JSON.stringify(root.toJSON(), null, 2)}\n`);
console.log(`wrote ${path.relative(sdkRoot, outFile)}`);
