# Tooling notes for emu8067

Environment facts that cost time to discover, and the correction that came out of them.

## Ghidra on this machine

Installed as the `ghidra` package `12.1.2-2.1`, from `cachyos-extra-znver4`
(built 2026-06-29, installed 2026-10-15). The application tree is `/opt/ghidra`
(742 MB); `/usr/bin` holds three symlinks that the PKGBUILD packages as real
entries — there is no `.INSTALL` script and no hook, so nothing generates them at
install time:

| `/usr/bin` entry | Target | Native name |
|---|---|---|
| `ghidra` | `/opt/ghidra/ghidraRun` | GUI launcher |
| `ghidra-analyzeHeadless` | `/opt/ghidra/support/analyzeHeadless` | headless analyzer |
| `pyghidra` | `/opt/ghidra/support/pyghidraRun` | Python scripting host |

Symlinks rather than copies, so that upgrading Ghidra moves all three entry
points at once and none is left pointing at a previous version. The
`ghidra-` prefix on `analyzeHeadless` follows the FSH rule for `/usr/bin`:
unprefixed, that name is too generic to claim ownership of.

**Use `ghidra-analyzeHeadless`, not the full path.** Verify with
`pacman -Ql ghidra | grep usr/bin`.

### `GHIDRA_INSTALL_DIR` is not needed and does not help

Verified empirically on 2026-10-15: importing a small `.COM` under
`env -u GHIDRA_INSTALL_DIR ghidra-analyzeHeadless …` succeeds
(`INFO REPORT: Import succeeded`). Ghidra's launch scripts locate their own
installation from their own path; they do not read that variable.

The variable was exported in this project because the `rea` MCP server reported:

```
provider_unavailable → "Set GHIDRA_INSTALL_DIR to an extracted Ghidra 12.1.x release directory."
checks: configuration → not set | installation → executable_missing
        headless → executable_missing | native_decompiler → executable_missing
```

Exporting it in a shell cannot reach an already-running MCP server, so REA
stayed on `provider_unavailable` for the whole session and every analysis went
through Ghidra headless directly. Treat REA's Ghidra provider as unavailable
and plan for the headless path. The target-free REA tools
(`inspect_pe_resources`, `inspect_artifact`) do work.

### Java

Resolved from `PATH` (OpenJDK 27, `/usr/lib/jvm/java-27-openjdk`). Ghidra 12.1
requires a 64-bit full JDK 21 or newer, so `java-environment>=21` is satisfied.
No `JAVA_HOME` needed.

## Correction: the `analyzeHeadless` claim

On 2026-10-15 this project asserted that `analyzeHeadless` was absent from
`PATH` and that only `/opt/ghidra/support/analyzeHeadless` existed. **That was
wrong.** The package installs it under the prefixed name `ghidra-analyzeHeadless`;
the earlier check only searched for the unprefixed spelling. The user caught it
with `which`.

Lesson: when `which <name>` fails, check the packaged prefix
(`pacman -Ql <pkg>`, `pacman -Fo`) before concluding that something is not
installed, and before asserting the absence as a fact. An unverified negative
becomes a repeated claim that other work then gets built on.

## Other environment facts

| Tool | Version | Note |
|---|---|---|
| `innoextract` | 1.9 | needed to unpack the reference installer; handles Inno Setup up to 6.0.5 |
| `podman` | 6.1.4 | rootless, `crun`, `overlay`; used to isolate the Windows runtime for differential testing |
| `rustc` / `cargo` | 1.99.0 | workspace targets edition 2024, `rust-version = 1.85` |
| `zig` | 0.16.0 | present but not used; the stack decision was Rust |
| CodeGraph | initialised | `.codegraph/codegraph.db`; use `codegraph_explore` ahead of grep/Read. Gitignored. |

`python3` is present without `pefile` and without `capstone`. PE parsing in this
project is done with a small inline `struct.unpack_from` script plus `objdump`.