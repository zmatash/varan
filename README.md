# Varan: an ARM emulator for teaching

A modern Windows alternative to KoMoDo for writing, assembling, running and
debugging ARM assembly. It is written in Rust with an [egui](https://github.com/emilk/egui) interface.

## Download

Get the latest build from the [Releases page](https://github.com/zmatash/varan/releases/latest):

- **Windows:** download `Varan-*-windows-x64.exe` and run it. It needs no installer. If
  SmartScreen warns you, click *More info* and then *Run anyway*.
- **macOS** (Apple Silicon and Intel, macOS 11+): open the `.dmg` and drag Varan to Applications.
  The app isn't notarised yet, so the first time you open it, right-click Varan and choose *Open*.
  If macOS says it is "damaged", run `xattr -cr /Applications/Varan.app`.
- **Linux** (x86-64): extract the `.tar.gz` and run `./varan`.

## Making a release

Bump `version` in `Cargo.toml`, commit, then `git tag v0.2.0 && git push origin v0.2.0`.
GitHub Actions tests and builds all three platforms, then creates a **draft** release for you to publish.

## Running from source

```
cargo run --release                 # start with a sample program
cargo run --release -- prog.s       # open a file
```

The built program is `target\release\varan.exe`. It is a single file, so you
can copy it anywhere.

## Features

- **Editor** with ARM syntax highlighting, line numbers, and click-to-set breakpoints in the gutter
- **Assembler** for KoMoDo syntax, with error messages linked to source lines (Problems tab)
- **Debugger**: run, pause, single step, step over `BL` calls, breakpoints, reset, and adjustable speed (1 instr/s up to full speed)
- **Registers** in hex or decimal. Values changed by the last step are highlighted, and you can double-click a register to edit it
- **Flags** N Z C V: click a flag to toggle it
- **Next instruction** with its disassembly and whether its condition will pass
- **Stack** view of the words at SP
- **Memory** viewer showing bytes or words with ASCII. Jump to any address or label, and recent writes are highlighted
- **Listing / Disassembly** of the address, machine code, disassembly and source for every instruction
- **Console** for SWI output and keyboard input for `SWI 1`
- Dark and light themes, and an adjustable editor font size (Ctrl +/-)
- Files are kept exactly as written: tabs line up on real tab stops (8 columns by default, which you can change under View > Tab width), and saving keeps the original tabs, line endings and encoding

## Keyboard shortcuts

| Key        | Action                        |
|------------|-------------------------------|
| F7 / Ctrl+B| Assemble                      |
| F5         | Run / continue                |
| Shift+F5   | Pause                         |
| F11        | Step one instruction          |
| F10        | Step over (runs `BL` calls)   |
| F9         | Toggle breakpoint at cursor   |
| Ctrl+R     | Reset                         |
| Ctrl+N/O/S | New / Open / Save             |

On macOS, use Cmd in place of Ctrl. The function keys may need `fn` held down.

F5 and F11 assemble automatically if the source has changed.

## Supported language (COMP1054 subset and more)

**Instructions:** `ADD ADC SUB SBC RSB RSC AND ORR EOR BIC MOV MVN CMP CMN TST TEQ`,
`MUL MLA UMULL UMLAL SMULL SMLAL`, `LDR STR LDRB STRB LDRH STRH LDRSB LDRSH`,
`LDM STM` (`IA IB DA DB FD ED FA EA`), `B BL BX SWI SWP MRS MSR`.

- Any instruction can be conditional (`ADDEQ`), and data-processing instructions take `S` (`ADDNES` or `ADDSNE`)
- Operand 2 can be `#imm`, `Rm`, `Rm, LSL #n` or `Rm, LSL Rs` (LSL/ASL, LSR, ASR, ROR, RRX)
- Addressing modes: `[R0]`, `[R0, #4]`, `[R0, R1, LSL #2]`, `[R0, #4]!`, `[R0], #4`, `label`

**Pseudo-ops:** `NOP`, `ADR`, `ADRL`, `LDR Rd, =value`, `PUSH {..}`, `POP {..}`,
`LSL/LSR/ASR/ROR/RRX Rd, Rm, ...`. Immediates that can't be encoded are
swapped automatically: `MOV #-1` becomes `MVN`, `CMP #-n` becomes `CMN`, and the same applies to `ADD`/`SUB` and `AND`/`BIC`.

**Directives:** `ENTRY`, `ORIGIN addr`, `ALIGN`, `name EQU value`, `DEFW`, `DEFH`,
`DEFB 1, 2, "text", 0`, `DEFS size`, `LTORG` (place a literal pool). `DCD`, `DCB`,
`ORG` and `SPACE` are accepted as aliases.

**Expressions:** decimal, `0x1F`, `&1F`, `0b101`, `'A'`, labels, and `+ - * / % << >> & | ^ ~ ( )`.

**Labels** start in column 1, optionally followed by `:`. Labels are case
sensitive, while mnemonics and registers are not. `;` starts a comment.

## SWI calls (as in KoMoDo)

| SWI | Effect                                           |
|-----|--------------------------------------------------|
| 0   | Print the character in R0                        |
| 1   | Read a character into R0 (waits for console input) |
| 2   | Halt                                             |
| 3   | Print the zero-terminated string at address R0   |
| 4   | Print R0 as a signed decimal integer             |

When you press Enter in the console input box, the line is sent followed by a
newline character (10).

## Machine model

- ARMv4 (ARM7TDMI) ARM-state instruction set. Thumb, coprocessors and exception modes are not emulated
- The full 32-bit address space is available, and unused memory reads as 0
- All registers start at 0, and PC starts at `ENTRY` (or the first instruction if there is no `ENTRY`)
- R15 reads as the current instruction address + 8, as on real hardware

## Project layout

```
src/asm.rs      two-pass assembler
src/cpu.rs      CPU + memory emulator and SWI handling
src/disasm.rs   disassembler
src/app.rs      GUI
src/editor.rs   code editor widget
src/tests.rs    tests (cargo test)
examples/       sample programs
```
