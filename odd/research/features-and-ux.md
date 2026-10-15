# emu8067 — Feature surface, UI/UX, TUI vs GUI, and added value

Research track T9b for the clean-room FOSS reimplementation of emu8086. This
document answers one question: what does the reference product do, what does it
fail to do, what should we add, and what should the frontends look like.

## How to read this document

| Label | Meaning |
|---|---|
| **observed** | A behaviour or affordance the reference product demonstrably has. Always carries a source citation. |
| **proposal** | Something this project should do that the reference does not have. Never carries a reference citation. |
| **gap** | An absence in the reference product, with the evidence that establishes the absence. |

Source citations use short forms: `FINDINGS §<section>` means
`re/FINDINGS.md`, and `docs-txt/<file>.txt` means `re/docs-txt/<file>.txt`.

Sections 1 and 2 build the inventory from the sources, not from recollection of
the product. Section 3 onward is design work.

---

## 1. Feature inventory — what to inherit

The reference product's surface is wide but shallow in one important way: almost
every feature is a *view* over a single-step interpreter, and there is no
execution model above "one instruction at a time". The inventory below is
grouped into the eleven categories the brief requires. 133 features were
identified; the ID prefix (`CPU-`, `DBG-`, …) is stable and is referenced from
the roadmap in section 6.

### 1.1 CPU emulation

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| CPU-1 | Instruction interpreter | Single function executes one 8086 instruction per call; 35 010 instructions of the function body. | Press **Single Step** / `F8`; the disassembly pane advances one line. | FINDINGS §The interpreter |
| CPU-2 | FLAGS engine | Sets and clears the nine architectural flags per instruction class, including parity via `set_parity_flag`. | Flags pane updates per step; values are shown as a per-instruction result. | FINDINGS §Behavioral contracts; FINDINGS §Flag engine |
| CPU-3 | ModR/M decoding | Implements only the 16 legal 8086 effective-address combinations. | `BX`/`BP` conflict produces a distinct assembler diagnostic, not a decode fault. | FINDINGS §diasm.dll effective-address table |
| CPU-4 | Segment:offset addressing | Logical addresses render canonically as `SSSS:OOOO`. | Every memory view prints logical addresses in that form. | FINDINGS §Operand/address print formats |
| CPU-5 | Interrupt vector dispatch | Software `INT n` reads the IVT at `0000–0400h` and transfers control. | `INT 10h` in the disassembly is annotated as a BIOS/DOS call. | FINDINGS §Interrupt dispatch; docs-txt/memory.txt |
| CPU-6 | CPU-generated exceptions | Divide error (`INT 0`) and `INTO` overflow (`INT 4`) are raised by the core, with a diagnostic string and a suggestion to edit the IVT. | Error dialog names the exception and points at the IVT address to change. | FINDINGS §Behavioural contracts; FINDINGS §Neighbouring engine functions |
| CPU-7 | Hardware interrupt bus | With `IF=1`, the emulator polls 256 bytes of a file; nonzero byte *N* dispatches IVT entry *N*, then clears the byte. | Enabled/disabled by `cli`/`sti` in the guest program and by a setting. | FINDINGS §Hardware interrupt bus; docs-txt/io.txt |
| CPU-8 | x87 decode-only | Recognises 46 80187 mnemonics for display with fallbacks `F??? st%d` / `??? st%d`. | `FADD` renders in the disassembly pane but does not execute. | FINDINGS §x87 mnemonic table |

### 1.2 Debugger

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| DBG-1 | Single step | Executes one instruction and stops. | `[Single Step]` button, `F8`. | docs-txt/emulator.txt |
| DBG-2 | Step over | Steps over a procedure or interrupt call. | `[Step Over]` button. | docs-txt/debug.txt |
| DBG-3 | Step back | Journal-based reverse execution; file operations not covered by the journal are flagged. | `[Step Back]` button, `F6`; a warning note when an operation is not reversible. | FINDINGS §Reverse execution; docs-txt/start_unframed.txt; `emu8086.ini` `FILE_UNDO_NOTE` |
| DBG-4 | Debug command language | Twelve commands parsed in the emulator's own console: `c i iw o ow d t p r rf` plus `L` length parameters. | Typed into a debug input line; output is hex, no suffix required. | docs-txt/debug.txt |
| DBG-5 | Byte-range compare | `c v1 L 7 v2` compares two byte arrays and prints only differing addresses. Registers are accepted as parameters. | `c SI L 20h DI` compares 32 bytes from `SI` against `DI`. | docs-txt/debug.txt |
| DBG-6 | Port I/O from the console | `i`, `iw`, `o`, `ow` read and write I/O ports independently of the guest program. | `ow 123 5512` writes a word to port `123h`. | docs-txt/debug.txt |
| DBG-7 | Memory dump | `d addr` dumps 128 bytes, repeats at the next block with no argument, honours `L` and square brackets. | `d ds:[bx+si]`, `d lion L 10`. | docs-txt/debug.txt |
| DBG-8 | Stop on condition | Evaluates an arithmetic/bitwise expression each instruction and halts when true. Only active while its window is open. | `debug → stop on condition`; expression like `1234h + 1`, `0FFh - 101b`. | docs-txt/stop_on_condition.txt; FINDINGS §Condition evaluator |
| DBG-9 | Expression evaluator | Recursive descent with an explicit operator stack; literals in decimal, `101b`, `34h`; operators `~ * / % + - << >> & ^ |`. | Typed into the stop-on-condition dialog. | FINDINGS §diasm.dll condition-expression evaluator; docs-txt/stop_on_condition.txt |
| DBG-10 | Register inspection and mutation | All registers visible; values may be typed over in place, or edited in a richer window. | Double-click a register box → **Extended Viewer**; type over a value to change it live. | docs-txt/emulator.txt |
| DBG-11 | Flag inspection and mutation | A dedicated flags window plus a per-instruction flag analyser that shows which flags the current instruction wrote. | `[Flags]` button opens the flags form. | FINDINGS §Module map (`frmFLAGS`, `frmFlagAnalyzer`); docs-txt/emulator.txt |
| DBG-12 | Debug log | A scrollable window records debug output separately from the console. | Opened as its own window. | FINDINGS §Module map (`frmDebugLog`) |

### 1.3 Memory inspection

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| MEM-1 | Memory window | Live hex+ASCII view of any physical or logical address. | Opened as its own window; doubles as a **d** command source. | FINDINGS §Module map (`frmMemory`) |
| MEM-2 | Extended Viewer | Converts a value between decimal, binary, hexadecimal, and character forms; the value is editable. | Double-click any register box or any memory-list row. | docs-txt/emulator.txt; FINDINGS §Module map (`frmExtendedViewer`, `frmBaseConvertor`) |
| MEM-3 | Little-endian word edit | Reads a word from a memory location: low byte from the selected byte, high byte from the next address. | Double-click a memory-list item → editable word. | docs-txt/emulator.txt |
| MEM-4 | Variables window | Driven by the `.symbol` sidecar emitted by the assembler, not by runtime heuristics. | Automatically populated after compiling. | docs-txt/compiler.txt; FINDINGS §Sidecar outputs |
| MEM-5 | Variable editor | A dedicated form for editing a variable's value and width while the program runs. | Opened from the variables window. | FINDINGS §Module map (`frmEditVariable`) |
| MEM-6 | Symbol table viewer | Shows the full symbol table as plain text. | Its own window; the `.symbol` file is also plain text and externally readable. | FINDINGS §Module map (`frmSymbolTableViewer`); docs-txt/compiler.txt |
| MEM-7 | Stack view | Renders the stack around `SP`, so pushes and pops are visible as a region, not as a number. | Its own window. | FINDINGS §Module map (`frmStack`) |
| MEM-8 | Global memory table | The IBM-PC memory layout is documented and auto-loaded: IVT, system information area, 640 KB free, VGA at `A0000`, CGA text at `B8000`, BIOS ROM at `F4000`. | Shown as reference; `INT_VECT` and `BIOS_ROM` files are loaded at fixed addresses. | docs-txt/memory.txt |
| MEM-9 | Custom memory map | A user file maps `address - filename` onto physical or `seg:off` addresses; `NO_SYS_INFO` suppresses the system-area update. | Create `custom_memory_map.inf` and restart. | docs-txt/custom_memory_map.txt; FINDINGS §Custom memory map |

