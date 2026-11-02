use crate::asm::{assemble, encode_imm};
use crate::cpu::{Cpu, StepResult};
use crate::disasm::disassemble;

fn load(src: &str) -> Cpu {
    let prog = assemble(src).unwrap_or_else(|e| panic!("assembly failed: {e:?}"));
    let mut cpu = Cpu::new();
    for (a, bytes) in &prog.segments {
        cpu.mem.load(*a, bytes);
    }
    cpu.regs[15] = prog.entry;
    cpu
}

fn run_with_input(src: &str, input: &str) -> Cpu {
    let mut cpu = load(src);
    cpu.input.extend(input.bytes());
    for _ in 0..1_000_000 {
        match cpu.step() {
            StepResult::Ok => {}
            StepResult::Halted => return cpu,
            other => panic!("unexpected {other:?} at pc=0x{:X}", cpu.pc()),
        }
    }
    panic!("program did not halt");
}

fn run(src: &str) -> Cpu {
    run_with_input(src, "")
}

fn asm_word(line: &str) -> u32 {
    let prog = assemble(&format!("    {line}\n")).unwrap_or_else(|e| panic!("{line}: {e:?}"));
    let b = &prog.segments[0].1;
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

#[test]
fn encodings_match_reference() {
    // Reference encodings from GNU as for ARMv4.
    let cases: &[(&str, u32)] = &[
        ("MOV R0, #1", 0xE3A00001),
        ("MOV R0, #-1", 0xE3E00000), // -> MVN R0, #0
        ("ADD R1, R4, R5, LSL #3", 0xE0841185),
        ("ADD R1, R4, R5, LSL R6", 0xE0841615),
        ("SUBS R0, R0, #1", 0xE2500001),
        ("ADDNES R0, R0, R1", 0x10900001),
        ("CMP R0, #10", 0xE350000A),
        ("CMP R0, #-1", 0xE3700001), // -> CMN R0, #1
        ("MUL R0, R1, R2", 0xE0000291),
        ("MLA R0, R1, R2, R3", 0xE0203291),
        ("LDR R0, [R1]", 0xE5910000),
        ("LDR R0, [R1, #4]!", 0xE5B10004),
        ("LDR R0, [R1], #4", 0xE4910004),
        ("LDR R2, [R0, R1, LSL #3]", 0xE7902181),
        ("STRB R0, [R1, #-1]", 0xE5410001),
        ("LDRH R0, [R1, #2]", 0xE1D100B2),
        ("LDRSB R0, [R1]", 0xE1D100D0),
        ("STMFD SP!, {R0-R3, LR}", 0xE92D400F),
        ("LDMFD SP!, {R0-R3, PC}", 0xE8BD800F),
        ("PUSH {R4, LR}", 0xE92D4010),
        ("SWI 2", 0xEF000002),
        ("BX LR", 0xE12FFF1E),
        ("MOV R0, R1, RRX", 0xE1A00061),
        ("LSL R0, R1, #2", 0xE1A00101),
        ("MRS R0, CPSR", 0xE10F0000),
        ("MSR CPSR_f, R0", 0xE128F000),
        ("UMULL R0, R1, R2, R3", 0xE0810392),
        ("SWP R0, R1, [R2]", 0xE1020091),
        ("NOP", 0xE1A00000),
        ("BIC R0, R0, #0xFF000000", 0xE3C004FF),
        ("AND R0, R0, #0xFFFFFF00", 0xE3C000FF), // -> BIC
    ];
    for (src, want) in cases {
        let got = asm_word(src);
        assert_eq!(got, *want, "{src}: got 0x{got:08X}, want 0x{want:08X}");
    }
}

#[test]
fn mnemonic_suffix_ambiguity() {
    // BLE = B + LE, BLS = B + LS, BLLT = BL + LT
    assert_eq!(asm_word("BLE 0") >> 24, 0xDA);
    assert_eq!(asm_word("BLS 0") >> 24, 0x9A);
    assert_eq!(asm_word("BLLT 0") >> 24, 0xBB);
    assert_eq!(asm_word("BICS R0, R0, #1") >> 20, 0xE3D);
    assert_eq!(asm_word("MOVEQS R0, #1") >> 20, 0x03B);
    assert_eq!(asm_word("MOVSEQ R0, #1") >> 20, 0x03B);
    assert_eq!(asm_word("LDREQB R0, [R1]") >> 20, 0x05D);
    assert_eq!(asm_word("LDRBEQ R0, [R1]") >> 20, 0x05D);
}

#[test]
fn disassembly_round_trip() {
    let lines = [
        "MOV R0, #1",
        "ADD R1, R4, R5, LSL #3",
        "SUBS R0, R0, #1",
        "CMP R0, #0xFF",
        "LDR R0, [R1, #4]!",
        "LDR R0, [R1], #4",
        "STRB R0, [R1, #-1]",
        "STMFD SP!, {R0-R3, LR}",
        "LDMFD SP!, {R0-R3, PC}",
        "MUL R0, R1, R2",
        "SWI 2",
        "LDRH R0, [R1, #2]",
    ];
    for l in lines {
        let w = asm_word(l);
        let d = disassemble(w, 0, &|_| None);
        let w2 = asm_word(&d);
        assert_eq!(w, w2, "{l} -> {d}");
    }
}

#[test]
fn immediate_encoding() {
    assert!(encode_imm(0xFF).is_some());
    assert!(encode_imm(0xFF00_0000).is_some());
    assert!(encode_imm(0xF000_000F).is_some());
    assert!(encode_imm(0x101).is_none());
    assert!(encode_imm(0x1FE).is_none()); // needs an odd rotation
    assert!(encode_imm(0x3FC).is_some());
}

#[test]
fn hello_world() {
    let cpu = run(r#"
        ADR R0, msg
        SWI 3
        SWI 2
msg     DEFB "Hello, world!\n", 0
"#);
    assert_eq!(cpu.output, "Hello, world!\n");
}

#[test]
fn loop_and_print_number() {
    let cpu = run(
        "
; sum 1..10
        MOV R0, #0
        MOV R1, #10
loop    ADD R0, R0, R1
        SUBS R1, R1, #1
        BNE loop
        SWI 4
        SWI 2
",
    );
    assert_eq!(cpu.output, "55");
}

#[test]
fn negative_number_print() {
    let cpu = run("  MOV R0, #-5\n  SWI 4\n  SWI 2\n");
    assert_eq!(cpu.output, "-5");
}

#[test]
fn function_call_with_stack() {
    let cpu = run(
        "
        ENTRY
        MOV SP, #0x10000
        MOV R0, #5
        BL fact
        SWI 4
        SWI 2

; recursive factorial of R0 -> R0
fact    STMFD SP!, {R4, LR}
        MOV R4, R0
        CMP R0, #1
        MOVLE R0, #1
        BLE done
        SUB R0, R0, #1
        BL fact
        MUL R0, R4, R0
done    LDMFD SP!, {R4, PC}
",
    );
    assert_eq!(cpu.output, "120");
    assert_eq!(cpu.regs[13], 0x10000);
}

#[test]
fn ldr_literal_and_data() {
    let cpu = run(
        "
size    EQU 4
        LDR R0, =0x12345678
        LDR R1, =table
        LDR R2, [R1, #size]
        LDR R3, =-1
        LDR R4, count
        SWI 2
table   DEFW 10, 20, 30
count   DEFW 99
",
    );
    assert_eq!(cpu.regs[0], 0x12345678);
    assert_eq!(cpu.regs[2], 20);
    assert_eq!(cpu.regs[3], 0xFFFF_FFFF);
    assert_eq!(cpu.regs[4], 99);
}

#[test]
fn read_input_echo() {
    let cpu = run_with_input(
        "
loop    SWI 1
        CMP R0, #10
        SWIEQ 2
        SWI 0
        B loop
",
        "abc\n",
    );
    assert_eq!(cpu.output, "abc");
}

#[test]
fn waits_for_input() {
    let mut cpu = load("  SWI 1\n  SWI 2\n");
    assert_eq!(cpu.step(), StepResult::WaitingForInput);
    assert_eq!(cpu.pc(), 0);
    cpu.input.push_back(b'x');
    assert_eq!(cpu.step(), StepResult::Ok);
    assert_eq!(cpu.regs[0], b'x' as u32);
}

#[test]
fn origin_and_entry() {
    let prog = assemble(
        "
        ORIGIN 0x1000
data    DEFB 1, 2, 3
        ALIGN
        ENTRY
start   ADR R0, data
        SWI 2
",
    )
    .unwrap();
    assert_eq!(prog.entry, 0x1004);
    assert_eq!(prog.symbols["start"], 0x1004);
    assert_eq!(prog.symbols["data"], 0x1000);
}

#[test]
fn flags_and_conditions() {
    let cpu = run(
        "
        MOV R0, #0x80000000
        ADDS R1, R0, R0      ; overflow: C=1 V=1 Z=1
        MOVCS R2, #1
        MOVVS R3, #1
        MOVEQ R4, #1
        MOV R5, #3
        CMP R5, #5
        MOVLT R6, #1
        MOVLO R7, #1
        MOVGT R8, #1
        SWI 2
",
    );
    assert_eq!(&cpu.regs[2..9], &[1, 1, 1, 3, 1, 1, 0]);
}

#[test]
fn shifts_and_rrx() {
    let cpu = run(
        "
        MOV R0, #1
        MOV R1, R0, LSL #31
        MOV R2, R1, ASR #31
        MOV R3, R1, LSR #31
        MOVS R4, R0, LSR #1   ; C = 1
        MOV R5, R4, RRX       ; R5 = 0x80000000
        MOV R6, #4
        MOV R7, R0, LSL R6
        SWI 2
",
    );
    assert_eq!(cpu.regs[1], 0x8000_0000);
    assert_eq!(cpu.regs[2], 0xFFFF_FFFF);
    assert_eq!(cpu.regs[3], 1);
    assert_eq!(cpu.regs[5], 0x8000_0000);
    assert_eq!(cpu.regs[7], 16);
}

#[test]
fn string_length_bytes() {
    let cpu = run(
        r#"
        ADR R1, str
        MOV R0, #0
next    LDRB R2, [R1], #1
        CMP R2, #0
        ADDNE R0, R0, #1
        BNE next
        SWI 4
        SWI 2
str     DEFB "komodo", 0
        ALIGN
"#,
    );
    assert_eq!(cpu.output, "6");
}

#[test]
fn errors_reported_with_lines() {
    let err = assemble("  MOV R0, #1\n  FOO R1\n  B nowhere\n  MOV R0, #0x101\n").unwrap_err();
    let lines: Vec<usize> = err.iter().map(|e| e.line).collect();
    assert_eq!(lines, vec![1, 2, 3]);
}

#[test]
fn label_on_own_line_is_aligned() {
    let prog = assemble("s DEFB \"abc\"\nlbl\n  MOV R0, #1\n").unwrap();
    assert_eq!(prog.symbols["lbl"], 4);
}

#[test]
fn adrl_far() {
    let cpu = run(
        "
        ADRL R0, far
        LDR R1, [R0]
        SWI 2
        DEFS 0x2000
far     DEFW 0xCAFE
",
    );
    assert_eq!(cpu.regs[1], 0xCAFE);
}

#[test]
fn tabs_and_colon_labels() {
    let cpu = run("max\tEQU\t3\n\tMOV\tR0, #max\nagain:\tSUBS\tR0, R0, #1\n\tBNE\tagain\n\tSWI\t2\n");
    assert_eq!(cpu.regs[0], 0);
}

#[test]
fn block_transfer_modes() {
    let cpu = run(
        "
        MOV R0, #0x1000
        MOV R1, #1
        MOV R2, #2
        MOV R3, #3
        STMIA R0!, {R1-R3}
        LDMDB R0!, {R4-R6}
        MOV SP, #0x2000
        STMFD SP!, {R1, R2}
        LDMFD SP!, {R7, R8}
        SWI 2
",
    );
    assert_eq!(cpu.regs[0], 0x1000);
    assert_eq!(&cpu.regs[4..9], &[1, 2, 3, 1, 2]);
    assert_eq!(cpu.regs[13], 0x2000);
    assert_eq!(cpu.mem.read_u32(0x1FF8), 1);
}

/// ARM7TDMI datasheet 4.11.6 (KoMoDo's jimulator agrees): with write-back,
/// the base is stored unchanged if it is first in the list, else updated.
#[test]
fn stm_writeback_with_base_in_list() {
    let cpu = run(
        "
        MOV R0, #0x1000
        MOV R1, #0x2000
        STMIA R0!, {R0, R2}
        STMIA R1!, {R0, R1}
        SWI 2
",
    );
    assert_eq!(cpu.mem.read_u32(0x1000), 0x1000);
    assert_eq!(cpu.regs[0], 0x1008);
    assert_eq!(cpu.mem.read_u32(0x2000), 0x1008);
    assert_eq!(cpu.mem.read_u32(0x2004), 0x2008);
    assert_eq!(cpu.regs[1], 0x2008);
}

/// Reset CPSR matches the ARM7TDMI and KoMoDo: SVC mode with IRQ/FIQ masked.
#[test]
fn reset_cpsr() {
    let cpu = run("  MRS R0, CPSR\n  SWI 2\n");
    assert_eq!(cpu.regs[0], 0xD3);
}

#[test]
fn halfword_and_signed_loads() {
    let cpu = run(
        "
        ADR R0, data
        LDRH R1, [R0]
        LDRSH R2, [R0]
        LDRSB R3, [R0, #2]
        LDRB R4, [R0, #2]
        MOV R5, #0x1200
        ADD R5, R5, #0x34
        STRH R5, [R0, #4]
        LDR R6, [R0, #4]
        SWI 2
data    DEFH 0xFFFE
        DEFB 0x80, 0
        DEFW 0xAAAA0000
",
    );
    assert_eq!(cpu.regs[1], 0xFFFE);
    assert_eq!(cpu.regs[2], 0xFFFF_FFFE);
    assert_eq!(cpu.regs[3], 0xFFFF_FF80);
    assert_eq!(cpu.regs[4], 0x80);
    assert_eq!(cpu.regs[6], 0xAAAA_1234);
}

#[test]
fn undefined_symbol_and_bad_register() {
    let err = assemble("  MOV R16, #1\n").unwrap_err();
    assert!(err[0].msg.contains("register"), "{:?}", err);
}

#[test]
fn long_multiply() {
    let cpu = run("  MOV R2, #-2\n  MOV R3, #3\n  SMULL R0, R1, R2, R3\n  UMULL R4, R5, R2, R3\n  SWI 2\n");
    assert_eq!((cpu.regs[0], cpu.regs[1]), (0xFFFF_FFFA, 0xFFFF_FFFF));
    assert_eq!((cpu.regs[4], cpu.regs[5]), (0xFFFF_FFFA, 2));
}

#[test]
fn example_programs() {
    assert_eq!(run(include_str!("../examples/hello.s")).output, "Hello, world!\n");
    assert_eq!(
        run_with_input(include_str!("../examples/echo.s"), "Hi there.\n").output,
        "HI THERE\nBye!\n"
    );
    let f = run(include_str!("../examples/factorial.s")).output;
    assert!(f.starts_with("1! = 1\n2! = 2\n3! = 6\n"), "{f}");
    assert!(f.ends_with("10! = 3628800\n"), "{f}");
    assert_eq!(run(include_str!("../examples/bubble_sort.s")).output, "-7 0 3 5 19 42 64 100 \n");
}
