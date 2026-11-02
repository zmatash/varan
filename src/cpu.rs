//! ARMv4 (ARM7TDMI, ARM state) CPU emulator with sparse 32-bit memory.

use std::collections::{HashMap, VecDeque};

const PAGE_BITS: u32 = 12;
const PAGE_SIZE: usize = 1 << PAGE_BITS;

/// Sparse little-endian memory covering the whole 32-bit address space.
/// Unwritten memory reads as zero.
#[derive(Clone, Default)]
pub struct Memory {
    pages: HashMap<u32, Box<[u8; PAGE_SIZE]>>,
}

impl Memory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.pages.clear();
    }

    pub fn read_u8(&self, addr: u32) -> u8 {
        match self.pages.get(&(addr >> PAGE_BITS)) {
            Some(p) => p[(addr as usize) & (PAGE_SIZE - 1)],
            None => 0,
        }
    }

    pub fn write_u8(&mut self, addr: u32, val: u8) {
        let page = self
            .pages
            .entry(addr >> PAGE_BITS)
            .or_insert_with(|| Box::new([0; PAGE_SIZE]));
        page[(addr as usize) & (PAGE_SIZE - 1)] = val;
    }

    pub fn read_u16(&self, addr: u32) -> u16 {
        u16::from_le_bytes([self.read_u8(addr), self.read_u8(addr.wrapping_add(1))])
    }

    pub fn write_u16(&mut self, addr: u32, val: u16) {
        let b = val.to_le_bytes();
        self.write_u8(addr, b[0]);
        self.write_u8(addr.wrapping_add(1), b[1]);
    }

    pub fn read_u32(&self, addr: u32) -> u32 {
        u32::from_le_bytes([
            self.read_u8(addr),
            self.read_u8(addr.wrapping_add(1)),
            self.read_u8(addr.wrapping_add(2)),
            self.read_u8(addr.wrapping_add(3)),
        ])
    }

    pub fn write_u32(&mut self, addr: u32, val: u32) {
        for (i, b) in val.to_le_bytes().iter().enumerate() {
            self.write_u8(addr.wrapping_add(i as u32), *b);
        }
    }

    pub fn load(&mut self, addr: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.write_u8(addr.wrapping_add(i as u32), *b);
        }
    }
}

/// What happened when the CPU tried to execute one instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    /// Instruction executed normally.
    Ok,
    /// Program executed SWI 2 (halt).
    Halted,
    /// Program executed SWI 1 but no input is available. PC is unchanged so
    /// the SWI re-executes once input arrives.
    WaitingForInput,
    /// Something went wrong (undefined instruction, misaligned PC, ...).
    Error(String),
}

pub const MODE_USR: u32 = 0x10;
pub const MODE_SVC: u32 = 0x13;

