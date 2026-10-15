# Softonic-repacked emu8086 4.08 — comparison with the 2010 upstream build

## Verdict

**Same product, payload binary-identical; only documentation and licensing text changed.**
Every executable and every data file in the 2015 Softonic package is byte-for-byte the same as in
the 2010 upstream package — `app/emu8086.exe` (`dba99f7c…`), `diasm.dll`, `MicroAsm.dll`,
`fasm/FASM.EXE`, `BIOS_ROM`, `FLOPPY_0`, `INT_VECT`, `default.binf`, `emu8086.ini`, `version.txt`,
all 84 unchanged example sources, all eight `DEVICES/*.exe` virtual-device programs, all 59 FASM
files, and all eight Microsoft/Oracle runtime files under `sys/`. The emulator's observable
behaviour is therefore identical by construction; nothing in the program can differ. The repack
**does** change the package around the program: 35 text files differ (32 documentation files,
2 readme/license files, 1 example source), 7 files were removed and 1 added, and the Inno Setup
stub's own 141 824-byte PE image differs in exactly 11 bytes (10 in the 44-byte
`RCDATA "11111"` setup-descriptor blob, 1 in `RT_VERSION`). *(observed)*

---

## Artifacts under comparison

| role | path | sha256 | size |
|---|---|---|---|
| 2010 upstream installer | `rea-bins/emu8086.exe` | `175061d9f03a56de25e867fd4aaf16925e6fb03afdfb3b403c05bda49ea716e7` | 3 227 332 B |
| 2010 payload | `re/extracted/` | — | 317 files |
| 2015 archive | `rea-bins/emu8086v408.zip` | `d56d6e42fe170c52df5abd6002b1e8fef0b840eb8d8807d77819fe1fc2e17afd` | 3 131 331 B |
| 2015 installer | `rea-bins/extracted/setup.exe` | `a1b36dc6313f7520f0541f6cdf091d59f75733bd2d7bfd35d21a86964af397db` | 3 220 050 B |
| 2015 payload | `re/extracted-softonic/` | — | 311 files |

The 2015 archive contains exactly two members:

```
$ cat rea-bins/extracted/ReadMe.txt
This is EMU8086 version 4.08
Repack for Windows 10
November 25, 2015
```

75 bytes, sha256 `9e2ace3a2afed8871a0ad4beda5663080e4081fc465ccab6937e1359f5a52dd4`. *(observed)*

---

## Q1 — Is the repacked payload byte-identical to the 2010 payload?

**Answer: every binary is identical; only text documentation differs.**

### Command

```bash
innoextract -d re/extracted-softonic rea-bins/extracted/setup.exe
```

Output: `Done.` — no warnings, no errors. The 2010 extraction behaves identically.

Both installers report identical setup metadata:

```
$ innoextract --info rea-bins/emu8086.exe
Inspecting "emu8086 microprocessor emulator" - setup data version 5.3.7 (unicode)
Languages:
 - default: English
No GOG.com game ID found!
Setup is not passworded!
```

The 2015 installer produces the same six lines, including the same setup data version **5.3.7**. *(observed)*

### Command

A per-file SHA-256 walk of both trees (309 vs 303 files under `app/`, 8 vs 8 under `sys/`) gives:

```
2010 files: 317  2015 files: 311
identical: 275  differing: 35  only-2010: 7  only-2015: 1
```

### Decisive subset — all executable and data artifacts

```
path                               2010 sha256        2015 sha256        same
app/emu8086.exe                    dba99f7c8795df28    dba99f7c8795df28    True
app/diasm.dll                      f9bacea9a5c29876    f9bacea9a5c29876    True
app/MicroAsm.dll                   6424c0a2bae1526d    6424c0a2bae1526d    True
app/fasm/FASM.EXE                  a7cd945a8da8a474    a7cd945a8da8a474    True
app/emu8086.ini                    de5eb64543658011    de5eb64543658011    True
app/BIOS_ROM                       8ff8497d11497710    8ff8497d11497710    True
app/FLOPPY_0                       b41bad70e3c0438d    b41bad70e3c0438d    True
app/INT_VECT                       ad963700f791b353    ad963700f791b353    True
app/version.txt                    fffbda544a37b473    fffbda544a37b473    True
app/default.binf                   e00dd0aeb0550b4b    e00dd0aeb0550b4b    True
sys/msvbvm60.dll                   20cbf8835f6fd387    20cbf8835f6fd387    True
sys/Mscomctl.ocx                   a018a023d59bb76a    a018a023d59bb76a    True
sys/stdole2.tlb                    6ea525bface5467c    6ea525bface5467c    True
sys/oleaut32.dll                   baeb2f7c1b8be567    baeb2f7c1b8be567    True
sys/olepro32.dll                   789c3c45eda1749b    789c3c45eda1749b    True
sys/asycfilt.dll                   de83c9d9203050b4    de83c9d9203050b4    True
sys/comcat.dll                     96d04cdfaf4f4d7b    96d04cdfaf4f4d7b    True
```

