# agentOS

agentOS is a lightweight Linux-compatible VM runtime for isolated filesystem,
process, terminal, language, and networking workloads.

This repository owns the native sidecar, VM kernel and VFS, language execution,
software package format and cache, and the embedded TypeScript and Rust Core
clients. Hosted orchestration adapters live in the services repository and
consume the public or same-version service integration surface from here.

## Software

Software is an immutable v2 `.aospkg` artifact. Core can install a verified
remote URL or a trusted local package path.

The process-wide cache is content-addressed by digest. Release tooling uploads
the catalog to S3-compatible object storage and emits a manifest of relative
artifact URLs, digests, and sizes. Runtime software is not resolved from npm.

## Quickstart

```bash
npm install @rivet-dev/agentos-core
```

```ts
import { AgentOs } from "@rivet-dev/agentos-core";

const vm = await AgentOs.create();
try {
  await vm.filesystem.writeFile("/tmp/hello.txt", "hello\n");
  console.log((await vm.process.exec("cat /tmp/hello.txt")).stdout);
} finally {
  await vm.dispose();
}
```

Embedded Core supports trusted local `.aospkg` paths, host-backed mount plugins,
external VM providers, and host bindings.

## Development

```bash
pnpm install --frozen-lockfile
pnpm build
pnpm check-types
cargo check --workspace
```

Rebuild the complete command catalog and stage the local object-store release
layout with:

```bash
just tools-rebuild
just software-artifacts-dry-run
```

Public documentation lives at [agentos-sdk.dev](https://agentos-sdk.dev).
