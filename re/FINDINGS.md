# RE findings — emu8086 v4.08

Target chain:

| Artifact | sha256 / size | Identity |
|---|---|---|
| `emu8086.exe` (input) | `175061d9…16e7`, 3 227 332 B | **Inno Setup 5.3.7 installer**, Delphi stub, 2010-01-06. Overlay `zlb\x1a` at file offset `0x22a00`, 3 085 508 B. |
| `re/extracted/app/emu8086.exe` | `dba99f7c…55e9`, 2 244 608 B | **Real target.** PE32 GUI i386, 2008-12-21, linker 6.0, `MSVBVM60.DLL` → **Visual Basic 6 native-compiled**, 3 sections, `.text` 2 225 868 B. Not packed. |
| `diasm.dll` | 98 304 B | **Microsoft Visual C++**, © emu8086.com. Native disassembler core. |
| `MicroAsm.dll` | 9 216 B | Delphi, single export `Copyright_emu8086`. |
| `fasm/FASM.EXE` | 75 264 B | FASM 1.64 — **permissively licensed**, reusable, must not be relicensed. |

## License status of the original

Proprietary shareware: 14-day trial, `USD 19.95` personal / `USD 295` site / `USD 1250` permanent, © EMU8086.COM (Haifa, Israel). Credits *"Portions Copyright 1997-2005 **Barry Allyn**"* — the E8086 author — plus FASM 1.64. No legitimate FOSS port exists; `n3r0-b1n4ry/emu8086` on GitHub is a pirated re-upload with `All rights reserved` and no license.

## Complete module map (VB6 name table @ `0x004370c0`)

Recovered verbatim from the binary's class-name table. This is the software architecture.

**Forms (37):** `frmMain` `frmInfo` `frmMacro` `frmEmulation` `frmScreen` `frmFLAGS` `frmOptions` `frm_mBox` `frmAbout` `frmOrigCode` `frmBaseConvertor` `frmStack` `frmEvaluator` `frmDebugLog` `frmExtendedViewer` `frmTrapping` `frmChooseTemplate` `frmEditVariable` `frmFlagAnalyzer` `frm16Color_DIALOG` `frmSetOutputDir` `frmMemory` `frmWriteBinToFloppy` `frmStartUp` `frmRegister` `frmSORRY_2_INTERRUPT` `frmEvaluatorHelper` `frmUpdate` `frmASCII_CHARS` `frmScreen_ASCII_CHARMAP` `frmStopOnCondition` `frmExportHTML` `frmSymbolTableViewer` `frmAssemble_ALL_IN_DIR` `frmBatchStatus` `frmDOS_FILE`

**Modules (31):** `Module1` `Module2` `StringTokenizer` `mInstructions` `mDefinition` `mBuild_Executable` `mCompMacro` `mCodeSelector` `mAnalyser` `mMath` `mString` `mEditor` `mRecentFiles` `mSYMBOL_TABLE` `mOptions` `mSegment_Names_Table` `mFORMS_STATE` `mOutput` `mWinState` `BugFixerMod` `UPDATE` `Registry` `modLaunchURL` `mRegistration` `mDOS_FILE` `inimodule` `mStepBack` `mFileDateTime_API` `mDOS_MEMORY_BLOCKS` `mMOUSE` `mINT_VECT_ROMWRITER` `mHW_INT` `mMemoryList` `mDisassemblyList`

**Data files / helpers referenced as strings:** `reg2x` `emulator.state` `Fonts.dat` `Other.dat` `Keywords.dat` `EmuPort.io` `getWindowsPath` `_location_counter_` `; [SOURCE]: `

## Behavioral contracts recovered