The nine virtual-device programs under `app/DEVICES/` (`LED_Display.exe`, `Printer.exe`,
`Robot.exe`, `Simple.exe`, `Stepper_motor.exe`, `Thermometer.exe`, `Traffic_Lights.exe`,
`VGA_STATE.exe`) and the eight `sys/` runtime files are likewise identical. *(observed)*

### Independent confirmation through Ghidra

The 2015 copy of `app/emu8086.exe` was re-imported into a fresh headless project
(`re/ghidra-project-softonic`, project name `emu8086_softonic`) and dumped with the existing
`re/scripts/EmuDump.java`:

```
INFO  EmuDump.java> funcs.tsv: 1538 funciones (GhidraScript)
```

```bash
$ sha256sum re/out/main/funcs.tsv re/out/softonic/main/funcs.tsv
8a4ed1fddf58db243e4b9c992b336b12abc2b332b6caaf6d448f005967b8da20  re/out/main/funcs.tsv
8a4ed1fddf58db243e4b9c992b336b12abc2b332b6caaf6d448f005967b8da20  re/out/softonic/main/funcs.tsv
```

The function inventory — name, entry address, instruction count, xref count and referenced strings
for all 1538 functions — is **byte-identical** to the 2010 analysis. The `strings` dump (4483
records) is identical as well once xref address lists are sorted; the raw files differ in exactly
one record (`00437598 "emu8086"`, 78 xrefs) where Ghidra emitted two xrefs in a different order.
*(observed)*

### File-level differences

275 files are identical and are not listed. The 43 non-identical paths:

