# Configurable cyber access programs

This branch adds a configuration default for Codex's cyber access program,
including per-model and per-profile overrides. It applies to ordinary turns,
goal continuations, and native subagents.

Access still depends on your account entitlement and the selected model. These
settings apply to the native OpenAI provider with ChatGPT authentication.

Based on Codex 0.162.0. Upstream provides Daybreak controls in TUI/exec behind
`features.cli_daybreak`, plus explicit per-turn app-server selection. This branch
adds server-side defaults and connects the saved thread choice to automatic goal
work. Compaction and API-key access-program policy use the upstream implementation.

## Configure

For example, request Daybreak Blue by default while leaving selected models on
automatic selection. Place this in `~/.codex/config.toml`, with the default at the top level:

```toml
cyber_access_program = "daybreak_blue"

[cyber_access_program_by_model]
"gpt-6-astra" = "auto"
"gpt-6.1-sol" = "auto"
```

Available values: `auto`, `standard`, `daybreak_blue`, and `daybreak_red`. Model
names are exact matches. `auto` leaves the program field out of the request;
`standard` explicitly requests standard treatment.

These settings also work in named `~/.codex/<name>.config.toml` profiles and agent
role files. An explicit `turn/start.cyberAccessProgram` takes precedence over
the saved thread choice, then configuration. Otherwise, the exact-model setting
takes precedence over the default. A profile's default does not erase
model-specific entries inherited from other configuration layers; override those
entries individually when needed.

These defaults are independent of `features.cli_daybreak`. Native CLI controls
send a per-turn selection, which takes precedence over server defaults.

Configured defaults apply to ChatGPT authentication. API-key turns retain
upstream behavior: an explicit program requires the
`features.api_key_cyber_access_programs` opt-in. A subscription default does not
implicitly opt an API-key session into that feature.

Defaults are evaluated for each new turn. See the
[app-server reference](codex-rs/app-server/README.md#initial-daybreak-choice-experimental)
for inheritance, recovery, and active-turn behavior.

## Change a running thread's preference

Enable the native `/daybreak` control with `--enable cli_daybreak`, or set
`cli_daybreak = true` under `[features]`. The command saves the current thread's
choice and the client's default for future threads. Account and model
availability still apply.

With this branch running the app server, the saved thread choice also applies to
automatic goal continuations. Off requests `standard`; on selects the model's
advertised Daybreak program, or requests Blue if none is advertised.
Unsupported requests can still be rejected by the backend. An unset thread
choice uses the configured defaults above.

External clients can use the existing experimental `thread/metadata/update`
request with `{"threadId":"THREAD_ID","daybreakEnabled":false}` or `true`.
After its acknowledgment, new turns use that choice. Automatic goal turns use
the saved choice rather than a selection carried from an earlier turn. A fresh
explicit `turn/start.cyberAccessProgram` still wins. The current turn, including
its tool calls and compaction, keeps its original selection. Interrupted-turn
recovery also keeps the interrupted turn's persisted selection; it is not a new
turn. Neither the goal nor the conversation needs to be replaced.

## Hide the buffering popup

To hide the informational “Giving this request a little extra thought” banner
and faster-model retry menu, set this in the connecting TUI's configuration:

```toml
[tui]
show_safety_buffering = false
```

The default is `true`. Restart the connecting TUI to load the display preference;
the app server does not need a restart for this setting. Backend checks, waiting,
refusals, verification, and diagnostic events are unchanged. It does not
automatically retry or switch models. The patched TUI must be running to use it.

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
version = "0.162.0+cyber.1"
```

This identifies the patched build based on upstream tag `rust-v0.162.0`.
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

The reported version should be `0.162.0+cyber.1`. You can move the complete
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

Regression tests cover configuration precedence, model switches, native goal
creation and continuation, subagent inheritance, API-key policy, and compaction
with the selected program. They also cover live preference changes, cold resume,
and the optional buffering UI.
Run the focused tests when updating the upstream base:

```bash
just test -p codex-config -p codex-core -p codex-tui --lib \
  -E 'test(cyber_access_program) | test(daybreak) | test(safety_buffering)'
just test -p codex-core -p codex-app-server --test all \
  -E 'test(cyber_access_program) | test(daybreak_metadata_toggle) | test(saved_daybreak_choice) | test(model_switch_program_pair)'
just test -p codex-exec --test all \
  -E 'test(daybreak) | test(cyber_access_program)'
```

After installing, verify a real tool call as well as model selection. A
successful `--version` or `--help` check alone does not establish that the
complete package works.
