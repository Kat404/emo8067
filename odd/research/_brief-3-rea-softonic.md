# Brief — Track 3: REA of the Softonic-repacked emu8086 4.08

You are a reverse-engineering analyst on **emu8067**, a clean-room FOSS Intel 8086 emulator at `/home/josel/Proyectos/emo8067`. Read this brief fully before acting; it replaces any prior assumption about scope.

**Deliverable:** `odd/research/softonic-build-comparison.md` (English, neutral register, no persona).

## Skills to load before work
- Native `skill` tool, id `reverse-engineer-anything`, then read its `references/native-and-artifacts.md`. Skip if already loaded.

## Clean-room rule (non-negotiable)
Reverse engineering is used **only** to establish observable behaviour and to write specifications. Never copy or transliterate decompiled code. Everything you write is an observation or a specification, never transliterated source.

## Existing context — do not re-derive, do not contradict
Read `re/FINDINGS.md` FIRST and build on it. Do not repeat its content; extend and cross-check it.

The 2010 upstream build (`emu8086.exe`, Inno Setup 5.3.7, sha256 `175061d9f03a56de25e867fd4aaf16925e6fb03afdfb3b403c05bda49ea716e7`) extracts to `re/extracted/`:
`app/emu8086.exe` (Visual Basic 6 native-compiled, sha256 `dba99f7c8795df28be4b153f8e979b0c572ef825d8b70fb779609f8303a055e9`, dated 2008-12-21), `diasm.dll` (MSVC), `MicroAsm.dll` (Delphi), `fasm/`, `inc/`, `documentation/`, 86 `examples/`, `BIOS_ROM`, `FLOPPY_0`, `emu8086.ini`, `version.txt` (reports 4.08), `LICENSE.txt`.

## Targets
1. `rea-bins/emu8086v408.zip` — sha256 `d56d6e42fe170c52df5abd6002b1e8fef0b840eb8d8807d77819fe1fc2e17afd`. Already extracted to `rea-bins/extracted/`. Its 75-byte `ReadMe.txt` says: "This is EMU8086 version 4.08 / Repack for Windows 10 / November 25, 2015".
2. `rea-bins/extracted/setup.exe` — sha256 `a1b36dc6313f7520f0541f6cdf091d59f75733bd2d7bfd35d21a86964af397db`, 3 220 050 bytes, Inno Setup 5.3.7 with a `zlb\x1a` overlay, dated 2015-11-25.
3. Compare against the already-extracted 2010 payload in `re/extracted/`.

## Tools
- `innoextract 1.9` is installed. Extract the Softonic payload to `re/extracted-softonic/`. Do NOT modify `rea-bins/`.
- Ghidra 12.1.2 headless. Export `GHIDRA_INSTALL_DIR=/opt/ghidra`, then:
  `/opt/ghidra/support/analyzeHeadless <projectDir> <name> -import <file> -processor x86:LE:32:default ...`
  Use a NEW project dir `re/ghidra-project-softonic` so the existing `re/ghidra-project` is untouched. Dumps go to `re/out/softonic/`. Reuse `re/scripts/EmuDump.java` with `-scriptPath re/scripts -postScript`; modes are `funcs`, `strings`, `decomp <regex> [n]`, `top <n>`, `callees <regex>`.
- `rea` MCP server, tools under `tools.rea.*` (`open_binary`, `binary_overview`, `inspect_artifact`, `inspect_pe_resources`). If it returns `provider_unavailable` because `GHIDRA_INSTALL_DIR` is unset inside the server process, do NOT fight it — fall back to Ghidra headless via the shell and say so in the report.
- Shell: `file`, `objdump`, `strings`, `sha256sum`, `python3` (no `pefile`/`capstone` installed).

## Questions, in priority order
1. **Is the repacked payload byte-identical to the 2010 payload?** Extract both and compare per-file sha256. Report exactly which files differ, are missing, or were added. This is the single most important question.
2. **What did the repacker actually change?** Compare the Inno Setup stubs: section table, import table, manifest (`requestedExecutionLevel`, Windows 10 compatibility hints), resource section, PE timestamp, linker version, and any added or removed files.
3. **Trust and provenance.** Look for Authenticode signatures (PE Security data directory entry 4), version resources (`FileVersion`, `ProductName`, `CompanyName`), and any bundled extra payloads. State what evidence exists and what does not. A missing signature is an observation, not a verdict.
4. **Third-party redistribution risk.** FASM 1.64 ships inside the payload under its own license. State whether this repack changed what is bundled, and what a redistributor of a derived work would have to do about it.
5. **Anything genuinely new** — behaviours, files, strings or version differences absent from `re/FINDINGS.md`. If the answer is "nothing new", say so explicitly and show the evidence that establishes it.

## Output contract for `odd/research/softonic-build-comparison.md`
- One-paragraph verdict up front: same product or not, and does the repack change the payload?
- Table of file-level differences: path | 2010 sha256 (short) | 2015 sha256 (short) | same/diff/added/removed.
- One section per question, each citing the concrete command run and its observed output.
- Explicit `## New findings` and `## Limits of this analysis` sections.
- Label every claim **observed** (with evidence), **inferred**, or **unknown**. Never dress an inference as an observation.

## Allowed edit surfaces
odd/research/softonic-build-comparison.md
re/extracted-softonic/
re/ghidra-project-softonic/
re/out/softonic/
rea-bins/extracted/
re/scripts/EmuDump.java
The section ends here.

## Rules
- English, neutral register, no jargon, no persona. Technical artifact, not a chat reply.
- Do not touch `crates/` or `Cargo.toml`; `cargo` is unrelated here.
- Bound shell output: counts, `--stat`, `tail`, `diff | head` over dumping whole files.
- If something fails, say so plainly and continue with what works. Never invent findings.

If you make important discoveries, decisions, or fix bugs, save them to engram via `mem_save` with project: 'emo8067'.

After your normal result envelope, close your final response text with a `## Key Learnings` block of 1-5 numbered items, each a standalone factual sentence of at least 20 characters and at least 4 words, omitting the block only if there is genuinely nothing reusable to report.