| path | 2010 sha256 | 2015 sha256 | status |
|---|---|---|---|
| `app/DEVICES/_READ_ME.txt` | `7096ee663912` | `cc5d7d4ecc5c` | differs |
| `app/ReadMe.txt` | `4f18b81524b7` | `cf53ee7f2002` | differs |
| `app/documentation/8086_bios_and_dos_interrupts.html` | `455c32fb3fa1` | `77cfcc1924dc` | differs |
| `app/documentation/8086_instruction_set.html` | `42efad430428` | `6b5f51375cc0` | differs |
| `app/documentation/_readme.txt` | `638dd203df60` | `af279c632c68` | differs |
| `app/documentation/asm_tutorial_01.html` | `c38e50e5a34e` | `657152b18660` | differs |
| `app/documentation/asm_tutorial_02.html` | `1dd48f4c040b` | `37587e9d214f` | differs |
| `app/documentation/asm_tutorial_03.html` | `510db00d5c70` | `fe682775760b` | differs |
| `app/documentation/asm_tutorial_04.html` | `dd455d85587a` | `73abeae013fc` | differs |
| `app/documentation/asm_tutorial_05.html` | `0b667449ac28` | `50dae72f50bc` | differs |
| `app/documentation/asm_tutorial_06.html` | `e1dc31f8154d` | `1decd42425c2` | differs |
| `app/documentation/asm_tutorial_07.html` | `2f5f73ad4919` | `9e374c982d32` | differs |
| `app/documentation/asm_tutorial_08.html` | `b78f49510b53` | `37581354a58d` | differs |
| `app/documentation/asm_tutorial_09.html` | `f2be9ec4a95c` | `dd82a8fe0683` | differs |
| `app/documentation/asm_tutorial_10.html` | `7a23b13a5a49` | `36edc90a2dde` | differs |
| `app/documentation/asm_tutorial_11.html` | `bbc77d906c7a` | `e1e7905e12ad` | differs |
| `app/documentation/asm_tutorial_12.html` | `9926acc66478` | `75abb7461762` | differs |
| `app/documentation/compatibility.html` | `640b6f676404` | `00129c711c88` | differs |
| `app/documentation/compiler.html` | `1ba83e98caf3` | `e9c94909c7c4` | differs |
| `app/documentation/custom_memory_map.html` | `7c36eb37d604` | `2a55b5bcc5f0` | differs |
| `app/documentation/debug.html` | `f5e398a16d00` | `4405176331a3` | differs |
| `app/documentation/editor.html` | `97b68ba6c077` | `285c83fc10d3` | differs |
| `app/documentation/emulator.html` | `b60e81c46f69` | `ebe0b2ba4225` | differs |
| `app/documentation/help.html` | `e18a8dc8b518` | `ad8493f61aa2` | differs |
| `app/documentation/io.html` | `6a31aa5f9608` | `bd30f90354a7` | differs |
| `app/documentation/memory.html` | `04d35fbb7273` | `0d0ddf897b35` | differs |
| `app/documentation/menu.html` | `7337581263fd` | `48f946ab7d63` | differs |
| `app/documentation/numbering_systems_tutorial.html` | `6ebd9c275e2a` | `9ead8ab55229` | differs |
| `app/documentation/reference.html` | `735bb83ade67` | `c46416f81f32` | differs |
| `app/documentation/register.html` | `a78a8d32e149` | `9c1525fec055` | differs |
| `app/documentation/requirements.html` | `e93f07f76785` | `277028945ec5` | differs |
| `app/documentation/start_unframed.html` | `b488aa8127b5` | `54284b7608f9` | differs |
| `app/documentation/stop_on_condition.html` | `0c977405d063` | `3f0da1022dd3` | differs |
| `app/documentation/tutorials.html` | `544ee7947943` | `0da40f6b0f96` | differs |
| `app/examples/snake.asm` | `b17cd625ae8c` | `153ede52700d` | differs |
| `app/DEVICES/simplest.com` | `b25342a131fe` | — | removed |
| `app/LICENSE.txt` | `f416da5a35cc` | — | removed |
| `app/documentation/img/lock.gif` | `b68c3193a3ab` | — | removed |
| `app/documentation/img/phone.gif` | `c0e9552d49c6` | — | removed |
| `app/documentation/img/phonef.gif` | `afc70c880030` | — | removed |
| `app/documentation/img/star.gif` | `5f45fc4838d8` | — | removed |
| `app/examples/simplest.asm` | `bd27c2acf07f` | — | removed |
| `app/LICENCE.txt` | — | `d3eae9baaac8` | added |

*(observed — generated by a SHA-256 walk over both extracted trees.)*

---

## Q2 — What did the repacker actually change?

### The PE image of the setup stub is 99.992 % identical

Both installers use the **same Inno Setup 5.3.7 loader stub**, byte-for-byte except 11 bytes.

```bash
$ python3  # compare bytes 0..0x22a00 (the whole PE image)
2010 stub bytes 0..0x22a00 sha256: 515b8de8b5b649becd16036beec90feb7c0bbb42a948bc4ada83cab38b166a63
2015 stub bytes 0..0x22a00 sha256: 865f54fa723d64cfecd563ddb6a04fe069f4126fa861d8a1387a7c64955f71a5
first diff offset: 0x21f44  num differing bytes: 11
```

Byte offsets of every difference inside the PE image:

```
0x21f44 0x21f45 0x21f48 0x21f49 0x21f54 0x21f55 0x21f5c 0x21f5d 0x21f5e 0x21f5f 0x2227a
```

*(observed.)*

### Structural comparison

| property | 2010 `emu8086.exe` | 2015 `setup.exe` | verdict |
|---|---|---|---|
| PE size (image + overlay) | 3 227 332 B | 3 220 050 B | differs (payload) |
| `e_lfanew` / PE signature | `0x100` / `PE\0\0` | `0x100` / `PE\0\0` | same |
| Machine | `0x014c` (i386) | `0x014c` | same |
| COFF timestamp | `0x4b445e48` = 2010-01-06 09:56:24 UTC | `0x4b445e48` = 2010-01-06 09:56:24 UTC | **same** |
| Sections | 9 | 9 | same |
| Optional-header magic | `0x010b` (PE32) | `0x010b` | same |
| Linker version | **2.25** | **2.25** | same |
| Characteristics | `0x818f` | `0x818f` | same |
| Import directory | RVA `0x1e000`, 3998 B | RVA `0x1e000`, 3998 B | same |
| Resource directory | RVA `0x23000`, 45 568 B | RVA `0x23000`, 45 568 B | same |
| TLS / IAT entries | `0x20000`/24, `0x1e350`/588 | `0x20000`/24, `0x1e350`/588 | same |
| Overlay offset / magic | `0x22a00` / `zlb\x1a` | `0x22a00` / `zlb\x1a` | same |
| Overlay size | 3 085 508 B | 3 078 226 B | differs |