#[derive(Clone)]
pub struct Cpu {
    /// R0-R15. R15 holds the address of the instruction about to execute.
    pub regs: [u32; 16],
    pub n: bool,
    pub z: bool,
    pub c: bool,
    pub v: bool,
    /// Low (control) bits of CPSR: I, F, T and mode.
    pub cpsr_ctrl: u32,
    pub mem: Memory,
    /// Console output produced by SWIs.
    pub output: String,
    /// Pending keyboard input for SWI 1.
    pub input: VecDeque<u8>,
    /// Number of instructions executed since reset.
    pub steps: u64,
    /// Addresses written during the most recent step (for highlighting).
    pub last_writes: Vec<(u32, u8)>,
    pc_written: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            regs: [0; 16],
            n: false,
            z: false,
            c: false,
            v: false,
            // Reset state as on ARM7TDMI and KoMoDo: SVC mode, IRQ and FIQ disabled.
            cpsr_ctrl: 0xC0 | MODE_SVC,
            mem: Memory::new(),
            output: String::new(),
            input: VecDeque::new(),
            steps: 0,
            last_writes: Vec::new(),
            pc_written: false,
        }
    }

    pub fn pc(&self) -> u32 {
        self.regs[15]
    }

    pub fn cpsr(&self) -> u32 {
        (self.n as u32) << 31
            | (self.z as u32) << 30
            | (self.c as u32) << 29
            | (self.v as u32) << 28
            | (self.cpsr_ctrl & 0xFF)
    }

    pub fn set_cpsr_flags(&mut self, val: u32) {
        self.n = val & (1 << 31) != 0;
        self.z = val & (1 << 30) != 0;
        self.c = val & (1 << 29) != 0;
        self.v = val & (1 << 28) != 0;
    }

    /// Read a register as an operand: R15 reads as the instruction address + 8.
    fn reg(&self, r: u32) -> u32 {
        if r == 15 {
            self.regs[15].wrapping_add(8)
        } else {
            self.regs[r as usize]
        }
    }

    fn set_reg(&mut self, r: u32, val: u32) {
        if r == 15 {
            self.regs[15] = val & !3;
            self.pc_written = true;
        } else {
            self.regs[r as usize] = val;
        }
    }

    fn write_mem_u8(&mut self, addr: u32, val: u8) {
        self.mem.write_u8(addr, val);
        self.last_writes.push((addr, val));
    }

    fn write_mem_u16(&mut self, addr: u32, val: u16) {
        let b = val.to_le_bytes();
        self.write_mem_u8(addr, b[0]);
        self.write_mem_u8(addr.wrapping_add(1), b[1]);
    }

    fn write_mem_u32(&mut self, addr: u32, val: u32) {
        for (i, b) in val.to_le_bytes().iter().enumerate() {
            self.write_mem_u8(addr.wrapping_add(i as u32), *b);
        }
    }

    pub fn cond_passed(&self, cond: u32) -> bool {
        match cond {
            0x0 => self.z,
            0x1 => !self.z,
            0x2 => self.c,
            0x3 => !self.c,
            0x4 => self.n,
            0x5 => !self.n,
            0x6 => self.v,
            0x7 => !self.v,
            0x8 => self.c && !self.z,
            0x9 => !self.c || self.z,
            0xA => self.n == self.v,
            0xB => self.n != self.v,
            0xC => !self.z && self.n == self.v,
            0xD => self.z || self.n != self.v,
            0xE => true,
            _ => false, // NV: never
        }
    }

    /// Execute a single instruction.
    pub fn step(&mut self) -> StepResult {
        self.last_writes.clear();
        let pc = self.regs[15];
        if pc & 3 != 0 {
            return StepResult::Error(format!("PC is not word aligned: 0x{pc:08X}"));
        }
        let instr = self.mem.read_u32(pc);
        self.pc_written = false;
        let cond = instr >> 28;
        let mut result = StepResult::Ok;
        if self.cond_passed(cond) {
            result = self.execute(instr);
        }
        match result {
            StepResult::Ok | StepResult::Halted => {
                if !self.pc_written {
                    self.regs[15] = pc.wrapping_add(4);
                }
                self.steps += 1;
            }
            StepResult::WaitingForInput | StepResult::Error(_) => {
                self.regs[15] = pc;
            }
        }
        result
    }

    fn execute(&mut self, instr: u32) -> StepResult {
        match (instr >> 25) & 7 {
            0b000 => {
                if instr & 0x0FFF_FFF0 == 0x012F_FF10 {
                    // BX
                    let rm = instr & 0xF;
                    let target = self.reg(rm);
                    if target & 1 != 0 {
                        return StepResult::Error(
                            "BX to Thumb state is not supported".to_string(),
                        );
                    }
                    self.set_reg(15, target);
                    StepResult::Ok
                } else if instr & 0xF0 == 0x90 {
                    match (instr >> 23) & 3 {
                        0b00 => self.exec_mul(instr),
                        0b01 => self.exec_mul_long(instr),
                        0b10 if (instr >> 20) & 3 == 0 => self.exec_swp(instr),
                        _ => self.undefined(instr),
                    }
                } else if instr & 0x90 == 0x90 {
                    self.exec_halfword(instr)
                } else if (instr >> 23) & 3 == 0b10 && instr & (1 << 20) == 0 {
                    self.exec_psr(instr)
                } else {
                    self.exec_dp(instr)
                }
            }
            0b001 => {
                if (instr >> 23) & 3 == 0b10 && instr & (1 << 20) == 0 {
                    self.exec_psr(instr)
                } else {
                    self.exec_dp(instr)
                }
            }
            0b010 => self.exec_single(instr),
            0b011 => {
                if instr & 0x10 != 0 {
                    self.undefined(instr)
                } else {
                    self.exec_single(instr)
                }
            }
            0b100 => self.exec_block(instr),
            0b101 => {
                let mut offset = (instr & 0x00FF_FFFF) << 2;
                if offset & 0x0200_0000 != 0 {
                    offset |= 0xFC00_0000;
                }
                if instr & (1 << 24) != 0 {
                    self.regs[14] = self.regs[15].wrapping_add(4);
                }
                let target = self.reg(15).wrapping_add(offset);
                self.set_reg(15, target);
                StepResult::Ok
            }
            0b111 if instr & (1 << 24) != 0 => self.exec_swi(instr & 0x00FF_FFFF),
            _ => self.undefined(instr),
        }
    }

    fn undefined(&self, instr: u32) -> StepResult {
        StepResult::Error(format!(
            "Undefined or unsupported instruction 0x{instr:08X} at 0x{:08X}",
            self.regs[15]
        ))
    }

    /// Barrel shifter. Returns (value, carry out).
    fn shift(&self, typ: u32, val: u32, amount: u32, by_register: bool) -> (u32, bool) {
        let c = self.c;
        if by_register {
            if amount == 0 {
                return (val, c);
            }
            match typ {
                0 => match amount {
                    1..=31 => (val << amount, (val >> (32 - amount)) & 1 != 0),
                    32 => (0, val & 1 != 0),
                    _ => (0, false),
                },
                1 => match amount {
                    1..=31 => (val >> amount, (val >> (amount - 1)) & 1 != 0),
                    32 => (0, val >> 31 != 0),
                    _ => (0, false),
                },
                2 => {
                    if amount >= 32 {
                        let s = val >> 31 != 0;
                        (if s { 0xFFFF_FFFF } else { 0 }, s)
                    } else {
                        (
                            ((val as i32) >> amount) as u32,
                            (val >> (amount - 1)) & 1 != 0,
                        )
                    }
                }
                _ => {
                    let a = amount & 31;
                    if a == 0 {
                        (val, val >> 31 != 0)
                    } else {
                        (val.rotate_right(a), (val >> (a - 1)) & 1 != 0)
                    }
                }
            }
        } else {
            match typ {
                0 => {
                    if amount == 0 {
                        (val, c)
                    } else {
                        (val << amount, (val >> (32 - amount)) & 1 != 0)
                    }
                }
                1 => {
                    if amount == 0 {
                        (0, val >> 31 != 0)
                    } else {
                        (val >> amount, (val >> (amount - 1)) & 1 != 0)
                    }
                }
                2 => {
                    if amount == 0 {
                        let s = val >> 31 != 0;
                        (if s { 0xFFFF_FFFF } else { 0 }, s)
                    } else {
                        (
                            ((val as i32) >> amount) as u32,
                            (val >> (amount - 1)) & 1 != 0,
                        )
                    }
                }
                _ => {
                    if amount == 0 {
                        // RRX
                        (((c as u32) << 31) | (val >> 1), val & 1 != 0)
                    } else {
                        (val.rotate_right(amount), (val >> (amount - 1)) & 1 != 0)
                    }
                }
            }
        }
    }

    /// Decode a data-processing operand 2. Returns (value, shifter carry).
    fn operand2(&self, instr: u32) -> (u32, bool) {
        if instr & (1 << 25) != 0 {
            let imm = instr & 0xFF;
            let rot = ((instr >> 8) & 0xF) * 2;
            let val = imm.rotate_right(rot);
            let carry = if rot == 0 { self.c } else { val >> 31 != 0 };
            (val, carry)
        } else {
            let rm = instr & 0xF;
            let typ = (instr >> 5) & 3;
            if instr & 0x10 != 0 {
                // Register-specified shift: PC reads as +12.
                let rs = (instr >> 8) & 0xF;
                let amount = self.reg(rs) & 0xFF;
                let val = if rm == 15 {
                    self.regs[15].wrapping_add(12)
                } else {
                    self.regs[rm as usize]
                };
                self.shift(typ, val, amount, true)
            } else {
                let amount = (instr >> 7) & 0x1F;
                self.shift(typ, self.reg(rm), amount, false)
            }
        }
    }

    fn exec_dp(&mut self, instr: u32) -> StepResult {
        let opcode = (instr >> 21) & 0xF;
        let s = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;
        let (op2, shift_c) = self.operand2(instr);
        let a = if rn == 15 && instr & (1 << 25) == 0 && instr & 0x10 != 0 {
            self.regs[15].wrapping_add(12)
        } else {
            self.reg(rn)
        };

        let (res, c, v, write) = match opcode {
            0x0 => (a & op2, shift_c, self.v, true),
            0x1 => (a ^ op2, shift_c, self.v, true),
            0x2 => {
                let (r, c, v) = add_with_carry(a, !op2, true);
                (r, c, v, true)
            }
            0x3 => {
                let (r, c, v) = add_with_carry(op2, !a, true);
                (r, c, v, true)
            }
            0x4 => {
                let (r, c, v) = add_with_carry(a, op2, false);
                (r, c, v, true)
            }
            0x5 => {
                let (r, c, v) = add_with_carry(a, op2, self.c);
                (r, c, v, true)
            }
            0x6 => {
                let (r, c, v) = add_with_carry(a, !op2, self.c);
                (r, c, v, true)
            }
            0x7 => {
                let (r, c, v) = add_with_carry(op2, !a, self.c);
                (r, c, v, true)
            }
            0x8 => (a & op2, shift_c, self.v, false),
            0x9 => (a ^ op2, shift_c, self.v, false),
            0xA => {
                let (r, c, v) = add_with_carry(a, !op2, true);
                (r, c, v, false)
            }
            0xB => {
                let (r, c, v) = add_with_carry(a, op2, false);
                (r, c, v, false)
            }
            0xC => (a | op2, shift_c, self.v, true),
            0xD => (op2, shift_c, self.v, true),
            0xE => (a & !op2, shift_c, self.v, true),
            _ => (!op2, shift_c, self.v, true),
        };

        if s && !(write && rd == 15) {
            self.n = res >> 31 != 0;
            self.z = res == 0;
            self.c = c;
            self.v = v;
        }
        if write {
            self.set_reg(rd, res);
        }
        StepResult::Ok
    }

    fn exec_psr(&mut self, instr: u32) -> StepResult {
        let spsr = instr & (1 << 22) != 0;
        if instr & (1 << 21) == 0 {
            // MRS
            let rd = (instr >> 12) & 0xF;
            let val = if spsr { 0 } else { self.cpsr() };
            self.set_reg(rd, val);
        } else {
            // MSR
            if spsr {
                return StepResult::Ok; // No SPSR in this model; ignore.
            }
            let val = if instr & (1 << 25) != 0 {
                (instr & 0xFF).rotate_right(((instr >> 8) & 0xF) * 2)
            } else {
                self.reg(instr & 0xF)
            };
            if instr & (1 << 19) != 0 {
                self.set_cpsr_flags(val);
            }
            if instr & (1 << 16) != 0 {
                let mode = val & 0x1F;
                if matches!(mode, 0x10 | 0x11 | 0x12 | 0x13 | 0x17 | 0x1B | 0x1F) {
                    self.cpsr_ctrl = (val & 0xC0) | mode;
                }
            }
        }
        StepResult::Ok
    }

    fn exec_mul(&mut self, instr: u32) -> StepResult {
        let rd = (instr >> 16) & 0xF;
        let rn = (instr >> 12) & 0xF;
        let rs = (instr >> 8) & 0xF;
        let rm = instr & 0xF;
        let mut res = self.reg(rm).wrapping_mul(self.reg(rs));
        if instr & (1 << 21) != 0 {
            res = res.wrapping_add(self.reg(rn));
        }
        if instr & (1 << 20) != 0 {
            self.n = res >> 31 != 0;
            self.z = res == 0;
        }
        self.set_reg(rd, res);
        StepResult::Ok
    }

    fn exec_mul_long(&mut self, instr: u32) -> StepResult {
        let signed = instr & (1 << 22) != 0;
        let acc = instr & (1 << 21) != 0;
        let s = instr & (1 << 20) != 0;
        let rd_hi = (instr >> 16) & 0xF;
        let rd_lo = (instr >> 12) & 0xF;
        let rs = (instr >> 8) & 0xF;
        let rm = instr & 0xF;
        let mut res: u64 = if signed {
            ((self.reg(rm) as i32 as i64) * (self.reg(rs) as i32 as i64)) as u64
        } else {
            (self.reg(rm) as u64) * (self.reg(rs) as u64)
        };
        if acc {
            let prev = ((self.regs[rd_hi as usize] as u64) << 32) | self.regs[rd_lo as usize] as u64;
            res = res.wrapping_add(prev);
        }
        if s {
            self.n = res >> 63 != 0;
            self.z = res == 0;
        }
        self.set_reg(rd_lo, res as u32);
        self.set_reg(rd_hi, (res >> 32) as u32);
        StepResult::Ok
    }

    fn exec_swp(&mut self, instr: u32) -> StepResult {
        let byte = instr & (1 << 22) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;
        let rm = instr & 0xF;
        let addr = self.reg(rn);
        let src = self.reg(rm);
        if byte {
            let old = self.mem.read_u8(addr) as u32;
            self.write_mem_u8(addr, src as u8);
            self.set_reg(rd, old);
        } else {
            let old = self.read_word_rotated(addr);
            self.write_mem_u32(addr & !3, src);
            self.set_reg(rd, old);
        }
        StepResult::Ok
    }

    /// ARM7 unaligned word load: the aligned word is rotated so the addressed
    /// byte ends up in the least significant byte.
    fn read_word_rotated(&self, addr: u32) -> u32 {
        let w = self.mem.read_u32(addr & !3);
        w.rotate_right((addr & 3) * 8)
    }

    fn exec_single(&mut self, instr: u32) -> StepResult {
        let reg_offset = instr & (1 << 25) != 0;
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let byte = instr & (1 << 22) != 0;
        let wb = instr & (1 << 21) != 0;
        let load = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;

        let offset = if reg_offset {
            let rm = instr & 0xF;
            let typ = (instr >> 5) & 3;
            let amount = (instr >> 7) & 0x1F;
            self.shift(typ, self.reg(rm), amount, false).0
        } else {
            instr & 0xFFF
        };
        let base = self.reg(rn);
        let offset_addr = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let addr = if pre { offset_addr } else { base };

        if load {
            let val = if byte {
                self.mem.read_u8(addr) as u32
            } else {
                self.read_word_rotated(addr)
            };
            if !pre || wb {
                self.set_reg(rn, offset_addr);
            }
            self.set_reg(rd, val);
        } else {
            let val = if rd == 15 {
                self.regs[15].wrapping_add(12)
            } else {
                self.regs[rd as usize]
            };
            if byte {
                self.write_mem_u8(addr, val as u8);
            } else {
                self.write_mem_u32(addr & !3, val);
            }
            if !pre || wb {
                self.set_reg(rn, offset_addr);
            }
        }
        StepResult::Ok
    }

    fn exec_halfword(&mut self, instr: u32) -> StepResult {
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let imm = instr & (1 << 22) != 0;
        let wb = instr & (1 << 21) != 0;
        let load = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let rd = (instr >> 12) & 0xF;
        let sh = (instr >> 5) & 3;

        let offset = if imm {
            ((instr >> 4) & 0xF0) | (instr & 0xF)
        } else {
            self.reg(instr & 0xF)
        };
        let base = self.reg(rn);
        let offset_addr = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let addr = if pre { offset_addr } else { base };

        if load {
            let val = match sh {
                1 => self.mem.read_u16(addr & !1) as u32,
                2 => self.mem.read_u8(addr) as i8 as i32 as u32,
                3 => self.mem.read_u16(addr & !1) as i16 as i32 as u32,
                _ => return self.undefined(instr),
            };
            if !pre || wb {
                self.set_reg(rn, offset_addr);
            }
            self.set_reg(rd, val);
        } else {
            if sh != 1 {
                return self.undefined(instr);
            }
            let val = if rd == 15 {
                self.regs[15].wrapping_add(12)
            } else {
                self.regs[rd as usize]
            };
            self.write_mem_u16(addr & !1, val as u16);
            if !pre || wb {
                self.set_reg(rn, offset_addr);
            }
        }
        StepResult::Ok
    }

    fn exec_block(&mut self, instr: u32) -> StepResult {
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let wb = instr & (1 << 21) != 0;
        let load = instr & (1 << 20) != 0;
        let rn = (instr >> 16) & 0xF;
        let list = instr & 0xFFFF;
        if list == 0 {
            return StepResult::Error(format!(
                "LDM/STM with an empty register list at 0x{:08X}",
                self.regs[15]
            ));
        }
        let count = list.count_ones();
        let base = self.reg(rn);
        let (mut addr, new_base) = if up {
            (if pre { base.wrapping_add(4) } else { base }, base.wrapping_add(4 * count))
        } else {
            let start = base.wrapping_sub(4 * count);
            (if pre { start } else { start.wrapping_add(4) }, start)
        };

        if load {
            if wb {
                self.set_reg(rn, new_base);
            }
            for r in 0..16 {
                if list & (1 << r) != 0 {
                    let v = self.mem.read_u32(addr & !3);
                    self.set_reg(r, v);
                    addr = addr.wrapping_add(4);
                }
            }
        } else {
            // ARM7TDMI (datasheet 4.11.6, and KoMoDo): with write-back, a base
            // register that is first in the list is stored unchanged; later in
            // the list it is stored already updated.
            let first = list.trailing_zeros();
            for r in 0..16 {
                if list & (1 << r) != 0 {
                    let v = if r == 15 {
                        self.regs[15].wrapping_add(12)
                    } else if wb && r == rn && r != first {
                        new_base
                    } else {
                        self.regs[r as usize]
                    };
                    self.write_mem_u32(addr & !3, v);
                    addr = addr.wrapping_add(4);
                }
            }
            if wb {
                self.set_reg(rn, new_base);
            }
        }
        StepResult::Ok
    }

    /// KoMoDo-compatible software interrupts.
    fn exec_swi(&mut self, num: u32) -> StepResult {
        match num {
            0 => {
                self.output.push(self.regs[0] as u8 as char);
                StepResult::Ok
            }
            1 => match self.input.pop_front() {
                Some(b) => {
                    self.regs[0] = b as u32;
                    StepResult::Ok
                }
                None => StepResult::WaitingForInput,
            },
            2 => StepResult::Halted,
            3 => {
                let mut addr = self.regs[0];
                // Guard against runaway strings.
                for _ in 0..1_000_000 {
                    let b = self.mem.read_u8(addr);
                    if b == 0 {
                        break;
                    }
                    self.output.push(b as char);
                    addr = addr.wrapping_add(1);
                }
                StepResult::Ok
            }
            4 => {
                self.output.push_str(&(self.regs[0] as i32).to_string());
                StepResult::Ok
            }
            _ => StepResult::Error(format!(
                "Unsupported SWI {num} (0x{num:X}) at 0x{:08X}. Supported: SWI 0-4.",
                self.regs[15]
            )),
        }
    }
}

/// Returns (result, carry, overflow) for a + b + carry_in.
fn add_with_carry(a: u32, b: u32, carry_in: bool) -> (u32, bool, bool) {
    let sum = a as u64 + b as u64 + carry_in as u64;
    let res = sum as u32;
    let carry = sum >> 32 != 0;
    let overflow = ((a ^ res) & (b ^ res)) >> 31 != 0;
    (res, carry, overflow)
}