### 1.4 Assembler

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| ASM-1 | Integrated two-pass assembler | Compiles source in the IDE and loads the result straight into the emulator. | `[Compile]` then `[Emulate]`. | docs-txt/compiler.txt; docs-txt/start_unframed.txt |
| ASM-2 | Strict operand typing | A 13-entry type-mismatch matrix rejects `MOV DS, 100`, `MOV AL, 300`, 8-vs-16-bit register mixes, segment/immediate combinations, direct `CS`/`IP` writes, and memory-to-memory. | Off by default; enabled by the `STRICT_SYNTAX` setting. Diagnostics quote the failing rule. | FINDINGS §STRICT_SYNTAX type-mismatch matrix; `emu8086.ini` `STRICT_SYNTAX` |
| ASM-3 | Addressing legality checks | `[BX]` cannot pair with `[BP]`; `[SI]` cannot pair with `[DI]`. | Diagnostic names both registers and the rule. | FINDINGS §Addressing legality checks |
| ASM-4 | MASM/TASM-compatible syntax | The dialect is stated to be fully compatible; MASM/TASM produce equivalent machine code for the same source. | Same source compiles under either assembler. | docs-txt/compatibility.txt |
| ASM-5 | Directives | `SEGMENT ENDS ORG DB DW DD EQU DUP STACK NONE ASSUME PROC END ENDS ENDP FAR NEAR MACRO ENDM COMMENT PAGE TITLE NAME INCLUDE .RADIX` are all recognised. | Written in source; the listing shows how each was consumed. | FINDINGS §Listing tokens |
| ASM-6 | Number bases | `&HFF` hex, `101b` binary, decimal, `0123456789$` MASM radix, `h` suffix; error text tells the user which convention was expected. | Written inline in operands and directives. | FINDINGS §Numeric literal rules |
| ASM-7 | Size prefixes | `byte ptr` / `word ptr` plus the short forms `b.` / `w.`. | `mov w.[bx], 1234h`. | docs-txt/compatibility.txt; FINDINGS §STRICT_SYNTAX matrix (`add 'b.' before ...`) |
| ASM-8 | Segment overrides | `cs: ds: es: ss:` and the dotted `.cs: .ds: .es: .ss:` forms. | `mov ah, es:[bx]`. | FINDINGS §Prefixes accepted; docs-txt/compatibility.txt |
| ASM-9 | Symbolic operand types | `d16` simple-variable operands, `DUP`, `EQU`, and `LABEL` are resolved before validation. | `var db 5` used as an operand. | FINDINGS §Operand-type signatures; docs-txt/compatibility.txt |
| ASM-10 | Macro engine | User macros with `LOCAL` labels and a recursion guard; assembly is iterative and reports the pass count and elapsed time. | `Assembled in N passes. Time spent: …`. | FINDINGS §Multi-pass assembling |
| ASM-11 | `emu8086.inc` macro library | Seven opt-in routines (`SCAN_NUM PRINT_STRING PTHIS GET_STRING CLEAR_SCREEN PRINT_NUM PRINT_NUM_UNS` via `DEFINE_*`) plus always-on `PUTC` and `PRINT`. | `CALL PRINT_NUM` after one `DEFINE_PRINT_NUM`. | FINDINGS §Macro library; `inc/emu8086.inc` |
| ASM-12 | Instruction expansion for 8086 compatibility | One source instruction may become several machine instructions: `ROL AL, 5` assembles to five `ROL AL, 1`. | Disassembly at that address shows five instructions. | docs-txt/compiler.txt |

### 1.5 Disassembler

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| DIS-1 | Native disassembler | A separate 200-function MSVC DLL decodes raw machine code without source. | Load a `.com`/`.exe`/`.bin` and read it without source. | FINDINGS §diasm.dll; docs-txt/emulator.txt |
| DIS-2 | House print format | Addresses print as `0%02X%02Xh` and logical addresses as `SSSS:OOOO`; bracket and bare forms are both supported. | Visible everywhere an address is shown. | FINDINGS §Operand/address print formats |
| DIS-3 | BIOS `DI` decoding | A 4-byte BIOS call is decoded as `BIOS DI` using the `FFFF` opcode prefix, e.g. `FFFFCD10` → interrupt `10h`. | Disassembly of BIOS stubs is readable rather than `int 0FFh`. | docs-txt/memory.txt |
| DIS-4 | Source-line mapping | A `.debug` sidecar maps machine code back to the original `.asm` lines so the emulator can select source while running. | Selecting a disassembly line highlights the source line that produced it. | docs-txt/compiler.txt |

### 1.6 Build and output

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| BLD-1 | `#make_com#` | Raw binary loaded with a `100h` prefix; entry point is always the first byte. | Auto-selected when `org 100h` is present. | docs-txt/compiler.txt; FINDINGS §Build/load flow guards |
| BLD-2 | `#make_exe#` | Emits a DOS MZ header field-for-field, including checksum, relocations, and minimum/maximum memory. | Auto-selected when a stack segment is found. | FINDINGS §DOS MZ EXE header emitter; docs-txt/compiler.txt |
| BLD-3 | `#make_bin#` | Raw binary plus an ASCII `.binf` sidecar giving load segment, load offset and every register preset. | `#AL=12#` … `#SP=FFFF#` in source; `.binf` written automatically. | docs-txt/compiler.txt; FINDINGS §Load-state file |
| BLD-4 | `#make_boot#` | Boot-sector binary pinned to `0000:7C00h`; must be under 512 bytes. | `[Virtual drive] → write 512 bytes at 7c00 to boot sector`. | docs-txt/compiler.txt; FINDINGS §Build/load flow guards |
| BLD-5 | `#mem=…#` directive | Writes arbitrary bytes into memory before execution: `#MEM=1000,01ABCDEF0122-0200,1233#`, physical or logical addresses. | Written in source; also available standalone in a `.binf`. | docs-txt/compiler.txt; `default.binf` |
| BLD-6 | Build-directive register presets | `#cs= #ds= #es= #ss= #ip= #sp= #si= #di= #bp= #al= … #ch=#` and `#load_segment=` / `#load_offset=`. | Preset any architectural register from source. | FINDINGS §Build directives; docs-txt/compiler.txt |
| BLD-7 | Sidecar outputs | `.symbol` (variables window), `.debug` (source mapping), `.binf` (load state), and a copy of the original source — all plain text and all readable in any editor. | Emitted next to the executable; deleted on rebuild. | docs-txt/compiler.txt; FINDINGS §Sidecar outputs |
| BLD-8 | Output directory selection | A dialog chooses where the executable is written. | Asked once per compile. | FINDINGS §Module map (`frmSetOutputDir`) |
| BLD-9 | Backups | Previous outputs are preserved as `name.exe_`, `name.bin_`, `name.com_`. | Written on every build. | FINDINGS §Sidecar outputs; `emu8086.ini` `ALWAYS_BACKUP` |
| BLD-10 | Templates | Four starting points: `0_com_`, `1_exe_`, `2_bin_`, `3_boot_` templates, each with a `fasm_`-prefixed variant. | `New → Clean` then choose a template. | FINDINGS §Templates |
| BLD-11 | External assembler delegation | `#masm#` and `#fasm#` hand compilation to MASM or to the bundled FASM 1.64. | Opt-in directive; MASM paths come from settings. | FINDINGS §Templates; `emu8086.ini` `MASM_PATH` |
| BLD-12 | Listing output | `LISTING=true` emits a `.list` showing what the assembler did and why. | Opened in the configured ASCII viewer. | `emu8086.ini` `LISTING` |
| BLD-13 | Batch directory assembly | Assembles every source file in a directory with a status window. | `emu8086 /a c:\sources`. | FINDINGS §Module map (`frmAssemble_ALL_IN_DIR`, `frmBatchStatus`); `emu8086.ini` `SILENT_ASSEMBLER` |

### 1.7 I/O and devices

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| DEV-1 | Unreserved port space | `IN`/`OUT` on ports `0000h–FFFFh`, with no reserved or fixed ports. | Any port works; two devices must not share one. | docs-txt/io.txt; FINDINGS §Virtual device bus |
| DEV-2 | File-backed port bus | Port *N* ⇄ byte *N* of a shared file, read and written by an external process. | An out-of-process device writes a file byte; the guest reads it on the next `IN`. | FINDINGS §Virtual device bus; docs-txt/io.txt |
| DEV-3 | Shipped devices | Seven example devices: Traffic Lights (port 4), Stepper Motor (7), Robot (9/10/11), Simple, LED Display, Thermometer, Printer. | `Virtual devices` menu. | docs-txt/DEVICES/_READ_ME.txt; docs-txt/io.txt |
| DEV-4 | Traffic Lights protocol | 12 lamps (4 green, 4 yellow, 4 red) on a word port; bit set = lit. | `MOV AX, 0000001011110100b` / `OUT 4, AX`. | docs-txt/io.txt |
| DEV-5 | Stepper Motor protocol | Three-phase magnet control on bits 0–2; top bit of port 7 signals ready. | `OUT 7, AL` with `001b`/`011b`/`010b`/`110b`. | docs-txt/io.txt |
| DEV-6 | Robot protocol with status register | Command register (port 9), data register (10), status register (11) with busy, new-data, and error bits. | `IN AL, 11` and test bit 1 before sending a command. | docs-txt/io.txt |
| DEV-7 | Robot world-map editor | Click-to-place walls and lamps, rotate the robot, bulk toggle lamps; the map auto-saves. | Right-click for a popup menu; map stored as a data file. | docs-txt/io.txt |
| DEV-8 | Devices in any language | A device is any program that can read and write a file; bindings are supplied for VB6, VB.NET, C, C++, C#, and Delphi. | Include the header, link against the file. | docs-txt/DEVICES/_READ_ME.txt; docs-txt/io.txt |
| DEV-9 | Zero-configuration device discovery | A device is activated when its filename appears anywhere in a source comment or string literal. | Write `; traffic_lights.exe` in a comment and the menu entry appears. | docs-txt/DEVICES/_READ_ME.txt |
| DEV-10 | `#start=…#` launch directive | Launches a device when the program runs, optionally from outside the devices folder. | `#start=traffic_lights.exe#` in source. | FINDINGS §Build directives; `emu8086.ini` `START_OUTSIDE_DEVICES_FOLDER` |
| DEV-11 | BIOS and DOS interrupt library | 11 interrupt vectors with 38 distinct `AH` functions: `10h 11h 12h 13h 15h 16h 19h 1Ah 20h 21h 33h`. | `INT 21h/09h` prints a `$`-terminated string. | docs-txt/8086_bios_and_dos_interrupts.txt |
| DEV-12 | Interrupt overriding | The user may redefine the target address of any interrupt vector, including `INT 0`. | `"change address of INT 0 in i…"` prompts the user to edit the IVT. | FINDINGS §Behavioural contracts |
| DEV-13 | Virtual floppy drives | Four floppy images, `FLOPPY_0`–`FLOPPY_3`; `FLOPPY_0` is a real 1 474 560-byte image. Drive count is discoverable through `INT 11h` bits 7–6. | `[Create new floppy drive]`; deletion is manual file deletion. | docs-txt/emulator.txt |
| DEV-14 | Boot from floppy | Boots the emulator from a virtual floppy, and writes a compiled binary into the boot sector. | `[Virtual drive] → boot from floppy`. | docs-txt/compiler.txt; FINDINGS §Module map (`frmWriteBinToFloppy`) |
| DEV-15 | Virtual hard drives | `vdrive/` subdirectories `A`–`F` back a DOS-visible filesystem. | Written by the DOS file API. | `re/extracted/app/vdrive/` |
| DEV-16 | DOS memory blocks | Implements the DOS memory block chain used by the file API. | `INT 21h/48h`, `4Ah`, `49h`. | FINDINGS §Module map (`mDOS_MEMORY_BLOCKS`); docs-txt/8086_bios_and_dos_interrupts.txt |
| DEV-17 | DOS file API | A full DOS file layer with its own window and module. | `INT 21h/3Ch`…`3Fh`, `40h`…`42h`. | FINDINGS §Module map (`mDOS_FILE`, `frmDOS_FILE`) |
| DEV-18 | VGA state device | A separate device process owns the emulated VGA state. | `VGA_STATE.exe` in the devices folder. | `DEVICES/` |