Section table, identical in both (`.text` `0x1000`/0x14618, `.itext` `0x16000`, `.data` `0x17000`,
`.bss` `0x18000`, `.idata` `0x1e000`, `.tls` `0x1f000`, `.rdata` `0x20000`, `.reloc` `0x21000`,
`.rsrc` `0x23000`/0xb200 at raw pointer `0x17800`). *(observed.)*

Import table parsed from both files is element-for-element identical — 12 import descriptors naming
`oleaut32.dll`, `advapi32.dll`, `user32.dll`, `kernel32.dll` and `comctl32.dll`, with identical
per-descriptor function counts (3/3/4/32/4/13/60/5/1/1/1/8). *(observed.)*

### The 11 changed bytes, located

10 of them fall inside the 44-byte `RCDATA "11111"` resource at file offset `0x21f34`; 1 falls
inside `RT_VERSION` at `0x2227a`.

`RCDATA "11111"` raw bytes:

```
2010  72 44 6c 50 74 53 cd e6 d7 7b 0b 2a | 01 00 00 00 | c4 3e 31 00 | 25 96 2b 00 |
      00 b0 11 00 | f3 d0 ce 0b | 20 b6 2a 00 00 2a 02 00 | 9a 5f a9 25
2015  72 44 6c 50 74 53 cd e6 d7 7b 0b 2a | 01 00 00 00 | 52 22 31 00 | b3 79 2b 00 |
      00 b0 11 00 | f3 d0 ce 0b | d7 99 2a 00 00 2a 02 00 | 3c 15 9c d7
```

The first 12 bytes are the ASCII tag `rDlPtS` followed by six fixed bytes — the Inno Setup loader
identifier — and are unchanged. Fields 16–19 and 20–23 are the loader's `TotalSize` and `OffsetEXE`;
`TotalSize` decodes to 3 227 332 and 3 220 050 respectively, which are **exactly** the two installer
file sizes, confirming the interpretation. `UncompressedSizeEXE` (`0x0011b000`) and `CRCEXE`
(`0x0bced0f3`) are identical in both. The remaining 6 changed bytes at blob offsets 32–33 and
40–43 are **unknown** — not identified. *(observed for the byte positions and the `TotalSize`
identity; unknown for the semantics of the remaining fields.)*

`RT_VERSION` (1208 B at `0x21fa0`) differs in exactly one byte, inside `LegalCopyright`:

```
2010  ... LegalCopyright  "Copyright  2010"
2015  ... LegalCopyright  "Copyright  2015"
```

Everything else in the version resource is identical: `dwFileVersion` and `dwProductVersion` are
`0.0.0.0` in both, `FileVersion` and `ProductVersion` strings are blank in both, and
`CompanyName` = `emu8086`, `FileDescription` = `emu8086 microprocessor emulator Setup`,
`ProductName` = `emu8086 microprocessor emulator`, `Comments` = `This installation was built with
Inno Setup.` in both. *(observed.)*

### Manifest — no Windows 10 compatibility was added

`RT_MANIFEST` (1376 B at `0x22458`) is **byte-identical** in both installers. Its content is the
stock Inno Setup 5.3.7 manifest:

```xml
<requestedExecutionLevel level="asInvoker" uiAccess="false"/>
...
<compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
  <application>
    <supportedOS Id="{e2011457-1546-43c5-a5fe-008deee3d3f0}"/>
    <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}"/>
  </application>
</compatibility>
```

Exactly two `supportedOS` entries are present, both predating Windows 8 (Windows Vista and Windows
7). No Windows 8, 8.1 or 10 GUID is declared, and `dpiAware` is the only `windowsSettings` entry.
*(observed — the byte range is identical in both files; the GUID→OS mapping is a documented public
constant.)*