- **Assembler** (cluster `0x004b0000–0x004c0000`): `STRICT_SYNTAX` operand validation with verbatim diagnostics — `"not enough operands for …"`, `"first operand can be AL or AX"`, `"second operand cannot be AL"`, `"first operand (port) should be …"`, `"operands do not match: …"`, `"segment registers cannot go …"`, `"cannot use segment register …"`, `"insufficient operands for MOV"`.
- **Opcode table / emulator front** (cluster `0x00500000–0x00530000`): `"no such opcode in 8086 instruction set"`, `"unknown opcode skipped:"`, `"LOCK-OPCODE:F0"`, `"change address of INT 0 in i…"`, `"INT 4 can be customized in i…"`, `"note: you can overwrite INT …"`, `"you can define this interrup…"`.
- **Flag engine** (`0x004368e0`): `set_parity_flag` (identifier survived in the binary), plus `"set_parity_flag: …"`, `"parity even"`, `"parity odd"`, `"Carry Flag"`, `"Zero Flag"`, `"Overflow Flag"`, `"Auxiliary"`, `"IP register"`, `"segment register"`, `"stack pointer"`.
- **Reverse execution:** `mStepBack` module + `ENABLE_STEP_BACK` / `MAXIMUM_STEPS_BACK` / `MAXIMUM_FILES_TO_TRACK` / `FILE_UNDO_NOTE` in `emu8086.ini`. Step-back is journal-based; file operations not covered by it are flagged in the UI.
- **Virtual device bus:** `IN`/`OUT` port *N* ⇄ byte *N* of `C:\emu8086.io` (ports `0..0FFFFh`). No reserved ports.
- **Hardware interrupt bus:** with `IF=1`, poll bytes `0..255` of `C:\emu8086.hw`; nonzero byte *N* → dispatch IVT entry *N* (memory `0000–0400h`), then clear the byte to `00`.
- **Condition evaluator:** decimal / `101b` binary / `34h` hex with `~ * / % + - << >> & ^ |`; segment:offset shown in hex.
- **Load-state file:** `*.binf` — line *n* sets load segment, load offset, `AL AH BL BH CL CH DL DH DS ES SI DI BP CS IP SS SP`, all hex, positional. Companion `#mem=addr,bytes-…#` directive.
- **Build directives:** `#make_bin#` `#make_boot#` `#start=…#` `#masm#` `#cs= #ds= #es= #ss= #ip= #sp= #si= #di= #bp= #al=…#ch=` `#mem=`.
- **Custom memory map:** `custom_memory_map.inf`, `address - filename` (physical or `seg:off`, hex), `NO_SYS_INFO` skips the `00400h–00500h` system area.
- **Macro library** `emu8086.inc`: opt-in `DEFINE_*` for `SCAN_NUM` `PRINT_STRING` `PTHIS` `GET_STRING` `CLEAR_SCREEN` `PRINT_NUM` `PRINT_NUM_UNS`; always-on `PUTC` `PRINT`.
- **Exit string:** `"PROGRAM HAS RETURNED CONTROL TO THE OPERATING SYSTEM"`.

## `diasm.dll` — the native disassembler + expression evaluator (MSVC, 200 funcs)

This DLL holds the parts that are *not* VB6, and it is where the observable text format lives.

### Effective-address (ModRM r/m) table, verbatim

```
[BX]+  [BP]+  [DI]+  [SI]+  [BP+DI]+  [BP+SI]+  [BX+DI]+  [BX+SI]+
[BP]   [DI]   [SI]   [BP+DI] [BP+SI]  [BX+DI]   [BX+SI]
[BP+DI] [BP+SI] [BX+DI] [BX+SI]
```

### Operand/address print formats, verbatim

```
%s 0%04Xh                       mnemonic + absolute address
%s%s[0%02X%02Xh]                bracket form, e.g. MOV [0A0B h], 1
%s%s0%02X%02Xh                  bare 16-bit hex
%s%s0%02X%02Xh:0%02X%02Xh       logical seg:offset
%s%s0%Xh                        8-bit hex
%s, [0%02X%02Xh]   %s, %s0%02X%02Xh   %s%s0%Xh, %s   %s, %s%s0%Xh
%s, %s   [0%02X%02Xh], %s   %s0%02X%02Xh, %s   EXT ???
```

Note the house style: hex is `0%02X%02Xh` — leading `0`, two-digit bytes, lowercase `h` suffix — and logical addresses always render as `SSSS:OOOO`.

### x87 (8087) mnemonic table — display only

