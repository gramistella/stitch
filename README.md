# Stitch

![Rust CI](https://github.com/gramistella/stitch/actions/workflows/ci.yml/badge.svg)
[![Rust Version](https://img.shields.io/badge/rust-1.89.0%2B-blue.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Stitch is a lightweight desktop utility that lets you **select a precise slice of a codebase** and “stitch” it into a single, shareable text block. It’s designed for **LLM chat interfaces you already use**—pasteable, auditable, and editor-agnostic.

> Originally prototyped in Python/Tk. Now rewritten in **Rust + Slint** for speed, portability, and a cleaner UX.

---

## ✨ What it does

- **Fast native UI (Slint)** with a responsive tree even on large projects.
- **Deterministic context packing**: you decide exactly which files/dirs are included and how they’re scrubbed.
- **Powerful filtering**
  - Include by extension: `.rs,.toml`
  - Exclude by extension (leading `-`): `-.lock,-.png`
  - Include takes precedence over exclude when both are present.
  - Dotfiles are visible by default.
- **Two “only” modes**
  - **Hierarchy Only** – just the tree
  - **Directories Only** – only directory names (no file contents)
- **“Select from Text…”**: paste a previously generated tree to auto-reselect the same files.
- **Scrubbing tools**
  - **Remove lines starting with** prefixes (e.g. `#, //, --`)
  - **Remove regex** (wrapped as `(?ms)` under the hood) to delete spans/blocks
- **Auto refresh**
  - Event-driven (via `notify`) with a lightweight periodic check; only triggers when changes are relevant given your filters.
- **One-click copy** of the final output.
- **Token & character stats**
  - Uses `tiktoken-rs` (`o200k_base`) when the `tokens` feature is enabled.

---

## 🧭 Philosophy

- Use the **chat models you already have**—no API keys.
- **Full control & auditability**: you see exactly what the model sees.
- A curated, minimal context often **beats** generic retrieval on long-tail tasks.

---

## 🧰 Install & Run

### Prerequisites
- **Rust** (stable) + **Cargo**
- Optional packaging helpers:
  - macOS DMG: `cargo-bundle` (auto-installed by `just dmg`)
  - `just` if you want the packaging shortcuts used by CI

### Run in dev
```bash
cargo run --features ui,tokens
# or optimized:
cargo run --release --features ui,tokens
```

> The default crate features already include `ui` and `tokens`.  
> Headless builds for tests: `cargo test --no-default-features`.

### Build a release binary
```bash
cargo build --release --features ui,tokens
```

### Create distributables (same commands CI uses)

Requires `just`:
```bash
cargo install just --locked
```

- **macOS (.app + .dmg)**
  ```bash
  just dmg
  # -> dist/Stitch-<version>.dmg
  ```
- **Windows (.zip with stitch.exe)**
  ```bash
  just exe
  # -> dist/stitch-<version>-windows-x86_64.zip
  ```
- **Linux (.tar.gz)**
  ```bash
  just tgz
  # -> dist/stitch-<version>-linux-<arch>[-musl].tar.gz
  ```

Cross-compile by setting `TARGET=<triple>` (e.g. `x86_64-unknown-linux-musl`) before running the recipe.

> **macOS Gatekeeper note**  
> If you see “Stitch is damaged and can’t be opened”:
> ```bash
> xattr -cr /Applications/Stitch.app
> ```

---

## 🖱️ How to use

1. **Select Folder** – choose your project root.
2. **Adjust Filters** (optional):
   - **Filter Extensions** (comma-separated):
     - include only: `.rs,.toml`
     - exclude some: `-.lock,-.png`
     - mixing (include wins): `.rs,.md,-.lock`
   - **Exclude Directories / Files** (comma-separated basenames)
     - sensible defaults are pre-filled (e.g. `.git`, `node_modules`, `target`, `LICENSE`, lockfiles, etc.)
3. **Select Items** – check files or directories. Directory checks cascade; you can override at any level.
4. **Choose Mode**
   - **Hierarchy Only** – emits only the tree
   - **Directories Only** – emits only selected dirs (no file contents)
5. **Generate Output** – you’ll get:
   - `=== FILE HIERARCHY ===` (unicode tree)
   - `=== FILE CONTENTS ===` (unless an “only” mode is active)
6. **Copy Output** – copies the **entire** output (even if the UI truncates display for very large results).

### “Select from Text…” (round-trip selection)
Paste a Stitch-generated hierarchy (first line = root folder name). Stitch parses it and reselects the files.  
Works with CRLF/LF line endings and is tolerant of trailing whitespace/blank lines.

---

## 🧠 Profiles & Workspace

Stitch keeps per-project state in a `.stitchworkspace` folder (auto-excluded from scans).

- **Workspace settings** (`workspace.json`): the “— Workspace —” entry in the selector.
- **Profiles**: save **named** selections and settings.
  - **Shared** profiles → `.stitchworkspace/profiles/*.json`  
    **Commit these to version control** to share with the team.
  - **Local/Private** profiles → `.stitchworkspace/local/profiles/*.json`  
    **Not for VCS** (per-user, machine-specific).
- UI actions:
  - **Save Workspace Settings** (when “— Workspace —” is selected)
  - **Save / Save As…** (choose Shared vs Local)
  - **Delete**, **Discard Changes**
- The current profile is remembered in `workspace.json`.

> **Git tip**  
> When Stitch creates `.stitchworkspace` for the first time, if a root `.gitignore` exists, Stitch appends:
> ```
> # Stitch workspace (per-user)
> .stitchworkspace/local/
> ```
> (only if not already present). This keeps local, per-user state out of your repo while letting you commit shared profiles and workspace defaults.

---

## 🤝 Team-wide Collaboration

Stitch is great for **team workflows**—you can standardize “what to share” for PRs, issues, and LLM prompts.

- **Commit the workspace** (excluding local):
  - Add and commit `.stitchworkspace/` to your repo
  - The `local/` subfolder is per-user and should stay ignored (Stitch helps by auto-appending it to `.gitignore` on first creation)
- **Share named profiles**:
  - Create profiles (e.g., `bug-4321`, `release-notes`, `llm-minimal`) as **Shared**
  - Commit the resulting JSON files under `.stitchworkspace/profiles/`
  - Teammates pull and select the same profile to get an identical file selection and scrub settings
- **Common patterns**:
  - **PR review kit**: `pr-1234` profile that captures only the touched areas + relevant context
  - **Minimal repro**: `repro-foo-crash` profile that trims the repo to what matters
  - **LLM prompt packs**: `api-client-minimal` / `frontend-deps` profiles you can swap between quickly

> **Why it works well**  
> Profiles are plain JSON and diff nicely in PRs. Everyone can audit exactly what goes in the stitched output. No proprietary format or editor plugin needed.

---

## 🧽 Scrubbing & Cleanup

- **Remove lines starting with:** comma-separated prefixes (e.g., `#, //, --`).
  - Full-line comments are removed (leading whitespace allowed).
  - **Inline** comments are removed only when **immediately preceded by whitespace** (incl. Unicode spaces & tabs).
  - **Protected regions:** content inside normal strings, raw strings (`r#"..."#` with hashes), and triple quotes (`"""..."""` / `'''...'''`) is preserved.
- **Remove regex:** your pattern is compiled as `(?ms)<your-pattern>` (multi-line + dot-matches-newline).
  - You may quote it with single/double or triple quotes; Stitch will strip the quotes before compiling.

> ⚠️ Scrubbing is text-only; it doesn’t parse language syntax. Double-check semantics before pasting back into a compiler.

---

## 🧩 Typical workflows

- **LLM context packing**: curate a minimal, auditable set of files.
- **Minimal repros**: share only the relevant sources + a tree.
- **Reviews & handoffs**: generate a portable snapshot for PRs/issues/email.
- **LLM-guided selection**: let a model propose a minimal tree; paste via “Select from Text…”.

---

## 🔬 Implementation notes

- **Tech**: Rust 2024 edition, Slint, `rfd`, `notify`, `regex`, `chrono`, `serde`/`serde_json`, `dunce`, `arboard`.
- **Auto refresh**:
  - Event-driven (`notify`) pump that filters out irrelevant changes (e.g., excluded dirs/files).
  - A lightweight periodic check is also in place.
- **Display limits**: the UI shows up to ~50k characters for responsiveness; **Copy Output** always copies the full text.
- **Token counting**:
  - With the `tokens` feature, Stitch uses `tiktoken-rs` (`o200k_base`) and counts special tokens.
  - For very large outputs (>16 MB) or without `tokens`, it falls back to a cheap approximation.
- **Extension matching semantics**:
  - Case-insensitive (`.TXT` matches `.txt`).
  - “Include” mode shows only files whose **last** extension segment matches (so `archive.tar.gz` is treated as `.gz`).

---

## 🧪 Testing & Benchmarks

- **Tests (headless)**  
  ```bash
  cargo test --no-default-features
  ```
  CI runs these on Linux/macOS/Windows and also checks the UI build path.

- **Benchmarks** (Criterion with HTML reports)  
  ```bash
  cargo bench
  # results under target/criterion
  ```

---

## 🤖 CI & Releases

- **CI**: `.github/workflows/ci.yml`
  - Lints (fmt + clippy), tests headless, and verifies the UI build path.
- **Releases**: `.github/workflows/release.yml`
  - Tag `v*` to build portable artifacts for Linux (`.tar.gz`), Windows (`.zip`), and macOS (`.dmg`), then attach to a GitHub Release.

---

## 🔧 Feature flags

- `ui` (default): build the Slint desktop app.
- `tokens` (default): enable accurate token counting with `tiktoken-rs`.

Headless library/test builds:
```bash
cargo build --no-default-features
cargo test  --no-default-features
```

> When built *without* `ui`, the `stitch` binary only prints a helpful message; the core library remains available for tests.

---

## 🧱 Known limitations / edges

- **Very large repos**: first scan can be heavy—lean on filters early.
- **Binary/huge files**: not specially parsed; consider excluding them.
- **Multi-dot extensions**: only the **last** segment is considered (e.g., `.tar.gz` → `.gz`).
- **Scrubbing**: may remove content inside comments/strings in ways that matter to your code—review before sharing.

---

## 🤝 Contributing

Issues and PRs welcome—especially around defaults (exclusions), performance, UI polish, and integrations.  
If adding assets, place third-party licenses in `LICENSES/`.

---

## 📄 License

This project is licensed under the **MIT License** — see [LICENSE](./LICENSE) for details.

It also bundles third-party assets:

- [JetBrains Mono](https://www.jetbrains.com/lp/mono/) font, licensed under the  
  [SIL Open Font License 1.1](./LICENSES/LICENSE-JetBrainsMono.txt).

All third-party licenses are collected in the [LICENSES/](./LICENSES) folder.