**This is the single most important negative result of the comparison: the "Repack for Windows 10"
claim in the archive readme is not backed by any manifest, resource, linker-version or binary change.**
No compatibility shim, no newer loader, no `asInvoker`→`requireAdministrator` change. *(observed for
the byte facts; the conclusion that no Windows 10 support mechanism was added follows directly.)*

### Files added or removed by the installer

`innoextract --list` on both installers reproduces exactly the 7 removals, 1 addition and 35
modifications found by hashing the extracted trees; no file exists in one installer's manifest and
not the other. Both installers list `app/ReadMe.txt` twice — once flagged `- overwritten`, once as a
payload entry — and the only difference on those two lines is the recorded size, `1.89 KiB` in 2010
against `1.78 KiB` in 2015. *(observed.)*

### No Softonic branding anywhere

`grep -ril softonic` over `re/extracted-softonic/` and over `re/extracted/` returns nothing.
`strings -el` over `setup.exe` yields only the stub's own `emu8086` version strings. The
`MAINICON` icon group (62 B at `0x21f60`) and all icon resources are byte-identical between the two
installers. *(observed.)*

---

## Q3 — Trust and provenance

### Authenticode

PE data-directory entry 4 (Certificate/Security) is zeroed in every artifact examined:

```
rea-bins/emu8086.exe          Entry 4 00000000 00000000 Security Directory
rea-bins/extracted/setup.exe  Entry 4 00000000 00000000 Security Directory
re/extracted/app/emu8086.exe  Entry 4 00000000 00000000 Security Directory
re/extracted/app/diasm.dll    Entry 4 00000000 00000000 Security Directory
re/extracted/app/MicroAsm.dll Entry 4 00000000 00000000 Security Directory
re/extracted/app/fasm/FASM.EXE Entry 4 00000000 00000000 Security Directory
```

**Neither the 2010 nor the 2015 installer, nor any payload binary, carries an Authenticode
signature.** This is an observation about the files. It is not a finding that the 2015 package is
malicious, and it is not a finding that the 2010 package is trustworthy — neither package offers
any machine-verifiable publisher identity. *(observed for the absence; unknown for why.)*

### Version resources

Covered in Q2. Both stubs declare `CompanyName` = `emu8086` (not Softonic, not Digital River), a
blank `FileVersion`, and `LegalCopyright` differing only by year. The payload `app/emu8086.exe` is
byte-identical in both packages, so its own version resources are also identical. *(observed.)*

### Bundled extra payloads

The `sys/` directory is identical in both packages — the eight files `msvbvm60.dll`, `Mscomctl.ocx`,
`cmax20.ocx`, `oleaut32.dll`, `olepro32.dll`, `asycfilt.dll`, `comcat.dll`, `stdole2.tlb` are the
same Microsoft VB6 / Oracle runtime set, with the same hashes. No file was added to the 2015
package that is not also in the 2010 package. *(observed.)*

The only file added anywhere is `app/LICENCE.txt`, and it replaces `app/LICENSE.txt` rather than
augmenting it.

### What the evidence supports, and what it does not

| claim | label |
|---|---|
| No Authenticode signature on either installer or on any payload binary | **observed** |
| No file present in one package and absent from the other, beyond the 7 + 1 listed | **observed** |
| The 2015 installer's stub is the same 2010-era loader, edited in 11 bytes | **observed** |
| The 2015 payload came from an emu8086.com distribution refreshed in Oct–Nov 2015 | **inferred** (see New findings) |
| The 2015 package was produced by Digital River / a reseller rather than by Softonic's own toolchain | **inferred**, strongly supported by the `mycommerce.com` and `dr.com` evidence |
| Whether Softonic or the vendor produced each of the 35 text edits | **unknown** |
| Whether the four removed GIFs and two removed example files were dropped by the vendor or by the repacker | **unknown** |
| Whether the package is safe to run | **unknown** — no dynamic analysis was performed, and none was requested |

---

## Q4 — Third-party redistribution risk

### What is bundled

FASM 1.64 ships complete inside the payload at `app/fasm/` in **both** packages:

```bash
$ diff -rq re/extracted/app/fasm re/extracted-softonic/app/fasm && echo IDENTICAL
fasm tree IDENTICAL
$ find re/extracted/app/fasm -type f | wc -l
59
```

59 files, all byte-identical: `FASM.EXE` (`a7cd945a8da8a474…`, dated 2005-10-17), `LICENSE.TXT`,
`URL.txt`, and the 56-file `INCLUDE/` tree. *(observed.)*

