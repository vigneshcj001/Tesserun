# Tesserun

A native Rust development studio for turning an application brief into source code, reviewing it with specialist agents, and publishing static websites.

Tesserun brings **development, quality checks, and deployment** into one desktop workspace. The interface uses egui/eframe; the application services, provider adapters, storage, and optional HTTP API are written in Rust.

> Current status: an early desktop implementation. Generated applications receive static checks and advisory AI review; they are not executed or behavior-tested automatically.

## Features

- **Agent workflow:** Planner, Developer, and Reviewer make separate requests to your configured provider and model.
- **Project workspace:** starter briefs, source editing, saved revisions, and protection against losing unsaved changes.
- **Quality checks:** deterministic source checks alongside advisory agent findings. Reports become stale when source changes.
- **Deployment:** export a source bundle or explicitly publish static files to Vercel.
- **Provider connections:** Ollama, OpenAI, Anthropic, Gemini, and Groq.
- **AI chat:** a session-only conversation with your configured model. Long chats send the most recent messages that fit provider limits (200 messages / 300 KB).
- **Responsive desktop work:** cancellable background operations and an optional agent activity panel.
- **Optional local API:** authenticated endpoints for generation, chat, and project listing.

The model-driven roles run sequentially within one workflow. QA performs deterministic checks; the release role handles export and publishing. They are not five independently running model agents.

See [docs/AI_SYSTEM_DESIGN.md](docs/AI_SYSTEM_DESIGN.md) for the request pipeline, safety controls and limits, and a review against common AI system design practices.

## Quick start on the existing Windows workspace

From PowerShell:

```powershell
cd D:\Tesserun-Workspace\Tesserun
.\scripts\rust.cmd run
```

Use the `.cmd` launcher if Windows blocks PowerShell scripts. It does not require changing the PowerShell execution policy.

When available, the launcher uses the project-local Rust and LLVM-MinGW toolchain under `.tools`. This toolchain is not included in the Git repository. Otherwise, the launcher uses a system Rust installation.

## Set up a fresh Windows clone

Install Git, Rust through rustup, and Visual Studio Build Tools with the **Desktop development with C++** workload and a Windows SDK. The commands below explicitly select the Windows MSVC toolchain, avoiding the existing GNU linker configuration.

```powershell
git clone https://github.com/vigneshcj001/Tesserun.git
cd Tesserun

rustup toolchain install stable-x86_64-pc-windows-msvc
cargo +stable-x86_64-pc-windows-msvc run --target x86_64-pc-windows-msvc --bin tesserun
```

The crate uses Rust edition 2024. Use a current stable compiler. Cargo downloads dependencies on the first build.

For a GNU build, `.cargo/config.toml` requires `ld.lld.exe` and `llvm-dlltool.exe` on PATH. The project-local launcher supplies these paths only when the matching local tools are present.

## Create your first project

1. Open **Settings** and choose a provider and model.
2. Enter a provider API key, or configure a local Ollama endpoint and installed model.
3. Open **Development**, describe the application, and run the agent team.
4. Review the generated source, save any edits, and run **Quality checks**.
5. Open **Deployments** to export source or publish to Vercel.

A development workflow makes three model requests. Provider usage may incur charges. If the advisory reviewer fails, generated source is retained and the review is marked as needing attention. Each build saves one revision under your original brief, along with the provider, model, prompt version, and plan used. Busy or failing providers (HTTP 429, 5xx, or connection errors) are retried up to twice. If the model's reply isn't usable project JSON, Tesserun asks once for a corrected reply; if that also fails, nothing is saved.

Keys entered in the UI stay in process memory. Provider environment variables are also supported: `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, and `GROQ_API_KEY`. Ollama endpoints are restricted to loopback addresses. `.env` files are not loaded automatically.

## Deploy a generated website

The integrated Vercel publisher supports the generated static HTML, CSS, and JavaScript files. It does not deploy the Tesserun desktop interface or a generated backend/database service.

1. Create an access token in your [Vercel account settings](https://vercel.com/account/tokens).
2. Close Tesserun and launch it from a PowerShell session configured as follows:

```powershell
$token = Read-Host "Paste Vercel token" -AsSecureString
# Paste the token at the prompt and press Enter; input is hidden.
$env:VERCEL_TOKEN = [System.Net.NetworkCredential]::new("", $token).Password
$env:TESSERUN_ENABLE_VERCEL_DEPLOYMENTS = "true"