`FSETPM FINIT FCLEX FDISI FUCOMPP FSCALE FRNDINT FSINCOS FSQRT FYL2XP1 FPREM FINCSTP FDECSTP FPREM1 FXTRACT FPATAN FPTAN FYL2X F2XM1 FLDLN2 FLDLG2 FLDPI FLDL2E FLDL2T FXCH FLD FST FSTP FFREE FADD FADD FMUL FMULP FSUB FSUBP FSUBR FSUBRP FDIV FDIVP FDIVR FDIVRP FCOM FCOMP FUCOM FUCOMP` with fallbacks `F??? st%d` and `??? st%d`.

So emu8086 **decodes 80187 instructions for display but the 8086 core does not execute them**.

### The condition-expression evaluator lives here too

Error strings recovered: `stack overflow` `expression too complex` `bad operator in stack!` `bad syntax` `missing ')'` `missing an operand` `= needs an lvalue` `= needs and lvalue` *(sic — original typo)* `bad expression` `recursion`, with digit sets `0123456789abcdefABCDEF` and `01234567`.

Structure: recursive-descent evaluator with an explicit operator stack — matching the documented operator set `~ * / % + - << >> & ^ |`. This is the `frmStopOnCondition` engine.

## The interpreter — `doStep()`

The single decisive identification: **`FUN_004eff00` @ `0x004eff00`, 35 010 instructions**, is `doStep()` — the 8086 instruction interpreter. Its own diagnostic strings name it: `"wrong C6 address on doStep()"`, `"wrong C7 address on doStep()"`, `"wrong EA for [8C]"`, `"no such opcode in 8086 instruction set"`.

Neighbouring engine functions:

| Address | Instrs | Recovered role (from its own strings) |
|---|---|---|
| `FUN_0051ff60` | 9 485 | interrupt dispatch / exceptions — `"divide error - overflow."`, `"to manually process this error,"`, `"change address of INT 0 in interrupt vector table"` |
| `FUN_005e6540` | 4 077 | `mStepBack` — `"There are no recorded back steps ...."`, `tmp\deleted_bkp_`, `.dat`, `FILE_UNDO_NOTE`, `emu8086.ini` |
| `FUN_005efe60` | 282 | `mHW_INT` — `"hardware interrupt: "` |
| `FUN_005e0880` | 100 | `"input redirection is not supported yet!"`, `"INT 21h/9h should be used instead. refer to interrupt reference."` |

`MicroAsm` handles the 8087 escape opcodes separately: `do_instruction_D8_DF`, `MicroAsm_D8_TAB` … `MicroAsm_DF_TAB`, `MicroAsm_FINIT`, `INIT_FPU_PUB`, `MicroAsm_D9_TAB/DA/DB/DC/DD/DE`. Internal setter names `set_BL set_BH set_CL set_CH set_DL set_DH`, `Reset_Before_Load_Anything`, `Set_After_Load`.

## Assembler grammar recovered verbatim (VB6 string tables `0x0043b000–0x00442000`)

Original internal identifiers survived in the binary: `lst_opNames` `lst_Opcodes1` `lst_EA_TCONST` `lblLaunching` `compile_JCC_LOOP(` `compile_JMP(` `sum_str()` `bin_to_long(` `to_signed_int()` `to_unsigned_byte()` `make_normal_hex(` `get_parameter_from_SYMBOL_TABLE:` `process_DD:` `process_byte_str()` `set_ENTRY_POINT:` `create_NEW_source:`.

