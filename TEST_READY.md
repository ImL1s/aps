# Test Infrastructure Readiness Certification (`TEST_READY.md`)

## Certification Status
- **Status**: READY FOR VERIFICATION
- **Date**: 2026-07-30
- **Track**: E2E Testing Track (`sub_orch_e2e`)

## Verification Checklist
- [x] `TEST_INFRA.md` published at project root covering test philosophy and feature mapping matrix.
- [x] `scripts/run_ps1_tests.sh` published with executable permissions (`chmod +x`), automated zip downloading (`psxtest_cpu.zip` from canonical Amidog server), extraction to `psxtest_cpu.exe`, offline fallback, BSD-compliant `mktemp` template format (`LOG_FILE=$(mktemp "${TMPDIR:-/tmp}/psx_amidog_test_XXXXXX")`), cross-platform timeout, and clean exit code semantics.
- [x] `crates/ps_core/Cargo.toml` correctly registers all 4 test targets (`tier1_unit_tests`, `tier2_boundary_tests`, `tier3_integration_tests`, `tier4_rom_tests`).
- [x] `crates/ps_core/src/bus/mock_bus.rs` memory mock engine defined.
- [x] Tier 1 Unit Test suite created (`tests/tier1_unit_tests.rs`) with synchronized `next_pc`.
- [x] Tier 2 Boundary Test suite created (`tests/tier2_boundary_tests.rs`) aligned with exported `ps_core` types.
- [x] Tier 3 Integration Test suite created (`tests/tier3_integration_tests.rs`) using concrete `MemoryBus`, `Cpu`, `Ram`, `Bios`, `Scratchpad` and correct MIPS 2-stage load delay timing.
- [x] Tier 4 ROM & E2E Test suite created (`tests/tier4_rom_tests.rs`) using concrete `MemoryBus`, `Cpu`, `Bios`.
- [x] All 20 features from `PROJECT.md` mapped to verification test tiers.

## Verification Commands
```bash
# 1. Run all 4 cargo test tiers (27 tests across 4 tiers 100% passing: Tier 1: 13, Tier 2: 8, Tier 3: 5, Tier 4: 1)
LIBRARY_PATH=/opt/homebrew/lib cargo test

# 2. Test script command line interface & options
./scripts/run_ps1_tests.sh --help

# 3. Test ROM zip download, extraction, and cache verification
./scripts/run_ps1_tests.sh --download-only
./scripts/run_ps1_tests.sh --force-download --download-only

# 4. Execute full test ROM harness run (requires release binary)
./scripts/run_ps1_tests.sh
```
