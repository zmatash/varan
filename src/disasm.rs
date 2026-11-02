//! ARM (ARMv4) disassembler producing KoMoDo-style (pre-UAL) syntax.

use crate::asm::COND_NAMES;

const DP_NAMES: [&str; 16] = [
    "AND", "EOR", "SUB", "RSB", "ADD", "ADC", "SBC", "RSC", "TST", "TEQ", "CMP", "CMN", "ORR",
    "MOV", "BIC", "MVN",
];
const SHIFT_NAMES: [&str; 4] = ["LSL", "LSR", "ASR", "ROR"];

pub fn reg_name(r: u32) -> &'static str {
    [
        "R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9", "R10", "R11", "R12", "SP",
        "LR", "PC",
    ][(r & 15) as usize]
}

fn imm_str(v: u32) -> String {
    if v < 10 { format!("#{v}") } else { format!("#0x{v:X}") }
}

fn shifted_reg(instr: u32) -> String {
    let rm = reg_name(instr & 0xF);
    let typ = ((instr >> 5) & 3) as usize;
    if instr & 0x10 != 0 {
        let rs = reg_name((instr >> 8) & 0xF);
        format!("{rm}, {} {rs}", SHIFT_NAMES[typ])
    } else {
        let amt = (instr >> 7) & 0x1F;
        match (typ, amt) {
            (0, 0) => rm.to_string(),
            (3, 0) => format!("{rm}, RRX"),
            (1 | 2, 0) => format!("{rm}, {} #32", SHIFT_NAMES[typ]),
            _ => format!("{rm}, {} #{amt}", SHIFT_NAMES[typ]),
        }
    }
}

fn reglist(list: u32) -> String {
    let mut parts = Vec::new();
    let mut r = 0;
    while r < 16 {
        if list & (1 << r) != 0 {
            let start = r;
            while r + 1 < 16 && list & (1 << (r + 1)) != 0 {
                r += 1;
            }
            if r - start >= 2 {
                parts.push(format!("{}-{}", reg_name(start), reg_name(r)));
            } else {
                for x in start..=r {
                    parts.push(reg_name(x).to_string());
                }
            }
        }
        r += 1;
    }
    format!("{{{}}}", parts.join(", "))
}

