# Varan: ARM emulator (KoMoDo alternative)

A Windows desktop app in Rust for writing, assembling, running and debugging ARM
assembly for the COMP1054 course. It replaces KoMoDo. The GUI uses eframe/egui
0.37, and rfd provides the file dialogs.

## Commands

```
cargo run                      # debug GUI (starts with a sample program)
cargo run --release -- file.s  # open a file
cargo test                     # all tests (library + GUI-side tests)
cargo test preview -- --ignored  # render the UI offscreen to target/preview/*.png (needs a GPU)
cargo build --release          # -> target\release\varan.exe (single file)
```

Keep the build free of warnings, and make sure `cargo test` passes before you finish.

## Specs

- `docs/ARMInst.pdf` is the COMP1054 instruction summary. It defines the syntax,
  directives, pseudo-ops and SWI 0-4 behaviour that must stay KoMoDo-compatible.
- `docs/ARM_doc.pdf` is the ARM7TDMI datasheet, the reference for instruction encodings.
  Extract its text with `pdftotext -layout`.

## Layout

The library crate `varan` (`src/lib.rs`) has no GUI dependencies:
- `asm.rs` is the two-pass assembler. Pass 1 assigns addresses, symbols and literal
  pool slots. Pass 2 encodes. It produces a `Program` (segments, entry, symbols,
  addr<->line maps, listing). `evaluate()` is the expression evaluator, also used by the UI.
- `cpu.rs` contains `Memory` (sparse 4 KiB pages, full 32-bit space, zero-filled)
  and `Cpu` (ARMv4 ARM state). `regs[15]` holds the *current* instruction address,
  and operand reads of R15 return +8. SWIs 0-4 are implemented here. `step()` returns a `StepResult`.
- `disasm.rs` is the pre-UAL disassembler, used by the listing and the "next instruction" panel.
- `tests.rs` covers encoding checks against reference machine code, end-to-end
  programs, and every file in `examples/` (via `include_str!`).

The binary is `src/main.rs`:
- `app.rs` holds the app state, the run/pause/step/step-over state machine, and all panels.
- `editor.rs` is the code editor: highlighting, the gutter with breakpoints,
  tab-stop rendering, and `install_fonts`.
- `fileformat.rs` loads and saves files losslessly. It remembers CRLF/LF, a UTF-8
  BOM and Latin-1, so an unedited file saves byte-identical.
- `theme.rs` has the dark and light colour palettes and the egui widget styling.
- `widgets.rs` has vector-drawn icons (no emoji fonts), toolbar buttons, tab bars and status pills.
- `cheatsheet.rs` is the F1 Instruction Reference window (the COMP1054 summary as searchable
  tables). It also embeds `docs/ARMInst.pdf` for Help -> Open COMP1054 Summary (PDF).
- UI fonts load from `%WINDIR%/Fonts` at runtime (Segoe UI, Cascadia Mono or Consolas) in
  `editor::install_fonts`, falling back to the egui defaults.

## Assembler conventions

- A label is a word at column 0 that isn't a mnemonic, OR any word followed by
  `:`, OR a word followed by another mnemonic or directive (so `str DEFB "x"` works).
  Labels are case-sensitive, while mnemonics and registers are not.
- To parse a mnemonic, try the longest base first, then accept the suffixes in
  either order (`ADDNES` and `ADDSNE`). This is what keeps `BLE` = B+LE,
  `BLS` = B+LS and `BLLT` = BL+LT working. Run `mnemonic_suffix_ambiguity` after any change here.
- Instructions and `DEFW` auto-align to 4 bytes. A label on a line by itself binds
  to the *next* statement's address, after alignment.
- Immediates that can't be encoded fall back to MOV<->MVN, CMP<->CMN, ADD<->SUB,
  AND<->BIC, ADC<->SBC.
- `LDR Rd,=x` uses MOV/MVN when it can. Otherwise it uses a literal pool, which is
  flushed at `LTORG`, at `ORIGIN`, or at the end of the source.
- Breakpoints are stored by **source line**. They become addresses through
  `line_to_addr`, but only for lines that contain code.

## Gotchas

- **egui 0.37 API:** the app implements `App::ui(&mut self, ui, frame)` and gets
  the root `Ui`. Panels are `egui::Panel::{top,bottom,left,right}(id).show(ui, ..)`
  and `CentralPanel::default().show(ui, ..)`. `Id::new` is deprecated, so use
  `Id::unique`. Cursor indices are `CharIndex` (use `.0`). If you're unsure of an
  API, grep `~/.cargo/registry/src/*/egui-0.37.0/src` rather than guessing.
- **Tabs:** egui draws `\t` at a fixed 4-space width. The editor never converts
  tabs. Instead, `TabJob` draws each tab in the `varan_tab` font family (a copy of
  the monospace font with zero tab width) and adds `leading_space` to reach the
  next tab stop. Never rewrite users' whitespace.
- **Headless egui tests:** use `ctx.begin_pass(RawInput::default())`, and finish
  with `ctx.end_pass().textures_delta.clear()`. Without that clear, the test panics.
- **`src/app.rs` has CRLF line endings (other sources are LF); keep it that way.** Multi-line string replacements in
  Python or sed silently fail to match, and backslash escapes in bash heredocs
  passed to Python get mangled. Use the Edit/Write tools for code edits.
- **Do not drive the GUI with synthetic keyboard/mouse input or full-screen
  screenshots.** The user works on this machine, and keystrokes went to another
  app. Verify behaviour with unit tests instead. Headless egui layout tests work
  for UI rendering checks.
