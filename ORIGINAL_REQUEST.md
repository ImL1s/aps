# Original User Request

## 2026-07-30T08:33:04Z

Prepare the `aps` (PlayStation 1 emulator in Rust) repository for open-source release: audit all files to ensure clean git status without secrets or junk, configure tag-triggered GitHub Release workflow (`.github/workflows/release.yml`) for precompiled multi-platform binaries, publish the repository to GitHub, and verify green CI/CD and release artifact generation.

Working directory: /Users/iml1s/Documents/mine/aps
Integrity mode: development

## Requirements

### R1. Git Working Directory Audit & Clean Commit
Inspect all untracked and modified files in the working directory. Ensure temporary files (`.DS_Store`, build artifacts, temporary ROM dumps, secrets) are strictly excluded via `.gitignore`. Perform clean atomic git commits.

### R2. Tag-Triggered Multi-Platform Release Workflow
Create a GitHub Actions Release workflow (`.github/workflows/release.yml`) triggered on tag pushes (`v*`). The workflow must:
1. Build optimized release binaries for Linux (`x86_64-unknown-linux-gnu`), macOS (`x86_64-apple-darwin` / `aarch64-apple-darwin`), and Windows (`x86_64-pc-windows-msvc`).
2. Package binaries into `.tar.gz` and `.zip` archives.
3. Automatically publish a GitHub Release with release notes and attach precompiled binary artifacts.

### R3. GitHub Repository Publishing & Metadata Configuration
Apply repository description and topics using `GH_METADATA.md` or `gh` CLI commands (`gh repo create` / `gh repo edit` / `gh secret`). Push `main` branch and create initial release tag `v0.1.0`.

### R4. End-to-End Verification & Green CI Check
Verify that:
1. Main branch push triggers `.github/workflows/ci.yml` and completes with status **GREEN**.
2. Tag push (`v0.1.0`) triggers `.github/workflows/release.yml` and successfully generates release assets.

## Acceptance Criteria

### Security & Git Audit
- [ ] `git status` is clean with zero sensitive files, temporary dumps, or binary junk committed. `.gitignore` excludes target/ and test ROM binaries.

### CI/CD & Release Workflow
- [ ] `.github/workflows/ci.yml` and `.github/workflows/release.yml` are present, valid YAML, and pass workflow validation.
- [ ] Release workflow automatically packages and attaches multi-platform precompiled binaries (Linux, macOS, Windows) to GitHub Releases.

### GitHub Open-Source Launch & Verification
- [ ] Git repository is published on GitHub with description and topic tags applied.
- [ ] `main` branch CI run is verified **GREEN**.
- [ ] Tag `v0.1.0` triggers build and successfully publishes GitHub Release with compiled release assets.
