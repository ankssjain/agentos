# agentOS embedded Core

`@rivet-dev/agentos-core` embeds an agentOS VM in a Node.js application. The
embedding process owns VM lifecycle and may supply trusted host bindings,
host-backed mounts, and local `.aospkg` paths.

Install Core directly for embedded use:

```sh
pnpm add @rivet-dev/agentos-core
```

Managed services can wrap Core with a transport-specific client. Those adapters
live outside this repository and intentionally expose a smaller trusted-host
surface.