**The repack changed nothing about what FASM files are bundled.** No FASM file was added, removed
or modified. The only FASM-adjacent text change is the spelling of the vendored licence copy:
`app/LICENSE.txt` → `app/LICENCE.txt`, with the heading `FASM - FLAT ASSEMBLER LICENSE` rewritten
as `FASM - FLAT ASSEMBLER LICENCE`. The licence body itself is unchanged. *(observed.)*

### The obligation

The vendored FASM licence (quoted verbatim from `re/extracted-softonic/app/LICENCE.txt`) is the
standard two-clause BSD-style text:

> Copyright remains Tomasz Grysztar, and as such any Copyright notices in the code are not to be
> removed.
> 1. Redistributions of source code must retain the above copyright notice, this list of
> conditions and the following disclaimer.
> 2. Redistributions in binary form must reproduce the above copyright notice, this list of
> conditions and the following disclaimer in the documentation and/or other materials provided
> with the distribution.

For a clean-room FOSS reimplementation (`crates/`) this is the **non-blocking** part of the
redistribution question: `emu8067` is a reimplementation, not a derivative work, so no FASM code is
carried over and clauses 1 and 2 do not bind it. FASM may be used as a reference assembler, or
dropped entirely — the emulator's own assembler is the thing being reimplemented. *(inferred from
the licence text and from the nature of a clean-room reimplementation.)*

What **does** bind any redistributor of a derived work — anything that ships `emu8086.exe`,
`diasm.dll`, `MicroAsm.dll` or the `fasm/` tree itself — is the emu8086 shareware licence, not
FASM's. `re/extracted-softonic/app/LICENCE.txt` limits evaluation to 14 days and routes purchase
through `https://shopper.mycommerce.com/checkout/product/2189-{7,10,9}`. A redistributor must
retain FASM's copyright notice and conditions in whatever documentation accompanies the binary
(clause 2), and must not remove copyright notices from the FASM code itself. *(observed for the
licence text; inferred for the application to a derived work.)*

Note that `emu8086.ini`, `BIOS_ROM`, `FLOPPY_0`, `INT_VECT` and `default.binf` are also
byte-identical between the two packages, so neither repack introduced a new data-file licensing
question. *(observed.)*

---

## Q5 — Anything genuinely new?

**Nothing new in the program. Several new things in the packaging.** Below is the evidence that
establishes this.

### Evidence that the program is unchanged

1. `app/emu8086.exe` has sha256 `dba99f7c8795df28be4b153f8e979b0c572ef825d8b70fb779609f8303a055e9`
   in both packages. A behavioural difference in the emulator is therefore impossible.
2. `diasm.dll`, `MicroAsm.dll`, `fasm/FASM.EXE`, `emu8086.ini`, `BIOS_ROM`, `FLOPPY_0`, `INT_VECT`,
   `default.binf`, `version.txt` and all eight `sys/` runtimes are identical.
3. All 1538 recovered functions, their addresses, sizes, xref counts and referenced strings are
   byte-identical between the 2010 Ghidra dump and a fresh 2026 dump of the 2015 copy.
4. The 4483-entry string table is identical once xref ordering is normalised.

Consequently, every behavioural contract, string, opcode table, diagnostic message and assembler
rule recorded in `re/FINDINGS.md` still holds without amendment. `re/FINDINGS.md` needs **no
correction** as a result of this analysis.

### New observations about the packaging