### 1.8 Screen and keyboard

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| SCR-1 | Emulated VGA text screen | Eight 4 096-byte text pages in `B800:0000`, addressable through `INT 10h/05h`. | A resizable emulator screen window. | docs-txt/memory.txt |
| SCR-2 | Teletype output | `INT 10h/0Eh` writes a character and advances the cursor. | The basis of the `PUTC` and `PRINT` macros. | docs-txt/8086_bios_and_dos_interrupts.txt; `inc/emu8086.inc` |
| SCR-3 | Cursor control | Set cursor shape (`01h`), set position (`02h`), read position (`03h`), scroll (`06h`), hide/show (`07h`). | Programmatic only. | docs-txt/8086_bios_and_dos_interrupts.txt |
| SCR-4 | 256-colour graphics mode | VGA mode `13h` fills `A0000:B1000`; examples draw shapes, pixels, attributes and scroll. | `0_sample_vga_graphics.asm`, `pixel.asm`, `attrib.asm`, `scroll.asm`, `pages.asm`. | docs-txt/memory.txt; `re/extracted/app/examples/` |
| SCR-5 | Text attributes | Foreground/background colour on a character write. | `INT 10h/13h`. | docs-txt/8086_bios_and_dos_interrupts.txt; `examples/colors.asm` |
| SCR-6 | Keyboard input | `INT 16h/00h` blocks for a key, `INT 16h/01h` polls the buffer. | `examples/keybrd.asm`, `examples/Count_Key_Presses.asm`. | docs-txt/8086_bios_and_dos_interrupts.txt |
| SCR-7 | Keyboard buffer overflow | Overflow is detectable and audibly signalled. | `emu8086.ini` `BEEP` also fires on ASCII 7. | `emu8086.ini` `BEEP` |
| SCR-8 | Mouse | `INT 33h` with initialise, show, hide and position functions. | `examples/mouse.asm`, `examples/mouse2.asm`. | docs-txt/8086_bios_and_dos_interrupts.txt |
| SCR-9 | Screen hot-keys | `SCREEN_HOTKEYS=true` lets `F8`/`F6` reach the debugger even when the guest screen has focus, instead of going to the guest keyboard buffer. | Toggle in settings. | `emu8086.ini` `SCREEN_HOTKEYS` |
| SCR-10 | ASCII character map | An on-screen glyph table and a per-glyph editor for building character sets. | `frmASCII_CHARS`, `frmScreen_ASCII_CHARMAP`. | FINDINGS §Module map |
| SCR-11 | Screen resize with fixed page stride | Resizing the window changes the bytes per page on screen, but the emulator always keeps `1000h` bytes reserved per page. | Drag the window edge. | docs-txt/memory.txt |
| SCR-12 | Mode and state inspection | A dedicated form dumps and edits the current video mode and page state. | `attrib.asm` support. | FINDINGS §Module map (`frmScreen`) |

### 1.9 File handling

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| FIL-1 | Binary loader | `.com`, `.exe` and `.bin` load directly; any unrecognised extension loads at the current `CS:IP`. | Drag or open a file. | docs-txt/emulator.txt; docs-txt/compiler.txt |
| FIL-2 | `.binf` load state | A positional ASCII file — line *n* is a specific field, order is significant, comments ignored. | `default.binf` is used when a file has no `.binf`. | FINDINGS §Load-state file; `default.binf` |
| FIL-3 | Default load state | A shipped `default.binf` supplies `CS=DS=ES=SS=0100h`, `SP=FFFEh`, and seeds `0100:FFFE` with a `RET` that lands on an `HLT` at `0100:FF00`. | The reference "returns control to the operating system" instead of crashing. | FINDINGS §Exit string; `default.binf` |
| FIL-4 | Configuration file | A plain INI drives 22 distinct settings (see 1.11). | Edited by hand or by the options dialog. | `emu8086.ini` |
| FIL-5 | Recent files | A recent-files list capped at 20, configurable. | `File` menu. | FINDINGS §Module map (`mRecentFiles`); `emu8086.ini` `RECENT_FILES` |
| FIL-6 | Editor extension routing | A configurable list of extensions opens in the source editor rather than the emulator. | `ASCII_EXTENTIONS=.cpp,.java,.vb,.c,.ini,…,.lst` | `emu8086.ini` `ASCII_EXTENTIONS` |
| FIL-7 | Shell integration | Right-click a `.com` in Explorer and send it to emu8086. | Context menu. | docs-txt/compatibility.txt |
| FIL-8 | Output cleanup | Stale sidecars are removed when an output is rebuilt. | Automatic. | FINDINGS §Sidecar outputs (`delete_noname_files:`) |
| FIL-9 | File date/time API | Exposes a host date/time function to the guest. | `mFileDateTime_API`. | FINDINGS §Module map |
| FIL-10 | Templates are upgrade-safe | The `inc/` folder warns that modifications are overwritten on update and must be copied out first. | Notice file in the folder. | `inc/__README_IMPORTANT.txt` |

### 1.10 Extensibility

| ID | Feature | What it does | User-facing interaction | Source |
|---|---|---|---|---|
| EXT-1 | Out-of-process devices in any language | The device bus is language-neutral by construction: it is a file, not an ABI. | Write a program that reads and writes one byte. | docs-txt/io.txt; DEVICES/_READ_ME.txt |
| EXT-2 | Language bindings for devices | Ready-made `io.bas`, `io.vb`, `io.h`, `io.cs`, `io.pas`. | Include and link. | DEVICES/_READ_ME.txt |
| EXT-3 | Guest-programmable interrupts | The user can point any IVT entry at their own handler, including `INT 0` and `INT 4`. | Edit the vector or use `examples/Custom_Interrupt.asm`. | FINDINGS §Behavioural contracts; `examples/Custom_Interrupt.asm` |
| EXT-4 | Configurable memory map | Load arbitrary binaries at arbitrary addresses to build a non-IBM-PC machine. | Edit `custom_memory_map.inf`, restart. | docs-txt/custom_memory_map.txt |
| EXT-5 | Extensible macro library | The shipped `emu8086.inc` is a plain source file users are expected to copy and extend. | Copy the file, add macros. | `inc/emu8086.inc`; `inc/__README_IMPORTANT.txt` |
| EXT-6 | External assembler delegation | Compile through MASM or FASM instead of the built-in assembler. | `#masm#`, `#fasm#`. | FINDINGS §Templates |
| EXT-7 | Device-in-source discovery | Naming a device in a comment is enough to enable it. | Write the filename in a comment. | DEVICES/_READ_ME.txt |

### 1.11 Settings

All observed from `re/extracted/app/emu8086.ini`.

