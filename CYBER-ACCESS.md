# Configurable cyber access programs

This branch adds a configuration default for Codex's cyber access program,
including per-model and per-profile overrides. It applies to ordinary turns,
goal continuations, and native subagents.

Access still depends on your account entitlement and the selected model. These
settings apply to the native OpenAI provider with ChatGPT authentication.

Based on Codex 0.159.0. Upstream now preserves the model and access-program pair
during compaction after a model switch; this branch still supplies the
configuration defaults described below.

## Configure

For example, request Daybreak Blue by default while leaving Astra on automatic
selection. Place this in `~/.codex/config.toml`, with the default at the top level:

```toml
cyber_access_program = "daybreak_blue"

[cyber_access_program_by_model]
"gpt-6-astra" = "auto"
```

Available values: `auto`, `standard`, `daybreak_blue`, and `daybreak_red`. Model
names are exact matches. `auto` leaves the program field out of the request;
`standard` explicitly requests standard treatment.

These settings also work in named `~/.codex/<name>.config.toml` profiles and agent
role files. An explicit `turn/start.cyberAccessProgram` takes precedence over
configuration. Otherwise, the exact-model setting takes precedence over the
default. A profile's default does not erase model-specific entries inherited
from other configuration layers; override those entries individually when needed.

Defaults are evaluated for each new turn. See the
[app-server reference](codex-rs/app-server/README.md#initial-daybreak-choice-experimental)
for inheritance, recovery, and active-turn behavior.

## Build the patched package

The commands below target Linux x86-64. Requirements: the repository's pinned
Rust toolchain, Python 3, `just`, and native build dependencies. Building the
bundled Bubblewrap helper also requires `pkg-config` and the libcap development
headers (`libcap-dev` on Debian/Ubuntu).

```bash
git clone --branch feat/default-cyber-access-program --single-branch \
  https://github.com/ustas-eth/codex.git codex-cyber
cd codex-cyber
```

The branch sets `[workspace.package].version` in `codex-rs/Cargo.toml` to:

```toml
version = "0.159.0+cyber.1"
```

This identifies the patched build based on upstream tag `rust-v0.159.0`.
Codex sends its compiled version to the backend; leaving it at
`0.0.0` can cause compatible models to be rejected with a misleading
ChatGPT-account error. Setting only the package builder's `--package-version`
does not change that request header.

Build the complete package:

```bash
STABLE_GIT_COMMIT="$(git rev-parse HEAD)" just assemble-codex-package \
  --target x86_64-unknown-linux-gnu \
  --cargo-profile release \
  --package-dir "$PWD/codex-cyber-package"
```

The package builder fetches and verifies the matching Codex-built V8 artifacts.
The resulting directory includes the CLI, code-mode host, and supporting
resources. Keep the directory together; copying only the `codex` executable
leaves out required helpers.

The commit stamp gives the runtime its build provenance; without it, Cargo
builds display `dev` in parts of the interface even with a release version.

For other platforms, consult the [package builder options](scripts/codex_package/README.md).
The checks described below were performed on Linux x86-64.

## Run separately from stock Codex

Run the package directly:

```bash
./codex-cyber-package/bin/codex --version
./codex-cyber-package/bin/codex
```

The reported version should be `0.159.0+cyber.1`. You can move the complete
package directory to a permanent location and put a symlink to its `bin/codex`
on your PATH under a distinct name such as `codex-cyber`.

For an app server:

```bash
./codex-cyber-package/bin/codex app-server --listen unix://
```

Stop the existing server deliberately before starting its replacement on the
same socket. Updating only the connecting TUI does not update the server.

This leaves your npm or other stock installation intact. Both installations
use the same Codex configuration and state by default.

## Verification

Regression tests cover configuration precedence, model switches, goal
continuations, subagent inheritance, and compaction with the selected program.
Run the focused tests when updating the upstream base:

```bash
just test -p codex-core --lib -E 'test(cyber_access_program)'
just test -p codex-core -p codex-app-server --test all \
  -E 'test(cyber_access_program) | test(model_switch_program_pair)'
```

After installing, verify a real tool call as well as model selection. A
successful `--version` or `--help` check alone does not establish that the
complete package works.