| # | observation | label |
|---|---|---|
| 1 | The 2015 stub's `RT_VERSION` `LegalCopyright` reads `Copyright  2015`, not `2010`; every other version field is blank or unchanged. | observed |
| 2 | The 44-byte `RCDATA "11111"` loader descriptor changed in 10 bytes, including `TotalSize` and `OffsetEXE`. | observed |
| 3 | The manifest is unchanged and declares only Vista + Windows 7 `supportedOS` GUIDs; `requestedExecutionLevel` is `asInvoker` in both. | observed |
| 4 | `app/LICENSE.txt` was renamed to `app/LICENCE.txt` and rewritten: RegNow / Digital River checkout URLs and explicit prices (`USD 19.95 / 295 / 1250`) replaced by bare `https://shopper.mycommerce.com/checkout/product/2189-{7,10,9}` links; "All prices are without VAT. Money back guarantee." dropped; `LICENSE` → `LICENCE` spelling throughout. | observed |
| 5 | `app/ReadMe.txt` replaces the named individual contact (name, street address, Haifa postcode, direct telephone number, `info@emu8086.com`) with `air.plane@dr.com` and `http://www.emu8086.com` under a heading changed from `CONTACT` to `CUSTOMER SERVICE`, and the copyright line from 2010 to 2015. | observed |
| 6 | `app/documentation/register.html` was rewritten as a pure `EMU8086 LICENCING` purchase page; `1 Year Site Licence` became `1 Year School Licence`, all prices removed. | observed |
| 7 | `app/documentation/asm_tutorial_11.html` — "making your own operating system", an 8285 B walkthrough of writing 512 bytes at `0000h:7C00h` and driving `FLOPPY_0` (22 lines mention floppy or boot sector) — was replaced by a 656 B stub reading "This tutorial is unpublished." with a `mailto:air.plane@dr.com` contact link. | observed |
| 8 | 31 of the 35 documentation HTML pages were re-saved (plus `documentation/_readme.txt`): inline `<style>` blocks stripped, `<body bgcolor text link vlink alink>` reduced to bare `<body …>`, sentence case normalised, and a UTF-8 BOM added to 17 of the 35 changed files (none of the 35 had one in 2010). After stripping markup, style and comments and lower-casing, **16 of those 31 pages have byte-identical text**; of the other 15, eleven are ≥ 0.95 similar and four are not: `asm_tutorial_11.html` (0.06), `register.html` (0.56), `help.html` (0.90), `emulator.html` (0.93). The 4 untouched pages are `_documentation_index.html`, `asm_tutorial_end.html`, `index.html`, `start.html`. | observed |
| 9 | `app/examples/simplest.asm`, `app/DEVICES/simplest.com` and four documentation images (`lock.gif`, `phone.gif`, `phonef.gif`, `star.gif`) were removed. The 2010 `help.html` was the only page referencing the GIFs, and it no longer does. | observed |
| 10 | `app/examples/snake.asm` goes from 135 code lines to 130; the only change is the 13-line `msg` string block, shortened to 5 lines and re-cased. The 120 executable instruction lines are unchanged. | observed |
| 11 | `app/DEVICES/_READ_ME.txt` was edited to delete the `Thermometer.exe` line from the device list — but `Thermometer.exe` is still present in the 2015 payload with the same hash as in 2010. The readme and the payload now disagree. | observed |
| 12 | `app/documentation/io.html` still points at `examples/simplest.asm`, which no longer exists in the package. `app/version.txt` (identical in both) also still references it. | observed |

### File modification timestamps — who touched what

Grouping the 2015 files by mtime separates two editing passes:

| 2015 mtime window | files | what changed |
|---|---|---|
| 2015-10-19 14:58 → 2015-10-20 09:07 | **25 differing files**: `ReadMe.txt`, `documentation/_readme.txt`, `asm_tutorial_01/03/04/05/06/07/08/09/10/11/12.html` (11 pages), `compatibility.html`, `compiler.html`, `debug.html`, `editor.html`, `menu.html`, `numbering_systems_tutorial.html`, `reference.html`, `register.html`, `requirements.html`, `start_unframed.html`, `stop_on_condition.html`, `tutorials.html` — plus the single **added** file `LICENCE.txt` (2015-10-19 15:10) | licensing URLs, 2015 copyright, `dr.com` contact, tutorial 11 withdrawal, markup re-save |
| 2015-11-24 23:48 → 2015-11-25 00:15 (27 minutes) | **10 files**: `asm_tutorial_02.html`, `8086_instruction_set.html`, `io.html`, `memory.html`, `custom_memory_map.html` (all 24 Nov); `8086_bios_and_dos_interrupts.html`, `help.html`, `emulator.html`, `DEVICES/_READ_ME.txt`, `examples/snake.asm` (all 25 Nov) | second markup pass, snake message rewrite, `Thermometer.exe` line deletion |

25 + 10 = 35, matching the differing-file count exactly; `LICENCE.txt` is the one added file and
sits in the first window. *(observed — mtimes as preserved by `innoextract`.)*