/// Disassemble one instruction at `addr`. `label` maps an address to a
/// symbol name, if any (used for branch targets).
pub fn disassemble(instr: u32, addr: u32, label: &dyn Fn(u32) -> Option<String>) -> String {
    let cond = COND_NAMES[(instr >> 28) as usize];
    let target_str = |t: u32| match label(t) {
        Some(l) => l,
        None => format!("0x{t:08X}"),
    };

    match (instr >> 25) & 7 {
        0b000 | 0b001 => {
            let imm = instr & (1 << 25) != 0;
            if !imm && instr & 0x0FFF_FFF0 == 0x012F_FF10 {
                return format!("BX{cond} {}", reg_name(instr & 0xF));
            }
            if !imm && instr & 0xF0 == 0x90 {
                let rd = reg_name((instr >> 16) & 0xF);
                let rn = reg_name((instr >> 12) & 0xF);
                let rs = reg_name((instr >> 8) & 0xF);
                let rm = reg_name(instr & 0xF);
                let s = if instr & (1 << 20) != 0 { "S" } else { "" };
                return match (instr >> 23) & 3 {
                    0b00 => {
                        if instr & (1 << 21) != 0 {
                            format!("MLA{cond}{s} {rd}, {rm}, {rs}, {rn}")
                        } else {
                            format!("MUL{cond}{s} {rd}, {rm}, {rs}")
                        }
                    }
                    0b01 => {
                        let name = match (instr >> 21) & 3 {
                            0 => "UMULL",
                            1 => "UMLAL",
                            2 => "SMULL",
                            _ => "SMLAL",
                        };
                        format!("{name}{cond}{s} {rn}, {rd}, {rm}, {rs}")
                    }
                    0b10 => {
                        let b = if instr & (1 << 22) != 0 { "B" } else { "" };
                        format!("SWP{cond}{b} {rn}, {rm}, [{rd}]")
                    }
                    _ => format!("DCD 0x{instr:08X}"),
                };
            }
            if !imm && instr & 0x90 == 0x90 {
                let load = instr & (1 << 20) != 0;
                let name = match ((instr >> 5) & 3, load) {
                    (1, false) => "STR",
                    (1, true) => "LDR",
                    (2, true) => "LDR",
                    (3, true) => "LDR",
                    _ => return format!("DCD 0x{instr:08X}"),
                };
                let suffix = match (instr >> 5) & 3 {
                    1 => "H",
                    2 => "SB",
                    _ => "SH",
                };
                let rn = reg_name((instr >> 16) & 0xF);
                let rd = reg_name((instr >> 12) & 0xF);
                let up = instr & (1 << 23) != 0;
                let sign = if up { "" } else { "-" };
                let off = if instr & (1 << 22) != 0 {
                    let v = ((instr >> 4) & 0xF0) | (instr & 0xF);
                    if v == 0 { None } else { Some(format!("#{sign}{v}")) }
                } else {
                    Some(format!("{sign}{}", reg_name(instr & 0xF)))
                };
                let pre = instr & (1 << 24) != 0;
                let wb = instr & (1 << 21) != 0;
                let addr_s = match (pre, off) {
                    (true, None) => format!("[{rn}]"),
                    (true, Some(o)) => format!("[{rn}, {o}]{}", if wb { "!" } else { "" }),
                    (false, None) => format!("[{rn}]"),
                    (false, Some(o)) => format!("[{rn}], {o}"),
                };
                return format!("{name}{cond}{suffix} {rd}, {addr_s}");
            }
            let opcode = (instr >> 21) & 0xF;
            let s_bit = instr & (1 << 20) != 0;
            if (instr >> 23) & 3 == 0b10 && !s_bit {
                let psr = if instr & (1 << 22) != 0 { "SPSR" } else { "CPSR" };
                if instr & (1 << 21) == 0 {
                    return format!("MRS{cond} {}, {psr}", reg_name((instr >> 12) & 0xF));
                }
                let mask = (instr >> 16) & 0xF;
                let mut fields = String::new();
                for (bit, ch) in [(8, 'f'), (4, 's'), (2, 'x'), (1, 'c')] {
                    if mask & bit != 0 {
                        fields.push(ch);
                    }
                }
                let src = if imm {
                    imm_str((instr & 0xFF).rotate_right(((instr >> 8) & 0xF) * 2))
                } else {
                    reg_name(instr & 0xF).to_string()
                };
                return format!("MSR{cond} {psr}_{fields}, {src}");
            }
            let name = DP_NAMES[opcode as usize];
            let rn = reg_name((instr >> 16) & 0xF);
            let rd = reg_name((instr >> 12) & 0xF);
            let op2 = if imm {
                imm_str((instr & 0xFF).rotate_right(((instr >> 8) & 0xF) * 2))
            } else {
                shifted_reg(instr)
            };
            if instr == 0xE1A0_0000 {
                return "NOP".to_string();
            }
            match opcode {
                0x8..=0xB => format!("{name}{cond} {rn}, {op2}"),
                0xD | 0xF => {
                    let s = if s_bit { "S" } else { "" };
                    format!("{name}{cond}{s} {rd}, {op2}")
                }
                _ => {
                    let s = if s_bit { "S" } else { "" };
                    format!("{name}{cond}{s} {rd}, {rn}, {op2}")
                }
            }
        }
        0b010 | 0b011 => {
            if instr & (1 << 25) != 0 && instr & 0x10 != 0 {
                return format!("DCD 0x{instr:08X}");
            }
            let load = instr & (1 << 20) != 0;
            let name = if load { "LDR" } else { "STR" };
            let b = if instr & (1 << 22) != 0 { "B" } else { "" };
            let pre = instr & (1 << 24) != 0;
            let wb = instr & (1 << 21) != 0;
            let t = if !pre && wb { "T" } else { "" };
            let rn_n = (instr >> 16) & 0xF;
            let rn = reg_name(rn_n);
            let rd = reg_name((instr >> 12) & 0xF);
            let up = instr & (1 << 23) != 0;
            let sign = if up { "" } else { "-" };
            let off = if instr & (1 << 25) != 0 {
                Some(format!("{sign}{}", shifted_reg(instr)))
            } else {
                let v = instr & 0xFFF;
                if rn_n == 15 && pre && !wb {
                    let t = if up {
                        addr.wrapping_add(8).wrapping_add(v)
                    } else {
                        addr.wrapping_add(8).wrapping_sub(v)
                    };
                    return format!("{name}{cond}{b} {rd}, {}", target_str(t));
                }
                if v == 0 { None } else { Some(format!("#{sign}{v}")) }
            };
            let addr_s = match (pre, off) {
                (_, None) => format!("[{rn}]"),
                (true, Some(o)) => format!("[{rn}, {o}]{}", if wb { "!" } else { "" }),
                (false, Some(o)) => format!("[{rn}], {o}"),
            };
            format!("{name}{cond}{b}{t} {rd}, {addr_s}")
        }
        0b100 => {
            let load = instr & (1 << 20) != 0;
            let pre = instr & (1 << 24) != 0;
            let up = instr & (1 << 23) != 0;
            let rn_n = (instr >> 16) & 0xF;
            let wb = instr & (1 << 21) != 0;
            let list = instr & 0xFFFF;
            let caret = if instr & (1 << 22) != 0 { "^" } else { "" };
            if rn_n == 13 && wb {
                // Stack forms
                if !load && pre && !up {
                    return format!("STMFD{cond} SP!, {}{caret}", reglist(list));
                }
                if load && !pre && up {
                    return format!("LDMFD{cond} SP!, {}{caret}", reglist(list));
                }
            }
            let mode = match (pre, up) {
                (false, true) => "IA",
                (true, true) => "IB",
                (false, false) => "DA",
                (true, false) => "DB",
            };
            let name = if load { "LDM" } else { "STM" };
            let w = if wb { "!" } else { "" };
            format!("{name}{cond}{mode} {}{w}, {}{caret}", reg_name(rn_n), reglist(list))
        }
        0b101 => {
            let mut off = (instr & 0x00FF_FFFF) << 2;
            if off & 0x0200_0000 != 0 {
                off |= 0xFC00_0000;
            }
            let t = addr.wrapping_add(8).wrapping_add(off);
            let l = if instr & (1 << 24) != 0 { "L" } else { "" };
            format!("B{l}{cond} {}", target_str(t))
        }
        0b111 if instr & (1 << 24) != 0 => {
            let n = instr & 0x00FF_FFFF;
            format!("SWI{cond} {n}")
        }
        _ => format!("DCD 0x{instr:08X}"),
    }
}