| ID | Setting | Default | Effect |
|---|---|---|---|
| CFG-1 | `STRICT_SYNTAX` | `true` | Turns the 13-rule operand matrix on. |
| CFG-2 | `LISTING` | `true` | Emits `.list` files. |
| CFG-3 | `ENABLE_STEP_BACK` | `true` | Enables reverse execution. |
| CFG-4 | `MAXIMUM_STEPS_BACK` | `default` | Journal depth limit. |
| CFG-5 | `MAXIMUM_FILES_TO_TRACK` | `default` | File-operation tracking limit. |
| CFG-6 | `FILE_UNDO_NOTE` | `true` | Warns about operations step-back cannot reverse. |
| CFG-7 | `SCREEN_HOTKEYS` | `true` | Debugger keys bypass the guest keyboard buffer. |
| CFG-8 | `HARDWARE_INTERRUPTS` | `true` | Initial state of `IF`. |
| CFG-9 | `HW_INTERRUPT_FILE` | `c:\emu8086.hw` | Hardware interrupt bus file. |
| CFG-10 | `EMUPORT` | `c:\emu8086.io` | Port bus file. |
| CFG-11 | `VDRIVE_PATH` | `default` | Virtual drive root. |
| CFG-12 | `MASM_PATH` / `_LINK_` / `_INCLUDE_` | `C:\MASM611\` | External assembler locations. |
| CFG-13 | Lexer / variables / debug-log fonts | `default` | Three independently configurable font size and face pairs. |
| CFG-14 | `FIX_SMALL_FONTS` | `true` | Compensates for terminal fonts narrower than 8 px. |
| CFG-15 | `BEEP` | `true` | Audible alert on ASCII 7 and buffer overflow. |
| CFG-16 | `START_OUTSIDE_DEVICES_FOLDER` | `false` | Whether `#start=` may launch arbitrary executables. |
| CFG-17 | `RECENT_FILES` | `default` | Recent-file count, max 20. |
| CFG-18 | `UPDATE_CHECK` | `0` | Update notification interval in days. |
| CFG-19 | `TRIM_ORIGINAL_SOURCE` | `true` | Trims source lines for the source-mapping view. |
| CFG-20 | `SAVE_ASCII_STATE` | `false` | Ascii state save; noted in the file as **not reloadable**. |
| CFG-21 | `ASCII_EXTENTIONS` | 22 extensions | Editor-vs-emulator routing. |
| CFG-22 | `ASCII_VIEWER` | `default` | External viewer for listings and exports. |
| CFG-23 | `SILENT_ASSEMBLER` | `false` | Suppresses status windows during batch assembly. |
| CFG-24 | `ALWAYS_BACKUP` | `true` | Keeps `name.exe_` style backups. |
| CFG-25 | `ACTIVATE_SCREEN` / `..._WHEN_STOPPED` | `true` | Raises the guest screen window on halt. |
| CFG-26 | `WARN_RUN` | `true` | Warns before running an executable outside the emulator. |
| CFG-27 | `ASK_OPEN_URL` | `true` | Confirms before opening online documentation. |
| CFG-28 | `STARTUPWIN` | `true` | Shows the startup window. |

### 1.12 Feature totals

| Group | Count |
|---|---|
| CPU emulation | 8 |
| Debugger | 12 |
| Memory inspection | 9 |
| Assembler | 12 |
| Disassembler | 4 |
| Build and output | 13 |
| I/O and devices | 18 |
| Screen and keyboard | 12 |
| File handling | 10 |
| Extensibility | 7 |
| Settings | 28 |
| **Total** | **133** |

---

## 2. Feature inventory — what is missing from the reference

These are absences, each with the evidence that establishes the absence. They
are ranked by how much an 8086 learner or developer is blocked by them.

| # | Missing capability | Evidence of absence | Cost to the user |
|---|---|---|---|
| GAP-1 | **No 8088 support** | The string `8088` does not occur in any of the 34 files in `re/docs-txt/`. `FINDINGS §diasm.dll effective-address table` lists only the 16-bit-only EA set, including `[SP]` and `[BP]` variants that require a 16-bit address-size override on an 8088. | Half of the real 8086/8088 family is unreachable. A user with an 8088 program has no way to model the high-half register quirks or the 8-bit-stack behaviour. |
| GAP-2 | **No execution timing model** | No occurrence of `MHz`, `clock` (in the timing sense), or cycle count in any `re/docs-txt/` file. The only speed control documented is "step delay" between instructions, a UI pacing value (`docs-txt/8086_instruction_set.txt` line 151). | A user cannot answer "why is my loop slow", cannot write cycle-accurate software, and cannot validate the emulator against timing documentation. |
| GAP-3 | **No documented breakpoints** | `frmTrapping` and the journal entry `tmp\deleted_bkp_` exist (`FINDINGS §Behavioral contracts`; `FINDINGS §Reverse execution`), so *some* breakpoint mechanism exists. But no file in `re/docs-txt/` contains the word `breakpoint`, and no syntax, hotkey, or behaviour is documented anywhere. | The single most important debugger affordance for a learner is undocumented even by the vendor. Its behaviour is unknowable from the sources, so we must design it from first principles. |
| GAP-4 | **No conditional breakpoints** | `docs-txt/stop_on_condition.txt` is the only halting mechanism beyond single-step, and it is explicitly *continuous* and *only active while its window is open*. There is no way to say "break when `AX == 0x1234` at this address". | Setting up a loop-termination breakpoint takes manual single-stepping. This is the highest-value debugger gap. |
| GAP-5 | **No memory watchpoints** | The debug command list in `docs-txt/debug.txt` contains `c i iw o ow d t p r rf` and nothing else. There is no watch command. | Watching a variable while it is being written by an interrupt handler or a loop is impossible without polling `d` by hand. |
| GAP-6 | **No instruction-level opcode coverage report** | `LISTING=true` (`emu8086.ini`) produces an assembler listing — an assembly-time artefact about source lines, not a coverage measurement. Nothing in `re/docs-txt/` reports which of the 256 opcodes or ModR/M forms the core can actually execute. | A contributor cannot tell whether a new opcode is implemented. A user cannot tell whether a program uses an instruction their emulator silently mis-executes. |
| GAP-7 | **No automated test runner for guest programs** | `examples/file-operations.asm` shows a guest program using the DOS file API; nothing in `re/docs-txt/` defines an assertion, expected-output, or exit-status convention. | The 85 shipped examples cannot be used as a regression suite. Every core change is validated only by unit tests of the core, never end-to-end by running real programs. |
| GAP-8 | **8087 is decode-only, never executed** | `FINDINGS §x87 mnemonic table` states it directly: the emulator "decodes 80187 instructions for display but the 8086 core does not execute them". | Any program using `FADD`/`FLD` disassembles but cannot run. The disassembler actively invites the user to try. |
| GAP-9 | **No save/load of machine state** | `emu8086.ini` `SAVE_ASCII_STATE=false` is annotated in the file itself as "(not possible to reload)". | A user cannot park a machine state and return to it. Crash reproduction requires replaying from the start. |
| GAP-10 | **No remote, headless, or network operation** | The only headless path documented is `emu8086 /a c:\sources` for batch assembly (`emu8086.ini` `SILENT_ASSEMBLER`). No debugger attach protocol exists. | Cannot drive the emulator from CI, from a script, or from another machine. This is the gap that costs us the most testability. |
| GAP-11 | **No network, serial, or USB device transport** | `docs-txt/io.txt` is explicit: the device bus is "byte *N* of `c:\emu8086.io`", and `DEVICES/_READ_ME.txt` requires read/write access to two files in the root of drive `C:`. Devices are local processes polling a local file. | The hardware-interrupt story ("external peripherals, microcontrollers, or the 8087") cannot be realised. A robot device only exists on the same machine. |
| GAP-12 | **No re-assembly from disassembly** | `.symbol` and `.debug` sidecars exist (`docs-txt/compiler.txt`), but nothing in `re/docs-txt/` offers to turn a disassembly back into assemblable source. | Disassembly is a read-only artefact. Round-tripping — the thing `T6` names — is not a product feature in the reference. |
| GAP-13 | **No accessibility provision** | `docs-txt/io.txt` describes the traffic-light device as using "yellow hexadecimal digits in caption (to achieve compact view)", i.e. colour carries meaning. `emu8086.ini` `FIX_SMALL_FONTS` exists because the UI breaks below 8-pixel font width. No screen-reader or high-contrast support is documented. | The reference is not usable with a screen reader and its compact encodings fail for colour-blind users. Since it is 16-bit-colour-only on Windows (`docs-txt/requirements.txt`), there is no theme to change. |
| GAP-14 | **No localisation** | All 34 documentation files and all UI strings are English only; no language setting appears in `emu8086.ini`. | Nothing to do here; recorded so the gap is not mistaken for an oversight in *our* inventory. |
| GAP-15 | **No multi-segment execution beyond `.exe`** | `docs-txt/compiler.txt` offers four output types; multi-segment support is described only for `#make_exe#`. Far calls are exercised by three examples but there is no documented segment-loading facility for the flat memory model. | Far pointer work is example-only, not productised. |
| GAP-16 | **No executable state inspection for self-modifying code** | `examples/self_modifying_code.asm` exists and the interpreter obviously supports it, but no UI surfaces "this instruction changed the bytes you are about to execute". | A learner studying self-modifying code has no tool for understanding it. |

---

## 3. Proposed additions with real value

All entries are **proposal**. Ranked by value-to-cost. "Verdict" is a judgement,
not a measurement.

