# Stitch

> **Rewritten from Python (Tkinter) to Rust (Slint)** for speed, portability, and a cleaner UX.  
> Original Python project served as the prototype; this repository is the modern Rust implementation.

Stitch is a lightweight desktop utility that lets you **select a precise slice of a codebase** and “stitch” it into a single, shareable text block. It’s designed to work with the **LLM chat interfaces you already use**—pasteable, auditable, and editor-agnostic.

---

## ✨ Highlights

- **Fast, native UI (Slint)** with a responsive tree and larger projects handled more smoothly than the Python/Tk version.
- **Deterministic context packing:** you choose exactly which files/dirs are included and how they’re scrubbed.
- **Powerful filtering:**
  - Include by extension (e.g. `.rs,.toml`)
  - Exclude by extension via a leading `-` (e.g. `-.lock,-.png`)
  - Exclude common dirs/files (e.g. `node_modules`, `target`, `.git`, lockfiles, caches)
- **Two “only” modes:** _Hierarchy Only_ (just the tree) and _Directories Only_.
- **“Select from Text…”**: paste a previously generated tree to auto-reselect the same files—great for bug repros and LLM-guided minimal contexts.
- **Scrubbing tools:** strip lines starting with given prefixes and/or apply a custom **regex** to remove spans before output.
- **Auto refresh:** watches for project changes on a polling interval and regenerates when selected files change.
- **One-click copy** of the final output.

---

## 🆚 What changed from the Python version?

- **Language/GUI:** Python + Tkinter → **Rust + Slint**
- **Performance:** faster directory scanning, smoother UI, better handling of larger trees.
- **UX polish:** richer tree interactions, consistent fonts, and better output formatting.
- **Packaging:** macOS DMG recipe via `cargo-bundle` (see `just dmg`), with cross-platform builds via Cargo.

---

## 🧭 Core Philosophy

- Use the **chat models you already have**—no API keys required.
- **Full control & auditability:** you see and decide what the model sees.
- Curated, minimal context often **outperforms** automatic retrieval for long-tail tasks.

---

## 🚀 Getting Started

### Prerequisites

- **Rust** (latest stable toolchain) and **Cargo**
- macOS users (optional for DMG): `cargo-bundle` (installed automatically by the `just dmg` recipe)

### Run in dev

```bash
cargo run

# or optimized:
cargo run --release
```

### Build a release binary

```bash
cargo build --release
```

### Create a macOS DMG (optional)

```bash
just dmg
# Produces: dist/Stitch-<version>.dmg
```

> macOS note: If, after installing, you see the error
>
>“Stitch” is damaged and can’t be opened.
> You should eject the disk image."
>
>Clear the quarantine attributes:
>
>```bash
>xattr -cr /Applications/Stitch.app
>```
>
>This removes the quarantine flag so the app can launch.

---

## 🖱️ How to Use

1. **Select Folder** – choose the project root.
2. **Adjust Filters** (optional):

   * **Filter Extensions:** comma-separated. Examples:

     * Include only: `.rs,.toml`
     * Exclude some: `-.lock,-.png`
     * Mix (include takes precedence): `.rs,.md,-.lock`
   * **Exclude Directories / Files:** comma-separated names (pre-filled with sensible defaults).
3. **Select Items** – check files or whole directories in the tree. Directory checks cascade to children (you can override specific files).
4. **Choose Mode**:

   * **Hierarchy Only** – emits only the tree.
   * **Directories Only** – tree of selected dirs (no file contents).
5. **Generate Output** – Stitch prints:

   * `=== FILE HIERARCHY ===` (unicode tree)
   * `=== FILE CONTENTS ===` with per-file blocks when not in an “only” mode.
6. **Copy Output** – one click to put everything on your clipboard.

### “Select from Text…” (round-trip selection)

Paste a hierarchy produced by Stitch (first line is the root folder name). Stitch parses it and auto-selects those files, so teams (or an LLM) can propose exactly what to include next run.

---

## 🔧 Scrubbing & Cleanup

* **Remove lines starting with:** Comma-separated prefixes. Lines beginning with any of these (ignoring indentation) are removed; if a prefix appears mid-line after whitespace and a word boundary, the remainder of the line is stripped.
* **Remove regex:** A multi-line, dot-matches-newline regex (we wrap your pattern with `(?ms)` under the hood). Useful to drop regions, banners, or credentials you’ve already sanitized locally.

> ⚠️ Regex/prefix removal is content-agnostic; it won’t parse language syntax. Use carefully to avoid changing semantics if you’re pasting code back into a compiler.

---

## 🧩 Typical Workflows

* **LLM context packing:** curate a minimal set of files and scrub noise before pasting into a chat.
* **Minimal repros:** share only the relevant sources and a tree.
* **Reviews & handoffs:** generate a portable, single-blob snapshot for PRs, issues, or email.
* **LLM-guided selection:** let a model propose a minimal tree; paste it back via “Select from Text…”

---

## 🛠️ Implementation Notes

* **Tech:** Rust, Slint UI, `rfd` (folder dialog), `regex`, `chrono`, `arboard` (clipboard), `pathdiff`, `dunce`.
* **Auto-refresh:** 30s polling interval compares a path snapshot and selected file mtimes; regenerates output when needed.
* **Icons/Fonts:** JetBrains Mono bundled for consistent rendering.

---

## ⚠️ Known Limitations

* **Very large repos:** initial scans can still be heavy. Use filters/exclusions early.
* **Binary/huge files:** not parsed specially—consider excluding or adding size caps in future releases.
* **Regex/prefix stripping:** may remove content inside strings/comments unintentionally; double-check before sharing.

---

## 🗺️ Roadmap (nice-to-haves)

* File-watcher back-end (event-driven instead of polling)
* Optional size/binary detection and skip notices
* `.gitignore`/glob support via `ignore` crate
* Save output to file (in addition to copy)
* CLI/daemon mode to enable IDE/agent integrations
* Token/char counters for budget planning
* Source map for patch application workflows

---

## 🤝 Contributing

Issues and PRs welcome—whether for defaults (exclusions), performance tweaks, UI polish, or new integrations.

---

## 📄 License

This project is licensed under the **MIT License** — see [LICENSE](./LICENSE) for details.

It also bundles third-party assets:

- [JetBrains Mono](https://www.jetbrains.com/lp/mono/) font, licensed under the  
  [SIL Open Font License 1.1](./LICENSES/LICENSE-JetBrainsMono.txt).

All third-party licenses are collected in the [LICENSES/](./LICENSES) folder.