.\scripts\rust.cmd run
```

For a fresh clone using MSVC, replace the last command with the Cargo run command from the setup section.

3. Select your project and save all source changes.
4. Open **Deployments**, select the production publishing confirmation, and click **Publish to Vercel**.
5. Open the returned URL. A **queued** response means the deployment was submitted; check the Vercel dashboard for its final status.

Optional environment variables `VERCEL_PROJECT_ID` and `VERCEL_TEAM_ID` select an existing project and team. The token must have access to the selected resources. Publishing is never automatic.

To keep the output local, use **Export source bundle**. Exports are written under `.tesserun-rust/exports` and include source files, a review report, and an export manifest.

Provider and deployment adapters are tested against local fixtures. Live paid-provider calls and production Vercel publishing have not been verified as part of this implementation. Test generated websites before publishing them.

## Validate and build

With the existing project-local tools:

```powershell
.\scripts\rust.cmd check
.\scripts\rust.cmd test
.\scripts\rust.cmd lint
.\scripts\rust.cmd format
.\scripts\rust.cmd build
```

For a fresh Windows MSVC setup:

```powershell
cargo +stable-x86_64-pc-windows-msvc check --target x86_64-pc-windows-msvc --all-targets
cargo +stable-x86_64-pc-windows-msvc test --target x86_64-pc-windows-msvc --all-targets
cargo +stable-x86_64-pc-windows-msvc clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-msvc build --target x86_64-pc-windows-msvc --release --bin tesserun
```

The local launcher writes the desktop release executable to `target/release/tesserun.exe`. The explicit MSVC build writes it to `target/x86_64-pc-windows-msvc/release/tesserun.exe`. Close a running instance before rebuilding on Windows.

Release builds enable thin LTO and strip symbols. Installer packaging, signing, and standalone distribution remain unverified. Performance has not been benchmarked.

## Architecture

| Module | Responsibility |
| --- | --- |
| `rust/domain.rs` | Project types, build metadata, messages, and generated-file constraints |
| `rust/prompts.rs` | Versioned prompts for each agent role |
| `rust/providers/` | Provider configuration, HTTP requests, and response decoding |
| `rust/service.rs` | Source generation service |
| `rust/workflow.rs` | Agent orchestration, static checks, and source export |
| `rust/storage.rs` | Immutable revisions and latest-project loading |
| `rust/release.rs` | Confirmed Vercel publishing |
| `rust/ui/` | Desktop state, navigation, screens, theme, and background tasks |
| `rust/api.rs` | Authenticated local HTTP routes |
| `rust/main.rs` | Desktop entry point |
| `rust/bin/server.rs` | Headless API entry point |
| `tests/rust_workflow.rs` | Integration tests using local fixtures |
| `docs/AI_SYSTEM_DESIGN.md` | AI pipeline, controls, and design review |

The native interface calls shared Rust services directly. Provider I/O runs on Tokio tasks. Cancellation prevents stale results from updating the workspace, and a task that crashes reports an error instead of leaving the workspace busy. History loading reads only the latest source revision for each project, and skips a damaged revision instead of hiding every project.

## Optional local HTTP API

Set `TESSERUN_API_TOKEN` to a random secret containing at least 32 characters, then run:

```powershell
.\scripts\rust.cmd server
```

On a fresh MSVC setup:

```powershell
cargo +stable-x86_64-pc-windows-msvc run --target x86_64-pc-windows-msvc --no-default-features --bin tesserun-server
```

The server binds to `127.0.0.1:8789`. Set `TESSERUN_RUST_PORT` to change the port.

| Route | Purpose |
| --- | --- |
| `GET /api/health` | Public health check |
| `GET /api/builds` | List saved projects |
| `POST /api/builds` | Generate source from `{ prompt, connection }` |
| `POST /api/chat` | Request a completion from `{ messages, connection }` |

All routes except health require `Authorization: Bearer <token>`. At most two generation or chat requests run at once; additional requests receive HTTP 429. A connection contains `provider`, `model`, optional `api_key`, and an optional loopback Ollama `endpoint`. Requests containing a browser Origin header are rejected. Keep this single-user API local to a trusted user; it is not a public hosting configuration.

## Local data and current limitations

- Revisions are stored in `.tesserun-rust/projects`; `TESSERUN_RUST_DATA_ROOT` overrides the data root.
- Saved source is unencrypted. Provider keys are not saved with projects.
- Each export or publish requires a fresh confirmation; the checkbox clears after every release action.
- Token usage, cost estimates, quality evaluations, and an independent reviewer model are not implemented yet. See the recommendations in `docs/AI_SYSTEM_DESIGN.md`.
- Older `.tesserun` data is not imported by this version.
- Generated files are validated for allowed names, duplicates, file count, and total size. Runtime and security verification are not implemented.
- Streaming, persistent chat history, model discovery, attachments, encrypted credential storage, OAuth, preview rendering, sandbox execution, automatic repair loops, and revision-selection UI remain unimplemented.
- The application is Rust-only; it does not have full feature parity with the earlier implementation.

Keep tokens, `.env` files, `.tools`, `target`, saved project data, and exports out of public commits. The integration tests under `tests/` should be included in the repository so contributors can run the documented checks.