| Rank | Addition | Solves | User story | Cost | Depends on | Verdict |
|---|---|---|---|---|---|---|
| 1 | **Instruction timing table** (cycle counts per opcode) | GAP-2 | "My loop over 1000 bytes takes 4000 cycles; I want the program to finish inside a 1 ms slot." | Medium: a generated table of ~256 entries plus a cycle counter on `Machine`. No core rework — `step()` already returns a `StopReason` and `Cpu` is a plain value. | T5 core complete | **Do it.** Small, self-contained, unlocks everything else that reasons about time. |
| 2 | **Symbolic expression evaluator, shared** | GAP-4, GAP-5 | "Break when `AX == 0x1234 && [DS:0100] > 0`" | Medium: the operator set is already recovered (`FINDINGS §condition-expression evaluator`), so this is a spec-to-code job, not research. Must resolve symbols from the `.symbol` sidecar. | T6 assembler (symbol table), T7 UI | **Do it.** It is the single highest-leverage debugger feature and the reference has none. |
| 3 | **Watchpoints and conditional breakpoints** | GAP-3, GAP-4, GAP-5 | "Watch `0100:0004` for writes; stop at `CS:0200` when `CX == 0`." | Medium once #2 exists: an address predicate and a per-step predicate in the step loop. | #2, T7 UI | **Do it.** Without this the TUI is a monitor, not a debugger. |
| 4 | **Built-in `.asm` test runner** | GAP-7 | "`emu8067 test examples/` — every shipped example must assemble, load, run to completion, and produce expected screen output." | Medium: a manifest format, an assertion convention, and a headless runner. The 85 examples are the corpus. | T5, T6, and a headless mode (see #9) | **Do it, early.** This is the maintainer's leverage: it is what makes every later feature safe to add. |
| 5 | **Instruction coverage report** | GAP-6 | "Which of the 8086 opcode map does the core execute, and which does it only decode?" | Small: a counter per opcode in the core plus a `--coverage` flag and an HTML/text report. | T5 core complete | **Do it.** Cheap, and it turns "unknown opcode skipped" into a visible, closable list. |
| 6 | **8088 mode** | GAP-1 | "Run this 8088 binary with its 8-bit stack and high-half register quirks." | Small to medium: a mode flag on `Machine` affecting stack address size, high-half register access, and prefetch behaviour. | T5 core complete | **Do it.** Tiny cost, doubles the hardware surface, and the project's own crate doc already claims 8086/8088 (`crates/core/src/lib.rs`). |
| 7 | **Native save/load of full machine state** | GAP-9 | "Save this state, send it to a friend, they reload it and continues from the same instruction." | Medium: the state is already a `Clone` value type; needs a versioned binary format and a checksum. | T5 core complete | **Do it.** Trivial given the core is a plain value, and it is the cheapest possible bug-report format. |
| 8 | **Record and replay of execution** | GAP-9, GAP-10 | "Record this session; attach the trace to a bug report so someone else can step through it." | Medium: a serialised event log (input + device output + state snapshots at intervals). Depends on #7 for snapshots and on #5 to know when an opcode is unknown. | #7, #5 | **Do it, after #7.** Extremely high value for a teaching tool because the artefact is shareable. |
| 9 | **Headless / CLI mode** | GAP-10 | "`emu8067 run program.com --max-steps 100000 --assert-screen 'Hello'` in CI." | Small: the core has no I/O, so a binary that owns the core and prints to stdout is a few hundred lines. | T5 core complete | **Do it.** This is the enabler for #4, #5, #8 and CI generally. Smallest item on this list with the largest enabling effect. |
| 10 | **VGA display modes: 16-colour text and 256-colour graphics, done properly** | GAP-3-adjacent, UX | "Show me the guest's 80×25 screen and its mode-13h framebuffer as real pixels." | Medium to large: mode 13h already exists in the reference; the work is rendering and a palette, not emulation. | T7 UI, guest-screen model in the core | **Do it.** Required for the 6 shipped graphics examples and for `INT 10h/13h` to be visible at all. |
| 11 | **In-process device API plus socket transport** | GAP-11, smell 1, smell 5 | "Write a device in Rust as a `trait Device` with no file I/O; or write one in Python and speak the framed protocol over a Unix socket." | Medium: the trait is ~20 lines; the socket transport and a framed protocol are the real cost. | T5 core, T8 packaging | **Do the trait now, the socket later.** The trait is what makes first-party devices (traffic lights, robot) testable. |
| 12 | **8087 co-processor extension** | GAP-8 | "Run a program that uses `FLD`/`FSTP`/`FADD` on an 8087 stack." | Large: an 8-level register stack, x87 tag word, control/status word, and correct `FINIT`/`FCLEX` semantics, all in a core that is currently `#![forbid(unsafe_code)]` and dependency-free. | T5 core complete, #1 (timing) | **Later, and deliberately.** Real value, but it is a second processor to validate and it cannot be done credibly before the 8086 core is complete and covered. |

### 3.1 Scope traps — ranked proposals we should *not* build now

| Proposal | Why it is a trap |
|---|---|
| **WebAssembly export** | The distribution argument is genuinely good and the core is dependency-free and deterministic, so the technical path is short. But it is a *packaging* task, not an emulation task, and it competes for attention with the test runner. Doing it before the core is covered produces a fast way to ship a wrong emulator. **Verdict: schedule at T8+, after #5 coverage is high. It is a good idea at the wrong time, not a bad idea.** |
| **Shared-nothing remote attach (gdb-style RSP)** | This buys remote debugging for a single-user teaching emulator whose primary use is local, and it requires a frozen, stable core API plus a state serialisation format before a single line can be written usefully. **Verdict: do not build. #7 (save/load state) delivers 80% of the benefit at 10% of the cost.** |
| **Full IBM-PC BIOS/DOS compatibility** | The reference implements 38 `AH` functions across 11 vectors. A complete DOS is unbounded: FCBs, handles, device drivers, INT 21h/4Dh, network redirector. **Verdict: never build. Match the recovered function set and stop.** |
| **Any 80286-or-later support** | The project goal is an 8086/8088 teaching emulator. 286 protected mode, A20, and the "unreal mode" are all large, and none of them teach 8086 assembly. **Verdict: never build.** |
| **FASM as an in-process dependency** | The reference delegates via `#fasm#` to a separately licensed binary (`FINDINGS §License status`, README note on FASM). Vendoring it creates a licence obligation and a second assembler to keep working. **Verdict: never build. Our own assembler plus a documented `#fasm#` escape hatch is enough.** |

---

## 4. UI/UX architecture — TUI vs GUI

### 4.1 What a debugger actually needs on screen at once

A debugger that fits its state on one screen shows six things. Ranked by how often
the user looks at them:

| Rank | Panel | Why it earns its space | Typical size |
|---|---|---|---|
| 1 | Disassembly at `CS:IP` | The one panel consulted every single step. | ≥ 60% of height |
| 2 | Registers + FLAGS | Read after every step; the primary feedback channel. | 4–6 columns × 20 rows |
| 3 | Memory at a followed address | The other half of "what changed". | 3–4 columns × 16 rows |
| 4 | Source line mapping | Only meaningful for code compiled from source; a strip of context, not a full editor. | 3–5 lines |
| 5 | Guest screen | The program's own output. Toggleable, never competing for space. | Full-screen takeover |
| 6 | Stack / variables / breakpoints / devices | Important but not continuous. Each is a pane you open. | On demand |

**Conclusion that drives the layout:** six panels do not fit in 80×25, and a
debugger that hides panels behind a keypress is a debugger beginners will not
find. So: **three persistent panes plus one transient overlay**, with a focus
model. The reference solves the same problem with 37 separate windows — the
module map in `FINDINGS §Complete module map` is a list of 37 forms, most of them
small modal viewers. That is the main UI lesson to invert.

### 4.2 Honest comparison

| Dimension | TUI (ratatui) | GUI (egui/eframe) |
|---|---|---|
| **Primary content** | Text: disassembly, hex, expressions. The reference's entire domain is text. | The same text, in widgets. A canvas GUI has to *build* a text grid before it can show anything. |
| **Dependency cost** | ratatui + crossterm. Text only. | eframe pulls winit, wgpu, and a shader toolchain; wgpu is the single heaviest dependency this project could take. The core's "no dependencies" promise survives a TUI; it does not survive eframe without a caveat. |
| **Binary size and build** | Small, fast to compile, portable. | Large, slow to compile, and a real source of per-platform breakage (Wayland/X11/GBM on Linux). |
| **Accessibility** | Strong, if built carefully: text scales with the terminal font, works with a screen reader if colour is never the sole carrier of meaning, works at any width. | egui renders to a canvas; screen readers see an unlabelled bitmap. Accessibility work is a separate project. |
| **Remote / headless** | Free: SSH, tmux, `script(1)`. A layout is a serialisable grid. | Not available. |
| **Beginner learning curve** | Higher at first — a beginner must learn the keymap. Lower ceiling once learned, because the text on screen is the same text they will find in a manual. | Lower at first — buttons are self-labelling. Higher ceiling, because a GUI hides the addresses behind panels and a beginner never learns to read a raw address. |
| **Multi-window / MDI** | No. Force a decision about what is on screen. | Easy — and that is exactly the failure mode to avoid (`FINDINGS §Complete module map`, 37 forms). |
| **The guest's own screen** | Renders naturally as a character grid (mode 13h text, and mode 13h graphics as a block-character approximation or a colour grid). | Can render real pixels easily. This is the TUI's one genuine weakness. |
| **Copy-paste** | Everything on screen is selectable text. | Requires per-widget plumbing. |

### 4.3 Recommendation

**TUI primary, GUI secondary.**

The TUI is primary because the product's entire value surface is text, because
the stack is already committed (`odd/tasks/emu8067-foss.md` T4 records the
Rust/ratatui/egui decision), because ratatui keeps the dependency budget honest,
and because a terminal is the only frontend that works over SSH and in CI — which
the headless test runner (§3, rank 9) depends on.

The GUI is secondary and deferred, and it should be built as a *view layer over
the same panel model the TUI defines*, not as a separate design. If the panel
model is wrong, a GUI makes it wrong twice as expensively.

**Concretely: build the TUI to the point where every panel is a pure function of
machine state plus a UI state struct, then the GUI is a rendering of that struct.**
The panel model is the product; ratatui and egui are two renderers of it.

One concession: the guest's VGA screen is the one place the TUI is genuinely
weak, and the reference proves learners care about it (6 shipped graphics
examples). Plan a full-screen takeover mode for it from the start.

### 4.4 TUI layout

```
┌─ emu8067 ──────────────────────────── test.com ────────────────── ⏸ HALTED ─┐
│ 1000:0000  B4 09          MOV AH,09h                                    │  0 │  ← 1 DISASSEMBLY (focus)
│ 1000:0002  CD 21          INT 21h              ; print "Hello, world!"   │  1 │
│ 1000:0004  B8 00 4D       MOV AX,4D00h           ; hello.asm:12         │  2 │     text colour:
│ 1000:0007  CD 21          INT 21h                                        │  3 │     cyan = current IP
│ 1000:0009  C3             RET                                            │  4 │     grey = executed
│ 1000:000A  B4 4C          MOV AH,4Ch                                       │  5 │     yellow = breakpoint
│ 1000:000C  CD 21          INT 21h              ; terminate               │  6 │     red   = watchpoint
│ 1000:000E  F4             HLT                                            │  7 │
│ 1000:000F  90             NOP                                            │  8 │
│>1000:0010  8B 1E 00 02    MOV BX,[011E]           ; i dx ?                │  9 │  ← cursor
│  1000:0014  8B C3         MOV AX,BX                                       │ 10 │
│  1000:0016  83 C0 02      ADD AX,2                                        │ 11 │
│  1000:0019  3D 00 10      CMP AX,1000h                                    │ 12 │
│  1000:001C  72 F2         JB 1000:0010                                   │ 13 │
│  1000:001E  CD 20         INT 20h                                        │ 14 │
│  1000:0020  00 00         DB 00h,00h                                     │ 15 │
├─ 2 REGISTERS ──────────┬─ 3 MEMORY · 0100:0000 ─────┬─ 4 FLAGS ──────────┤ 16 │
│ AX  0001  BX  0004     │ 0100  48 65 6C 6C 6F 20 77 │ CF 0  PF 1 AF 0    │ 17 │  ← 3 MEMORY
│ CX  000A  DX  0000     │ 0101  6F 72 6C 64 21 24 0D │ ZF 0  SF 0 TF 0    │ 18 │     cursor-centred
│ SI  0000  DI  0000     │ 0102  0A 24 00 00 00 00 00 │ OF 0  IF 1 DF 0    │ 19 │     window, 16 rows
│ BP  0000  SP  FFFE     │ 0103  E8 21 00 00 00 00 00 │ ▲ last write: ZF   │ 20 │
│ CS  0100  IP  0010     │ 0104  05 06 07 08 09 0A 0B │ AX  0000  ← written │ 21 │
│ DS  0100  ES  0100     │ ─────────────────────────  │ last insn:  ADD AX,2│ 22 │
│ SS  0100               │ 0100 0000 F4 00 00 00 00 00 │                    │ 23 │
│                        │ ─────────────────────────  │                    │ 24 │
│                        │ 0100 00FF 00 00 00 00 00 00 │                    │ 25 │
│                        │ 0102 .................... | │                    │ 26 │
├─ 5 SOURCE CONTEXT ────┴────────────────────────────┴────────────────────┤ 27 │
│ hello.asm:10   mov dx, offset msg                                     │    │  ← 5 SOURCE STRIP
│ hello.asm:11   mov ah, 9                                              │    │
│ hello.asm:12   int 21h                    ◄ current                    │    │
├─ 6 STATUS ─────────────────────────────────────────────────────────────┤ 28 │
│ STEP 1 243  ·  BREAK 2  ·  WATCH 1  ·  CYC 1 842  ·  DEVICE robot.exe  │    │  ← 6 STATUS (always on)
└────────────────────────────────────────────────────────────────────────┘ 29 │
```

**Handling 1 MiB without rendering it.** Panels 2, 3 and 4 are never
materialised as full memory. Specifically:

- **Memory (panel 3)** is a *window*, not a buffer. The pane owns a cursor
  address and renders exactly `rows × 16` bytes around it. Moving the cursor is
  a pointer move, not a slice. There is no `Vec<u8>` of 1 MiB anywhere in the UI.
- **Jump, don't scroll.** `g 0100` centres the window; `G` opens a goto prompt.
  There is no scrollbar, because a scrollbar over 1 MiB implies a resolution the
  pane does not have.
- **Disassembly (panel 1)** follows `CS:IP` only. It is never asked to
  disassemble a range.
- **Variables** come from the symbol table (up to a few hundred entries), never
  from a scan of memory.
- **Gutter** shows the row address so a byte is always identifiable after it
  scrolls off.

**Keyboard affordances (TUI).**

| Key | Action | Rationale |
|---|---|---|
| `F5` / `F5` | Run / continue to next breakpoint | Matches the reference's `F5` (`docs-txt/start_unframed.txt`). |
| `F8` / `F8` | Single step | Matches the reference's `F8`. |
| `F6` / `F6` | Step back | Matches the reference's `F6`. |
| `F7` | Step into / step over | Matches the reference's step-over button. |
| `F9` | Toggle breakpoint at `CS:IP` | The undocumented feature GAP-3 identified; given the obvious hotkey. |
| `F2` / `Shift+F2` | Next / previous breakpoint | Consistent with the editor's bookmark keys in `docs-txt/editor.txt`. |
| `Tab` | Cycle pane focus | Single key moves between the three persistent panes. |
| `1` `2` `3` | Focus disassembly / registers / memory | Direct addressing beats cycling for a beginner. |
| `PageUp` / `PageDown` | Scroll the focused pane | |
| `g` then hex | Goto address or `label` in the focused pane | Labels resolve through the symbol table. |
| `e` | Edit the value under the cursor (register or memory) | The reference's double-click-to-edit, with a keyboard path. |
| `b` then expression | Set a conditional breakpoint | Closes GAP-4. |
| `w` then address | Set a watchpoint | Closes GAP-5. |
| `x` | Toggle the guest screen (full-screen takeover) | The VGA weak spot; a full takeover needs no compromise. |
| `d` | Debug command line — the reference's `c i iw o ow d t p r rf` set (`docs-txt/debug.txt`) | Preserving the reference's command vocabulary is a compatibility win for migrating users. |
| `?` | Help overlay listing every binding | Non-negotiable for the beginner curve. |
| `q` | Quit | |

**Mouse affordances (TUI).** Wheel scrolls the focused pane; click focuses a
pane; click a register or memory row to edit it (the reference's
double-click-to-edit, preserved); right-click opens a context menu per pane. No
drag-resize: terminal size is the layout.

**What the reference gets right that we must keep.** `F5`/`F8`/`F6` for
run/step/step-back (`docs-txt/start_unframed.txt`); editing a register by typing
over it (`docs-txt/emulator.txt`); the debug command vocabulary
(`docs-txt/debug.txt`); and the fact that the guest screen is a window you can
raise, not a permanent half of the layout (`emu8086.ini` `ACTIVATE_SCREEN`).

### 4.5 GUI layout

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ emu8067   test.com ● HALTED          [▶ Run] [⏭ Step] [⏮ Back] [⏹ Stop]  [?] │ ← toolbar
├─────────────────────────────────────────┬─────────────────────────────────────┤
│                                         │  REGISTERS                          │
│  DISASSEMBLY                            │  ┌────────┬────────┬────────┐      │
│  ┌───┬──────────────────┬─────────────┐ │  │ AX 0001│ BX 0004│ CX 000A│      │
│  │0000│ B4 09    MOV AH,09h          │ │  ├────────┼────────┼────────┤      │
│▓▶│0002│ CD 21    INT 21h  ; print    │ │  │ DX 0000│ SI 0000│ DI 0000│      │
│  │0004│ B8 00 4D MOV AX,4D00h        │ │  ├────────┼────────┼────────┤      │
│▓▶│0007│ CD 21    INT 21h             │ │  │ BP 0000│ SP FFFE│ IP 0010│      │
│  │0009│ C3      RET                  │ │  ├────────┴────────┴────────┤      │
│  │000A│ B4 4C    MOV AH,4Ch          │ │  │ CS 0100  DS 0100  ES 0100  │      │
│  │000C│ CD 21    INT 21h  ; term     │ │  │ SS 0100                    │      │
│▓▶│000E│ F4      HLT                  │ │  └───────────────────────────┘      │
│▓▶│000F│ 90      NOP                  │ │  FLAGS  CF0 PF1 AF0 ZF0 SF0        │
│▓▶│0010│ 8B 1E … MOV BX,[011E] ◄IP   │ │          OF0 IF1 DF0 TF0  ▲ZF      │
│  │0014│ 8B C3   MOV AX,BX            │ ├─────────────────────────────────────┤
│  │0016│ 83 C0 02 ADD AX,2            │ │  MEMORY  [0100:0000]      [Go…]    │
│  │0019│ 3D 00 10 CMP AX,1000h        │ │  ┌─────────────────────────────────┐ │
│  │001C│ 72 F2   JB 0010              │ │  │0100 48 65 6C 6C 6F 20 77 6F|H│ │
│  │001E│ CD 20   INT 20h              │ │  │0101 72 6C 64 21 24 0D 0A 24|C│ │
│  │…  │                                │ │  │0102 00 00 ···· ···· ···· ····| │ │
│  └────────────────────────────────────┘ │  └─────────────────────────────────┘ │
├─────────────────────────────────────────┴─────────────────────────────────────┤
│  SOURCE   hello.asm:10  mov dx, offset msg                                     │ ← bottom drawer
│           hello.asm:11  mov ah, 9                                              │
│           hello.asm:12  int 21h                              ◄ current         │
├─────────────────────────────────────────────────────────────────────────────────┤
│ STATUS  step 1 243 · break 2 · watch 1 · cyc 1 842 · device robot.exe          │
└─────────────────────────────────────────────────────────────────────────────────┘
        [ Watch ] [ Breakpoint ] [ Variables ] [ Stack ] [ Screen ] [ Devices ]     ← dock
```

**Keyboard affordances (GUI).** Same verbs as the TUI, because a user who
switches frontends should not relearn the vocabulary: `F5` run, `F8` step,
`F6` step back, `F9` breakpoint, `Ctrl+G` goto, `Ctrl+E` edit value, `Ctrl+D`
debug command line. Mouse is primary: click a row to focus, double-click to
edit, click a gutter marker to toggle a breakpoint, scroll wheel over any pane.

**The GUI's one real advantage** is the dock at the bottom right: variables,
stack, devices and the guest screen are one click away without a keymap. That
is the accessibility and discoverability argument for the GUI, and it is the
reason to build it second rather than never.

**Handling 1 MiB in the GUI.** Identical discipline: the memory view is a
windowed model over a cursor, rendering only visible rows. egui's `ScrollArea`
is deliberately *not* used for the memory pane, because a scroll area over
65 536 rows is both a performance trap and a UX trap. Virtual-scroll the pane
the same way the TUI does it.

---

## 5. Design smells to fix

The five smells are recorded verbatim in `FINDINGS §Design smells to fix`. Each
entry below gives the replacement architecture and the honest cost of not
matching the original.

### 5.1 Drive-root device bus requiring administrator rights

**Observed.** The device bus is `C:\emu8086.io` (port *N* ⇄ byte *N*) and the
hardware interrupt bus is `C:\emu8086.hw` (256 bytes), both in the drive root —
`FINDINGS §Virtual device bus`, `FINDINGS §Hardware interrupt bus`,
`docs-txt/io.txt`. `DEVICES/_READ_ME.txt` states the requirement in capitals:
"It is required to have read/write privileges for these two files in root
directly of drive C:".

**proposal — replacement.** Two layers, in this order of preference:

1. *In-process.* First-party devices implement a trait owned by the core-adjacent
   crate:
   ```rust
   pub trait Device {
       fn in_port(&mut self, port: u16) -> u8;
       fn out_port(&mut self, port: u16, value: u8);
       fn poll_hardware_interrupt(&mut self) -> Option<u8> { None }
   }
   ```
   Registered on a `DeviceBus` held by the frontend. Zero I/O, zero privileges,
   fully unit-testable. This is what makes the traffic-lights and robot devices
   testable rather than demo-able.
2. *Out-of-process, framed.* Devices in other languages connect over a Unix
   domain socket (Linux/macOS) or a named pipe (Windows), under a per-user path
   from `XDG_DATA_HOME`. The protocol is length-prefixed binary frames with a
   request/reply correlation id and an explicit error channel.

**Compatibility cost.** Programs that relied on hand-editing `C:\emu8086.io` to
poke ports lose that. Cost is low: the file was an undocumented side effect of a
bug (smell 5), and no documentation encourages editing it. Mitigation if
wanted: a `compat_file_bus` feature that maps the old byte-per-port layout onto
the new trait, at the cost of the race in smell 5.

### 5.2 Hard-coded drive paths

**Observed.** Virtual drives are pinned to `C:\emu8086\vdrive\`
(`FINDINGS §Design smells`, `emu8086.ini` `VDRIVE_PATH=default`); the device bus
files are pinned to `C:\emu8086.io` / `.hw`; the compiler writes to `MyBuild`
under the install directory (`docs-txt/start_unframed.txt`); the templates and
macro library live in the install directory and are **overwritten on upgrade**
(`inc/__README_IMPORTANT.txt`).

**proposal — replacement.** Nothing is hard-coded. Resolution order:
`--vdrive <path>` flag → `EMU8067_VDRIVE` environment variable →
`$XDG_DATA_HOME/emu8067/vdrive` (or `%LOCALAPPDATA%\emu8067\vdrive`) → a
directory beside the source file. Directories are created on demand, never at
install time, and never in a system location. Build output goes next to the
source file unless `--out <dir>` says otherwise. The macro library is read from
the user's own include path first, falling back to the shipped copy, so an
upgrade cannot silently destroy a customised `emu8086.inc`.

**Compatibility cost.** Effectively none. These paths were never a documented
contract; they were the symptom of a Windows-3.1-era installer. The one real
behaviour to preserve is the *shape*: `MyBuild` as a default output directory
with its sibling `examples/` and `inc/` folders is a convention users learned,
and it is worth reproducing as a default.

### 5.3 Registration and licensing path

**Observed.** A 14-day trial with `USD 19.95` personal / `295` site / `1250`
permanent, purchased through RegNow/Digital River, implemented in `frmRegister`
and `mRegistration` with payment links (`FINDINGS §License status`,
`docs-txt/register.txt`, `FINDINGS §Module map`).

**proposal — replacement.** Omit it entirely. The Unlicense has no
registration, no key, no telemetry, and no network call at startup. `UPDATE_CHECK`
in `emu8086.ini` is the corresponding smell and is also omitted: release
announcements belong on the project's own page, not in a startup poll.

**Compatibility cost.** None. Licensing is not a feature. The only thing to
carry over is the *commercial* signal — the reference's `ReadMe.txt` and licence
text show a product people paid for, which tells us the feature set below was
worth paying for.

### 5.4 Untestable core

**Observed.** The backend is 2.2 MB of native-compiled VB6 with no test seam for
the CPU core (`FINDINGS §Design smells`, item 4). The interpreter is a single
35 010-instruction function `doStep()` at `0x004eff00`
(`FINDINGS §The interpreter`) — everything about the state is reachable only
through the GUI.

**proposal — replacement.** Already partly in place and worth stating
explicitly, because it is the fix for four other smells at once:
`crates/core` has no dependencies, no I/O, and `#![forbid(unsafe_code)]`
(`crates/core/src/lib.rs`). The whole processor is a `#[derive(Clone)]` value
type with a public `step()` and an explicit `StopReason`. The frontend owns a
`Machine` and can only observe it. That gives, for free:

- exhaustive pattern matching over `StopReason` instead of a modal error dialog;
- `Machine` as a fixture in the `.asm` test runner (§3, rank 4);
- serialisable state for save/load (§3, rank 7);
- full instruction coverage measurement (§3, rank 5);
- a headless binary that has no UI to instantiate (§3, rank 9).

**Compatibility cost.** None, and it is the precondition for the entire
roadmap. The rule to hold: *the core never formats a string, never opens a
socket, and never knows a window exists.* Diagnostics are a `StopReason`; the
frontend renders it.

### 5.5 Frameless shared-file device protocol

**Observed.** The device protocol is a raw shared file: no framing, no error
channel, and read-modify-write races (`FINDINGS §Design smells`, item 5). The
hardware-interrupt bus is worse — the emulator clears byte *N* to `00` after
dispatching (`FINDINGS §Hardware interrupt bus`), which is a non-atomic
read-then-clear across two processes. Two devices on the same port conflict with
no detection mechanism (`docs-txt/io.txt`).

**proposal — replacement.** For socket transports: a 4-byte little-endian
length prefix, then a frame of `{ correlation_id: u32, op: u8, port: u16,
value: u8 }`. Requests get exactly one reply; a frame that does not decode is a
protocol error, not a silent zero. Hardware interrupts become
`poll_hardware_interrupt() -> Option<u8>` on the trait, which removes the race
entirely because there is no shared file to race on. A `Bus` holds a port→device
map and returns a typed `PortConflict` error when two devices claim one port,
turning smell 5's silent conflict into a startup diagnostic.

**Compatibility cost.** This is the highest-cost item, and it is worth being
precise about why. The seven shipped devices are standalone Windows executables
written in Visual Basic and C (`DEVICES/_READ_ME.txt`), built against `io.bas`,
`io.h`, `io.cs` etc. They cannot be recompiled. Options, in order:

1. *Accept the loss.* Reimplement the four interesting ones (traffic lights,
   stepper, robot, LED) as in-process Rust devices. The other three (simple,
   thermometer, printer) are demos. Cost: those `.exe`s stop working.
2. *Ship a compat shim.* A small `emu8067-legacy-bus` process presents the old
   `emu8086.io` / `.hw` files on disk and translates them onto the framed
   protocol. The original `.exe`s work unchanged. Cost: the race and the
   privileges of the original come back with it, for those devices only.

**Recommendation: option 1, plus option 2 as an opt-in convenience if a user
asks.** Carrying the file bus permanently to preserve seven decade-old
executables would reintroduce the exact smell we are removing — permanently, on
every platform, in order to support Windows binaries.

---

## 6. Prioritised roadmap

Phases are ordered by dependency, not by excitement. The rule is that nothing
in a later phase is allowed to make an earlier phase's tests go red.

| Phase | Contents | Ties to | Exit criterion |
|---|---|---|---|
| **P0 — Make the core testable before it grows** | Finish T5: all 8086 opcodes, exact flag semantics, segment/offset rules. Add §3 rank 5 (coverage report) and rank 9 (headless binary) *immediately*, because both are cheap now and expensive later. Add rank 6 (8088) and rank 7 (state save/load) while the core is still the only thing that exists. | **T5** | `cargo run -- coverage` emits a table showing every unexecuted opcode; `emu8067 run file.com --max-steps N` exits non-zero on a fault. All 85 shipped examples assemble and run to `HLT` with no unknown-opcode skip. |
| **P1 — The assembler, complete and strict** | T6: full directive set, MASM/TASM syntax, `STRICT_SYNTAX` equivalent with the 13 recovered rules, macro engine, `#make_com#`/`#make_exe#`/`#make_bin#`/`#make_boot#`, `.symbol`/`.debug`/`.binf` sidecars. Disassembler round-trip. | **T6** | An example assembled by emu8067 is byte-identical to the same source assembled by MASM; the reverse holds for the recovered templates. Disassembly of an output re-assembles to the same bytes. |
| **P2 — The test harness that makes everything after it safe** | §3 rank 4: `.asm` test runner, assertion convention, headless CI over all 85 examples plus new regression cases. Add §3 rank 1 (timing table) now that the core is complete and covered. | **T5, T6, T8** | CI is green on three platforms and a deliberately broken flag change turns it red in a named test. |
| **P3 — A debugger that is actually a debugger** | T7 core: §3 rank 2 (expression evaluator), rank 3 (watchpoints and conditional breakpoints). The panel model as a pure function of state, per §4.3. The CLI debugger command vocabulary from `docs-txt/debug.txt`. | **T7** | `b AX==0x1234 && CS==0x0200` stops at exactly that address; `w 0100:0004` fires on write and on read-only access reports why it did not. |
| **P4 — The TUI** | §4.4 layout: disassembly / registers+flags / memory, source strip, status line, guest-screen takeover, help overlay. Guest screen model plus §3 rank 10 (16-colour and 256-colour rendering). | **T7** | The 85 examples can each be opened, run, and inspected without a mouse. Text scales with the terminal font and stays usable at 80×24. |
| **P5 — Devices, extensibility and packaging** | In-process `Device` trait and the four reimplemented devices; framed socket transport; `custom_memory_map.inf` equivalent; virtual floppy; XDG paths. T8: packaging and CI for Linux/macOS/Windows. | **T8**, P0 | A user-written device in any language works over a socket with no privileges and no drive root. Three platform artifacts from one command. |
| **P6 — Differentiation** | §3 rank 8 (record and replay), §3 rank 10 (remaining VGA modes), §3 rank 12 (8087) if and only if P0–P5 are stable and the 8086 core is at full coverage. | — | A recorded trace replays identically on a different machine. |
| **P7 — Optional distribution** | WebAssembly export (§3.1), last. | **T8** | A `.wasm` build runs the emulator in a browser with the TUI as an xterm.js canvas. Only attempted once coverage is high. |

### 6.1 What should wait

| Item | Wait until | Why |
|---|---|---|
| §3 rank 10, VGA graphics rendering | P4 | The core model must exist first; this is a renderer, not an emulator. |
| §3 rank 11, socket device transport | P5 | Only meaningful once first-party devices prove the trait. |
| §3 rank 8, record and replay | P6 | Useless without a stable state format (rank 7) and coverage data (rank 5). |
| GUI frontend (§4.5) | P6 at the earliest | The panel model must be proven in the TUI first; the GUI renders the same struct. |
| WebAssembly export | P7 | Packaging, not emulation. |

### 6.2 What should never be built

| Item | Reason |
|---|---|
| Registration, licensing, activation, or any network call at startup | §5.3. The licence is The Unlicense. |
| A device bus that requires elevated privileges | §5.1 and §5.5. Permanent unless someone files a concrete, unanswerable bug against the design. |
| A vendored or forked FASM | Licence obligation plus a second assembler to maintain (§3.1). |
| 80286 or later support | Out of scope by definition; teaches nothing about 8086. |
| A complete DOS/BIOS | Unbounded (§3.1). Match the recovered 38-function surface and stop. |
| A remote-attach debug protocol | §3.1. Rank 7 delivers the benefit at a fraction of the cost. |
| Per-device protocol compatibility shim for the original VB6 device `.exe`s as a default | §5.5. Opt-in only; as a default it reimports the smell it was meant to retire. |

---

## Open questions

Decisions that need the maintainer before implementation starts.

| # | Question | Why it is blocking | Recommendation |
|---|---|---|---|
| Q1 | **Crate naming is inconsistent.** The project, README and this brief say `emu8067`; `crates/core/Cargo.toml` declares `emu8086-core`; `odd/tasks/emu8067-foss.md` T4 records `emu8066-core`. Three different names for one product. | Every published artifact, crate path and doc link inherits the error. | Pick one — `emu8067` is the project's stated name — and rename the crate before anything is published. |
| Q2 | **Is the 8088 mode in or out?** GAP-1 is a documented absence, and `crates/core/src/lib.rs` already advertises "Intel 8086/8088". | It changes the `Machine` API from day one; retrofitting it is far more expensive than declaring it now. | **In.** Small cost, and the crate doc already commits to it. |
| Q3 | **Does the expression evaluator resolve assembly symbols, or only registers and memory?** | Determines whether §3 ranks 2 and 3 need the assembler finished first, and whether P3 can start in parallel with T6. | Resolve symbols. A breakpoint on a bare address is not what a learner writes; they write a variable name. It also makes the `.symbol` sidecar load-bearing instead of decorative. |
| Q4 | **Should the in-process `Device` trait live in `crates/core` or a new crate?** | If it is in the core, the core grows an extension point it must maintain; if it is separate, the core needs a dependency-free way to expose a bus. | Separate crate (`emu8067-devices`), with the core exposing only a plain `Bus` of closures or a minimal trait it defines. Keeps `crates/core` at zero dependencies. |
| Q5 | **Is the legacy device `.exe` compatibility shim wanted at all?** §5.5 lays out both options. | Determines whether P5 includes a sub-project. | Default off, on request. See §5.5. |
| Q6 | **Should WebAssembly be a committed goal or an aspiration?** | It changes the dependency policy for P7 (wasm-bindgen vs a pure-wasm core) and has to be decided before the crate layout is finalised. | Decide at P7. Keep `crates/core` compilation-target-agnostic (no `std::time`, no filesystem) so the choice stays open — which it currently already is. |
| Q7 | **What is the compatibility promise for the `.binf` and `custom_memory_map.inf` formats?** Both are documented plain-text formats from the reference and both are cheap to reproduce exactly. | If the answer is "reproduce exactly", P1 and P5 inherit a hard file-format requirement and need their own tests. | Reproduce both exactly. They are ASCII, they are documented, and they cost almost nothing to keep compatible — the same reason `.binf` exists at all is that a user can hand-edit it. |
| Q8 | **Is the reference's `STRICT_SYNTAX` default right for us?** `emu8086.ini` ships it `true`, which rejects `MOV DS, 100` for beginners. | It is a teaching tool; rejecting a beginner's first segment load with a type error may be hostile. But the recovered diagnostics are the best error text this project will ever write. | Keep strict, keep the recovered diagnostics, and add a one-line hint to each error naming the working form. The reference's own message for `MOV DS, 100` already does this ("general purpose register should be used, for example `MOV AX, 100` / `MOV DS, AX`"). |

---

## Gaps in this research

| Gap | Detail |
|---|---|
| Breakpoint behaviour is undocumented | `frmTrapping` and `tmp\deleted_bkp_` (`FINDINGS §Behavioral contracts`) prove a mechanism exists, but no `re/docs-txt/` file documents its syntax or hotkey. GAP-3 records this as an absence of *documentation*, not of *function*. The behaviour could not be recovered without reverse-engineering the binary, which is Track 3's scope. |
| Editor scope | §1.4 inventories the assembler's editor features only where they affect assembly (`docs-txt/editor.txt` lists ~70 editor hotkeys). Whether to inherit the source editor at all, or ship a plain text editor, is not decided here — see Q8's neighbour question, and treat it as an open item for the maintainer. |
| Device `.exe` protocols | Only the traffic lights, stepper motor and robot protocols are documented (`docs-txt/io.txt`). `Simple.exe`, `LED_Display.exe`, `Thermometer.exe`, `Printer.exe` and `VGA_STATE.exe` have no documented port map, so their reimplementations cannot be specified from these sources. |
| No documentation track input | Track T9a (`odd/research/documentation-guide.md`) owns the 12 tutorials and the numbering-systems material. This document assumes those tutorials remain the pedagogical spine and does not duplicate them. |