The second window ends at `00:15:58 -0600`, the same calendar day as the `ReadMe.txt` that names
the repack date. It is **inferred** — not observed — that the second pass belongs to the repack
build rather than to the vendor's own release process; a vendor build on the same day would look
identical. The `dr.com` / `mycommerce.com` evidence makes it **inferred** that Digital River, the
same company already named in the 2010 `LICENSE.txt` ("purchase digital licence key from RegNow/Digital
River company"), produced the 2015 licensing text. Which of the 43 changes each party made is
**unknown**.

---

## New findings

These are additions to `re/FINDINGS.md`'s coverage, all from packaging rather than from the
program:

1. **The Softonic repack carries no code change of any kind.** The 2015 `app/emu8086.exe` is
   byte-identical to the 2010 one, and a fresh Ghidra run over the 2015 copy reproduces the 2010
   function and string inventories byte-for-byte. Anyone who already has the 2010 analysis needs no
   new reverse engineering work from this package.
2. **"Repack for Windows 10" is not implemented anywhere in the package.** The PE manifest still
   declares only Windows Vista and Windows 7 as supported, the loader is still the 2010 build with
   timestamp `0x4b445e48` and linker version 2.25, and `requestedExecutionLevel` is still
   `asInvoker`. Whatever the repack label means, it is not a compatibility change in the executable.
3. **The 2015 licensing and contact text is Digital River's, not Softonic's and not emu8086.com's.**
   Purchase URLs moved from `regnow.com/softsell/nph-softsell.cgi` to
   `shopper.mycommerce.com/checkout/product/`, the developer contact was replaced by
   `air.plane@dr.com`, and the vendor's named office address and telephone number were removed from
   `ReadMe.txt`. Any purchase path derived from this package routes through Digital River.
4. **Two new documentation inconsistencies are present in the 2015 package.**
   `app/DEVICES/_READ_ME.txt` no longer lists `Thermometer.exe` although that executable is still
   installed, and `app/documentation/io.html` still directs the reader to `examples/simplest.asm`
   which is no longer installed.
5. **Tutorial 11 was withdrawn between the two packages.** The "making your own operating system"
   tutorial — the boot-sector walkthrough that exercises `FLOPPY_0` and `#make_boot#` — is replaced by
   an "unpublished" placeholder. `re/FINDINGS.md` records the `FLOPPY_0` image and the `#make_boot#`
   directive but not this tutorial's prose, so no existing finding is invalidated; anyone planning
   `emu8067` documentation to mirror it should know the source is now a stub.

---

## Limits of this analysis

1. **Static only.** Nothing in either package was executed. Claims about behaviour rest on the
   byte-identity of the binaries, which is sufficient for "the behaviour is identical" but says
   nothing about what the program does at runtime.
2. **No `[Run]`, `[Registry]`, `[Tasks]` or `[Code]` section comparison.** `innoextract` does not
   expose the compiled Inno Setup script, and the `zlb` overlay is LZMA-compressed. Whether the two
   installers register the same autostart entries, shortcuts, file associations or uninstall
   behaviour is **unknown**. This is the most significant gap: an installer is more than its
   payload, and only the payload was compared.
3. **Six bytes of `RCDATA "11111"` are unidentified.** Their position is known; their meaning is
   not.
4. **Attribution of individual text edits is unresolved.** Timestamps show two editing windows but
   cannot identify the author of any specific change.
5. **One record in the Ghidra string dump is order-unstable.** The single `00437598 "emu8086"`
   record enumerates 78 xrefs in a different order between runs. The xref *set* is identical;
   Ghidra's enumeration order is not deterministic. This is a property of the tool, not a
   difference between the packages.
6. **The REA MCP Ghidra provider was unavailable.** `binary_overview` returned
   `provider_unavailable` with `GHIDRA_INSTALL_DIR is not set` inside the server process, and
   remediation advice "do not fight it". All deep analysis was therefore performed with Ghidra
   12.1.2 headless through the shell. The target-free `inspect_pe_resources` operation *did* work
   and was used on `setup.exe` (Evidence `ev_8936a15d4ad4c6ce02c9e780731b8fbf92d592f0c158e56fa22fe4157f8afcef`,
   provider `rea-pe-resources`), confirming the resource-directory parse reported above.
7. **The VB6 decompilation was not repeated.** Because the input hash matches the already analysed
   2010 binary, re-deriving the VB6 name table and string tables would reproduce existing artefacts;
   only the function and string inventories were regenerated as an independent check.
8. **`re/ghidra-project` was not touched.** All new Ghidra state went to `re/ghidra-project-softonic`
   and `re/out/softonic/`.