**Operand-type signatures** (the assembler's type system, verbatim):

```
AL,xb   AX,xw                     IN            AX,xw   AL,ib        OUT
AL,ib   AX,iw   eb,ib   ew,ib   ew,iw   rw,ed       ALU immediate
DS,mw   DS,rw   ES,mw   ES,rw   SS,mw   SS,rw       MOV seg<-,mem/reg
xb,AL   xw,AX   rb,ib   rw,iw                            MOV reg,reg/imm
ew,CS   ew,DS   ew,ES   ew,SS                            MOV to segment
rb,eb   eb,rb   rw,ew   ew,rw   AX,rw   rw,AX            XCHG
eb,CL   ew,CL   eb,1    ew,1                              SHIFT
d16 (simple var)
```

**`STRICT_SYNTAX` type-mismatch matrix** (verbatim, all 13 checks):

```
8 bit and 16 bit register
16 bit and 8 bit register
16 bit register and 8 bit address
8 bit address and 16 bit register
8 bit register and 16 bit address (add 'b.' before ...)
16 bit address and 8 bit register (add 'b.' before ...)
second operand is over 8 bits!
cannot use segment register with an immediate value
segment registers cannot go together!
first operand cannot be an immediate value
CS cannot be modified directly (use far JMP or CALL)
IP cannot be modified directly (use JMP or CALL)
cannot get value of IP register (use POP after CALL)
wrong use of segment register
mem to mem
memory to memory is not allowed for 8086:
```

**Addressing legality checks:** `[BX] cannot go with [BP] - wrong addressing!` · `[SI] cannot go with [DI] - wrong addressing!`

**Prefixes accepted:** `REPE REPZ REPNE REPNZ` · `cs: ds: es: ss:` · `.cs: .ds: .es: .ss:` · `SHORT` · `FAR` · `LOCK-OPCODE:F0`

**Numeric literal rules:** `&HFF` · `zero prefix must be added to a HEX value, for example …` · `probably no zero prefix for hex; or no 'h' suffix;` · `0123456789$` (MASM radix) · `cannot convert to 16 bit value:` · `undefined variable or over 16 bits`

**Register name list, verbatim:** `AH, AL, BH, BL, DH, DL, CH, CL, SI, DI, AX, BX, CX, DX, CS, IP, S…` (truncated in the table; SP/DS/ES/SS follow)

**DOS MZ EXE header emitter**, field-for-field: `EXE HEADER - bytes from 0000 to …` then `exe signature (M)=&H4D` `(Z)=&H5A`, `bytes on last page (l.byte/h.byte)`, `512 byte pages in file (l.byte/h.byte)`, `relocations (l.byte/h.byte)`, `paragraphs in header (l.byte/h.byte)`, `minimum memory`, `maximum memory`, `SS - stack segment`, `SP - stack pointer`, `check sum`, `IP - instruction pointer`, `CS - code segment`, `&H1E`, `relocation table adress`, `overlay number`.

**Build/load flow guards** (verbatim): `#make_com# detected.` `#make_exe# detected.` `#make_bin# detected.` `#make_boot# detected (.bin)` `directive: org 100h - found!` `directive: org 100h - not found!` `EXE file not built, aborted - ORG 100h found!` `COM file not built, aborted - ORG 100h not found!` `boot record file not built, aborted - "org 7c00h"` `is compiled successfully into` `bytes.` `Cannot generate a COM file. Remove the stack segme…`

**Sidecar outputs:** `.symbol` and `.debug` files per output (`noname.com.symbol`, `noname.com.debug`, `noname.exe.symbol`, `noname.exe.debug`, `noname.bin.symbol`, `noname.bin.debug`, …), cleaned by `delete_noname_files:`. Backups written as `noname.exe_`, `noname.bin_`, `noname.com_`.

**Templates:** `0_com_template.txt` `1_exe_template.txt` `2_bin_template.txt` `3_boot_template.txt` (and `fasm_`-prefixed variants), plus `#fasm#` detection and a `chkUseFASM` switch.

**Multi-pass assembling:** `Assembled in` … `passes. Time spent:` … `seconds.` and `recursive MACRO definition is over ` — confirming an iterative macro-expansion pass counter.

**Listing tokens:** `LISTING` `LABEL` `PROC` `END` `ENDS` `ENDP` `DUP` `FAR` `NEAR` `EQU` `ORG` `DB` `DW` `DD` `STACK` `NONE` `SEGMENT` `ASSUME` `MACRO` `ENDM` `COMMENT` `PAGE` `TITLE` `NAME` `INCLUDE` `.RADIX`

## Design smells to fix in the FOSS reimplementation

1. The device bus lives at `C:\emu8086.io` and `C:\emu8086.hw` — the **drive root** — which is the sole reason the app demands administrator rights. Replace with a unix-socket / named-pipe pair, or a per-user XDG data dir.
2. Virtual drives hard-coded to `C:\emu8086\vdrive\`.
3. Licensing/registration path (`frmRegister`, `mRegistration`, `RegNow` payment links) is dead weight.
4. Backend is 2.2 MB of native-compiled VB6 — no test seam for the CPU core.
5. The device protocol is a raw shared file with no framing, no error channel, and read-modify-write races.