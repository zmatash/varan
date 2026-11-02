//! Two-pass assembler for KoMoDo-style ARM assembly.
//!
//! Syntax summary:
//! * `;` starts a comment.
//! * A label is an identifier at the start of a line (optionally followed by
//!   `:`). Labels are case sensitive; mnemonics and registers are not.
//! * Directives: ENTRY, ORIGIN, ALIGN, EQU, DEFW, DEFH, DEFB, DEFS, LTORG
//!   (plus the common aliases DCD, DCW, DCB, SPACE, ORG).
//! * Pseudo-ops: NOP, ADR, ADRL, LDR Rd,=value, PUSH, POP, LSL/LSR/ASR/ROR/RRX,
//!   and automatic MOV<->MVN, CMP<->CMN, ADD<->SUB, AND<->BIC for immediates.

use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub struct AsmError {
    /// 0-based source line.
    pub line: usize,
    pub msg: String,
}

/// One row of the assembler listing.
#[derive(Debug, Clone)]
pub struct ListingRow {
    pub addr: u32,
    pub bytes: Vec<u8>,
    /// 0-based source line (None for literal pool entries).
    pub line: Option<usize>,
    pub is_code: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    /// Contiguous blocks of bytes to load into memory.
    pub segments: Vec<(u32, Vec<u8>)>,
    pub entry: u32,
    /// Label / EQU values.
    pub symbols: BTreeMap<String, u32>,
    /// Instruction address -> source line (0-based).
    pub addr_to_line: BTreeMap<u32, usize>,
    /// Source line -> first instruction address on that line.
    pub line_to_addr: BTreeMap<usize, u32>,
    pub listing: Vec<ListingRow>,
}

// ---------------------------------------------------------------------------
// Mnemonics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Base {
    Dp(u32),
    Mul,
    Mla,
    Umull,
    Umlal,
    Smull,
    Smlal,
    Ldr,
    Str,
    Ldm,
    Stm,
    B,
    Bl,
    Bx,
    Swi,
    Swp,
    Mrs,
    Msr,
    Adr,
    Adrl,
    Nop,
    Push,
    Pop,
    Shift(u32),
    Rrx,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Suffix {
    /// Only a condition.
    Cond,
    /// Condition plus optional S.
    CondS,
    /// Load: B, T, BT, H, SH, SB.
    Load,
    /// Store: B, T, BT, H.
    Store,
    /// LDM/STM addressing mode.
    Block,
    /// SWP: optional B.
    Swp,
    /// No suffix at all.
    None,
}

const MNEMONICS: &[(&str, Base, Suffix)] = &[
    ("AND", Base::Dp(0x0), Suffix::CondS),
    ("EOR", Base::Dp(0x1), Suffix::CondS),
    ("SUB", Base::Dp(0x2), Suffix::CondS),
    ("RSB", Base::Dp(0x3), Suffix::CondS),
    ("ADD", Base::Dp(0x4), Suffix::CondS),
    ("ADC", Base::Dp(0x5), Suffix::CondS),
    ("SBC", Base::Dp(0x6), Suffix::CondS),
    ("RSC", Base::Dp(0x7), Suffix::CondS),
    ("TST", Base::Dp(0x8), Suffix::CondS),
    ("TEQ", Base::Dp(0x9), Suffix::CondS),
    ("CMP", Base::Dp(0xA), Suffix::CondS),
    ("CMN", Base::Dp(0xB), Suffix::CondS),
    ("ORR", Base::Dp(0xC), Suffix::CondS),
    ("MOV", Base::Dp(0xD), Suffix::CondS),
    ("BIC", Base::Dp(0xE), Suffix::CondS),
    ("MVN", Base::Dp(0xF), Suffix::CondS),
    ("MUL", Base::Mul, Suffix::CondS),
    ("MLA", Base::Mla, Suffix::CondS),
    ("UMULL", Base::Umull, Suffix::CondS),
    ("UMLAL", Base::Umlal, Suffix::CondS),
    ("SMULL", Base::Smull, Suffix::CondS),
    ("SMLAL", Base::Smlal, Suffix::CondS),
    ("LDR", Base::Ldr, Suffix::Load),
    ("STR", Base::Str, Suffix::Store),
    ("LDM", Base::Ldm, Suffix::Block),
    ("STM", Base::Stm, Suffix::Block),
    ("B", Base::B, Suffix::Cond),
    ("BL", Base::Bl, Suffix::Cond),
    ("BX", Base::Bx, Suffix::Cond),
    ("SWI", Base::Swi, Suffix::Cond),
    ("SVC", Base::Swi, Suffix::Cond),
    ("SWP", Base::Swp, Suffix::Swp),
    ("MRS", Base::Mrs, Suffix::Cond),
    ("MSR", Base::Msr, Suffix::Cond),
    ("ADR", Base::Adr, Suffix::Cond),
    ("ADRL", Base::Adrl, Suffix::Cond),
    ("NOP", Base::Nop, Suffix::None),
    ("PUSH", Base::Push, Suffix::Cond),
    ("POP", Base::Pop, Suffix::Cond),
    ("LSL", Base::Shift(0), Suffix::CondS),
    ("ASL", Base::Shift(0), Suffix::CondS),
    ("LSR", Base::Shift(1), Suffix::CondS),
    ("ASR", Base::Shift(2), Suffix::CondS),
    ("ROR", Base::Shift(3), Suffix::CondS),
    ("RRX", Base::Rrx, Suffix::CondS),
];

pub const COND_NAMES: [&str; 16] = [
    "EQ", "NE", "CS", "CC", "MI", "PL", "VS", "VC", "HI", "LS", "GE", "LT", "GT", "LE", "", "NV",
];

fn parse_cond(s: &str) -> Option<u32> {
    Some(match s {
        "EQ" => 0,
        "NE" => 1,
        "CS" | "HS" => 2,
        "CC" | "LO" => 3,
        "MI" => 4,
        "PL" => 5,
        "VS" => 6,
        "VC" => 7,
        "HI" => 8,
        "LS" => 9,
        "GE" => 10,
        "LT" => 11,
        "GT" => 12,
        "LE" => 13,
        "AL" | "" => 14,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Mnemonic {
    base: Base,
    cond: u32,
    /// Extra suffix text (S, B, H, SB, FD, ...), already validated.
    extra: &'static str,
}

const LOAD_EXTRAS: &[&str] = &["", "B", "T", "BT", "H", "SH", "SB"];
const STORE_EXTRAS: &[&str] = &["", "B", "T", "BT", "H"];
const BLOCK_EXTRAS: &[&str] = &["", "IA", "IB", "DA", "DB", "FD", "ED", "FA", "EA"];
const S_EXTRAS: &[&str] = &["", "S"];
const SWP_EXTRAS: &[&str] = &["", "B"];
const NO_EXTRAS: &[&str] = &[""];

fn parse_mnemonic(tok: &str) -> Option<Mnemonic> {
    let up = tok.to_ascii_uppercase();
    let mut candidates: Vec<&(&str, Base, Suffix)> =
        MNEMONICS.iter().filter(|(n, _, _)| up.starts_with(n)).collect();
    candidates.sort_by_key(|(n, _, _)| std::cmp::Reverse(n.len()));
    for (name, base, suffix) in candidates {
        let rest = &up[name.len()..];
        if *suffix == Suffix::None {
            if rest.is_empty() {
                return Some(Mnemonic { base: *base, cond: 14, extra: "" });
            }
            continue;
        }
        let extras = match suffix {
            Suffix::Cond => NO_EXTRAS,
            Suffix::CondS => S_EXTRAS,
            Suffix::Load => LOAD_EXTRAS,
            Suffix::Store => STORE_EXTRAS,
            Suffix::Block => BLOCK_EXTRAS,
            Suffix::Swp => SWP_EXTRAS,
            Suffix::None => NO_EXTRAS,
        };
        // Try <cond><extra> then <extra><cond>.
        for split in 0..=rest.len() {
            let (a, b) = rest.split_at(split);
            if let (Some(cond), Some(extra)) = (parse_cond(a), extras.iter().find(|e| **e == b)) {
                return Some(Mnemonic { base: *base, cond, extra });
            }
            if let (Some(extra), Some(cond)) = (extras.iter().find(|e| **e == a), parse_cond(b)) {
                return Some(Mnemonic { base: *base, cond, extra });
            }
        }
    }
    None
}

const DIRECTIVES: &[&str] = &[
    "ENTRY", "ORIGIN", "ORG", "ALIGN", "EQU", "DEFW", "DCD", "DEFH", "DCW", "DEFB", "DCB", "DEFS",
    "SPACE", "LTORG", "END",
];

fn is_directive(tok: &str) -> bool {
    let up = tok.to_ascii_uppercase();
    DIRECTIVES.contains(&up.as_str())
}

/// True if `tok` is an instruction mnemonic or directive (used by the editor
/// for syntax highlighting too).
pub fn is_keyword(tok: &str) -> bool {
    is_directive(tok) || parse_mnemonic(tok).is_some()
}

pub fn parse_reg(s: &str) -> Option<u32> {
    let up = s.trim().to_ascii_uppercase();
    match up.as_str() {
        "SP" => return Some(13),
        "LR" => return Some(14),
        "PC" => return Some(15),
        "FP" => return Some(11),
        "IP" => return Some(12),
        "SL" => return Some(10),
        _ => {}
    }
    let num = up.strip_prefix('R')?;
    if num.is_empty() || num.len() > 2 || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: u32 = num.parse().ok()?;
    if n < 16 { Some(n) } else { None }
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum ExprErr {
    Undefined(String),
    Syntax(String),
}

impl ExprErr {
    fn msg(&self) -> String {
        match self {
            ExprErr::Undefined(s) => format!("undefined symbol '{s}'"),
            ExprErr::Syntax(s) => s.clone(),
        }
    }
}

struct ExprParser<'a> {
    chars: Vec<char>,
    pos: usize,
    syms: &'a dyn Fn(&str) -> Option<i64>,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

impl<'a> ExprParser<'a> {
    fn peek(&mut self) -> Option<char> {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
        self.chars.get(self.pos).copied()
    }

    fn eat(&mut self, s: &str) -> bool {
        self.peek();
        let sc: Vec<char> = s.chars().collect();
        if self.chars[self.pos..].starts_with(&sc) {
            self.pos += sc.len();
            true
        } else {
            false
        }
    }

    fn parse(&mut self) -> Result<i64, ExprErr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<i64, ExprErr> {
        let mut v = self.parse_xor()?;
        loop {
            if self.eat("||") {
                return Err(ExprErr::Syntax("unexpected '||'".into()));
            } else if self.eat("|") {
                v |= self.parse_xor()?;
            } else {
                return Ok(v);
            }
        }
    }

    fn parse_xor(&mut self) -> Result<i64, ExprErr> {
        let mut v = self.parse_and()?;
        while self.eat("^") {
            v ^= self.parse_and()?;
        }
        Ok(v)
    }

    fn parse_and(&mut self) -> Result<i64, ExprErr> {
        let mut v = self.parse_shift()?;
        while self.eat("&") {
            v &= self.parse_shift()?;
        }
        Ok(v)
    }

    fn parse_shift(&mut self) -> Result<i64, ExprErr> {
        let mut v = self.parse_add()?;
        loop {
            if self.eat("<<") {
                let r = self.parse_add()?;
                v = ((v as u32 as u64) << (r & 63)) as i64;
            } else if self.eat(">>") {
                let r = self.parse_add()?;
                v = ((v as u32 as u64) >> (r & 63)) as i64;
            } else {
                return Ok(v);
            }
        }
    }

    fn parse_add(&mut self) -> Result<i64, ExprErr> {
        let mut v = self.parse_mul()?;
        loop {
            if self.eat("+") {
                v = v.wrapping_add(self.parse_mul()?);
            } else if self.eat("-") {
                v = v.wrapping_sub(self.parse_mul()?);
            } else {
                return Ok(v);
            }
        }
    }

    fn parse_mul(&mut self) -> Result<i64, ExprErr> {
        let mut v = self.parse_unary()?;
        loop {
            if self.eat("*") {
                v = v.wrapping_mul(self.parse_unary()?);
            } else if self.eat("/") {
                let r = self.parse_unary()?;
                if r == 0 {
                    return Err(ExprErr::Syntax("division by zero".into()));
                }
                v = v.wrapping_div(r);
            } else if self.eat("%") {
                let r = self.parse_unary()?;
                if r == 0 {
                    return Err(ExprErr::Syntax("division by zero".into()));
                }
                v = v.wrapping_rem(r);
            } else {
                return Ok(v);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<i64, ExprErr> {
        if self.eat("-") {
            Ok(self.parse_unary()?.wrapping_neg())
        } else if self.eat("+") {
            self.parse_unary()
        } else if self.eat("~") {
            Ok(!self.parse_unary()?)
        } else {
            self.parse_atom()
        }
    }

    fn parse_atom(&mut self) -> Result<i64, ExprErr> {
        let c = match self.peek() {
            Some(c) => c,
            None => return Err(ExprErr::Syntax("expected a value".into())),
        };
        if c == '(' {
            self.pos += 1;
            let v = self.parse()?;
            if !self.eat(")") {
                return Err(ExprErr::Syntax("missing ')'".into()));
            }
            return Ok(v);
        }
        if c == '\'' {
            // Character literal.
            let rest: String = self.chars[self.pos + 1..].iter().collect();
            let (ch, used) = parse_char_escape(&rest)
                .ok_or_else(|| ExprErr::Syntax("bad character literal".into()))?;
            self.pos += 1 + used;
            if self.chars.get(self.pos) != Some(&'\'') {
                return Err(ExprErr::Syntax("unterminated character literal".into()));
            }
            self.pos += 1;
            return Ok(ch as i64);
        }
        if c == '&' {
            self.pos += 1;
            return self.parse_digits(16);
        }
        if c.is_ascii_digit() {
            let next = self.chars.get(self.pos + 1).map(|c| c.to_ascii_lowercase());
            if c == '0' && next == Some('x') {
                self.pos += 2;
                return self.parse_digits(16);
            }
            if c == '0' && next == Some('b') {
                // 0b101 binary (but not if it's actually e.g. 0b as hex; we don't support hex suffixes)
                self.pos += 2;
                return self.parse_digits(2);
            }
            return self.parse_digits(10);
        }
        if is_ident_start(c) {
            let start = self.pos;
            while self.pos < self.chars.len() && is_ident_char(self.chars[self.pos]) {
                self.pos += 1;
            }
            let name: String = self.chars[start..self.pos].iter().collect();
            return (self.syms)(&name).ok_or(ExprErr::Undefined(name));
        }
        Err(ExprErr::Syntax(format!("unexpected '{c}'")))
    }

    fn parse_digits(&mut self, radix: u32) -> Result<i64, ExprErr> {
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_alphanumeric() || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }
        let s: String = self.chars[start..self.pos].iter().filter(|c| **c != '_').collect();
        if s.is_empty() {
            return Err(ExprErr::Syntax("expected digits".into()));
        }
        let v = u64::from_str_radix(&s, radix)
            .map_err(|_| ExprErr::Syntax(format!("invalid number '{s}'")))?;
        if v > u32::MAX as u64 {
            return Err(ExprErr::Syntax(format!("number '{s}' does not fit in 32 bits")));
        }
        Ok(v as i64)
    }
}

/// Parse one (possibly escaped) character. Returns the char and number of
/// source chars consumed.
fn parse_char_escape(s: &str) -> Option<(u32, usize)> {
    let mut it = s.chars();
    let c = it.next()?;
    if c != '\\' {
        return Some((c as u32, 1));
    }
    let e = it.next()?;
    let v = match e {
        'n' => 10,
        'r' => 13,
        't' => 9,
        '0' => 0,
        'a' => 7,
        'b' => 8,
        'f' => 12,
        'v' => 11,
        'e' => 27,
        '\\' => '\\' as u32,
        '\'' => '\'' as u32,
        '"' => '"' as u32,
        _ => return None,
    };
    Some((v, 2))
}

fn eval_expr(s: &str, syms: &dyn Fn(&str) -> Option<i64>) -> Result<i64, ExprErr> {
    let mut p = ExprParser { chars: s.chars().collect(), pos: 0, syms };
    let v = p.parse()?;
    if p.peek().is_some() {
        let rest: String = p.chars[p.pos..].iter().collect();
        return Err(ExprErr::Syntax(format!("unexpected '{}'", rest.trim())));
    }
    Ok(v)
}

/// Evaluate an expression (numbers, labels, arithmetic) to a 32-bit value.
/// Used by the UI for address boxes and register editing.
pub fn evaluate(expr: &str, symbols: &BTreeMap<String, u32>) -> Result<u32, String> {
    let syms = |n: &str| symbols.get(n).map(|v| *v as i64);
    let v = eval_expr(expr.trim(), &syms).map_err(|e| e.msg())?;
    if v < i32::MIN as i64 || v > u32::MAX as i64 {
        return Err("value does not fit in 32 bits".into());
    }
    Ok(v as u32)
}

// ---------------------------------------------------------------------------
// Source parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Stmt {
    line: usize,
    label: Option<String>,
    /// Upper-cased op (mnemonic or directive), if any.
    op: Option<String>,
    /// Original op text (for error messages).
    op_raw: String,
    args: String,
}

/// Remove a `;` comment, respecting quotes.
fn strip_comment(line: &str) -> &str {
    let mut in_str: Option<char> = None;
    let mut prev_backslash = false;
    for (i, c) in line.char_indices() {
        match in_str {
            Some(q) => {
                if c == q && !prev_backslash {
                    in_str = None;
                }
                prev_backslash = c == '\\' && !prev_backslash;
            }
            None => {
                if c == ';' {
                    return &line[..i];
                }
                if c == '"' || c == '\'' {
                    in_str = Some(c);
                    prev_backslash = false;
                }
            }
        }
    }
    line
}

fn take_ident(s: &str) -> (&str, &str) {
    let end = s
        .char_indices()
        .find(|(i, c)| if *i == 0 { !is_ident_start(*c) } else { !is_ident_char(*c) })
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    s.split_at(end)
}

fn parse_line(line_no: usize, raw: &str) -> Result<Option<Stmt>, AsmError> {
    let text = strip_comment(raw);
    if text.trim().is_empty() {
        return Ok(None);
    }
    let at_col0 = !text.starts_with(|c: char| c.is_whitespace());
    let mut rest = text.trim_start();
    let mut label = None;

    let (first, after) = take_ident(rest);
    if first.is_empty() {
        return Err(AsmError {
            line: line_no,
            msg: format!("unexpected '{}'", rest.trim()),
        });
    }
    if let Some(after_colon) = after.strip_prefix(':') {
        label = Some(first.to_string());
        rest = after_colon.trim_start();
    } else if at_col0 && !is_keyword(first) {
        label = Some(first.to_string());
        rest = after.trim_start();
    } else {
        // A word followed by another mnemonic/directive is a label, even if
        // it looks like an instruction (e.g. `str DEFB "hi"`) or is indented.
        let after_trim = after.trim_start();
        let (second, after_second) = take_ident(after_trim);
        let second_is_word = !second.is_empty()
            && after.starts_with(|c: char| c.is_whitespace())
            && (after_second.is_empty() || after_second.starts_with(|c: char| c.is_whitespace()));
        if second_is_word && is_keyword(second) && parse_reg(second).is_none() {
            label = Some(first.to_string());
            rest = after.trim_start();
        }
    }

    if rest.is_empty() {
        return Ok(Some(Stmt { line: line_no, label, op: None, op_raw: String::new(), args: String::new() }));
    }
    let (op, args) = take_ident(rest);
    if op.is_empty() {
        return Err(AsmError { line: line_no, msg: format!("unexpected '{}'", rest.trim()) });
    }
    if !is_keyword(op) {
        return Err(AsmError { line: line_no, msg: format!("unknown instruction '{op}'") });
    }
    Ok(Some(Stmt {
        line: line_no,
        label,
        op: Some(op.to_ascii_uppercase()),
        op_raw: op.to_string(),
        args: args.trim().to_string(),
    }))
}

/// Split on commas that are not inside brackets, braces or quotes.
fn split_operands(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str: Option<char> = None;
    let mut prev_backslash = false;
    let mut cur = String::new();
    for c in s.chars() {
        if let Some(q) = in_str {
            cur.push(c);
            if c == q && !prev_backslash {
                in_str = None;
            }
            prev_backslash = c == '\\' && !prev_backslash;
            continue;
        }
        match c {
            '"' | '\'' => {
                in_str = Some(c);
                prev_backslash = false;
                cur.push(c);
            }
            '[' | '{' | '(' => {
                depth += 1;
                cur.push(c);
            }
            ']' | '}' | ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Parse a string literal (with escapes). Returns bytes.
fn parse_string_lit(s: &str) -> Option<Vec<u8>> {
    let q = s.chars().next()?;
    if (q != '"' && q != '\'') || s.len() < 2 || !s.ends_with(q) {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    let mut out = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = inner.chars().collect();
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        let (c, used) = parse_char_escape(&rest)?;
        if c > 0xFF {
            return None;
        }
        out.push(c as u8);
        i += used;
    }
    Some(out)
}

fn is_string_item(s: &str) -> bool {
    (s.starts_with('"') && s.len() >= 2 && s.ends_with('"'))
        || (s.starts_with('\'') && s.len() > 3 && s.ends_with('\'') && parse_string_lit(s).map(|b| b.len() > 1).unwrap_or(false))
}

// ---------------------------------------------------------------------------
// Encoding helpers
// ---------------------------------------------------------------------------

/// Encode a 32-bit value as an ARM rotated 8-bit immediate (rot<<8 | imm8).
pub fn encode_imm(v: u32) -> Option<u32> {
    for rot in 0..16 {
        let r = v.rotate_left(rot * 2);
        if r <= 0xFF {
            return Some((rot << 8) | r);
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
enum Shift {
    None,
    Imm(u32, u32),
    Reg(u32, u32),
    Rrx,
}

fn shift_type(s: &str) -> Option<u32> {
    match s.to_ascii_uppercase().as_str() {
        "LSL" | "ASL" => Some(0),
        "LSR" => Some(1),
        "ASR" => Some(2),
        "ROR" => Some(3),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
enum Op2 {
    Imm(u32),
    Reg(u32, Shift),
}

/// Assembly context for pass 2.
struct Ctx<'a> {
    symbols: &'a HashMap<String, i64>,
    addr: u32,
}

impl Ctx<'_> {
    fn eval(&self, s: &str) -> Result<i64, String> {
        let syms = |n: &str| self.symbols.get(n).copied();
        eval_expr(s, &syms).map_err(|e| e.msg())
    }

    fn eval_u32(&self, s: &str) -> Result<u32, String> {
        let v = self.eval(s)?;
        if v < i32::MIN as i64 || v > u32::MAX as i64 {
            return Err(format!("value {v} does not fit in 32 bits"));
        }
        Ok(v as u32)
    }

    fn reg(&self, s: &str) -> Result<u32, String> {
        parse_reg(s).ok_or_else(|| format!("expected a register, found '{s}'"))
    }

    fn imm(&self, s: &str) -> Result<u32, String> {
        let e = s
            .trim()
            .strip_prefix('#')
            .ok_or_else(|| format!("expected an immediate value (#...), found '{s}'"))?;
        self.eval_u32(e)
    }

    /// Parse "LSL #3", "LSR R2", "RRX".
    fn shift(&self, s: &str, allow_reg: bool) -> Result<Shift, String> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("RRX") {
            return Ok(Shift::Rrx);
        }
        let (name, rest) = s.split_at(s.find(|c: char| c.is_whitespace() || c == '#').unwrap_or(s.len()));
        let typ = shift_type(name).ok_or_else(|| format!("expected a shift (LSL, LSR, ASR, ROR, RRX), found '{s}'"))?;
        let rest = rest.trim();
        if rest.starts_with('#') {
            let n = self.imm(rest)?;
            match typ {
                0 if n <= 31 => Ok(if n == 0 { Shift::None } else { Shift::Imm(0, n) }),
                1 | 2 if (1..=32).contains(&n) => Ok(Shift::Imm(typ, n & 31)),
                1 | 2 if n == 0 => Ok(Shift::None),
                3 if (1..=31).contains(&n) => Ok(Shift::Imm(3, n)),
                3 if n == 0 => Ok(Shift::None),
                _ => Err(format!("shift amount {n} out of range")),
            }
        } else if let Some(r) = parse_reg(rest) {
            if !allow_reg {
                return Err("register-specified shift not allowed here".into());
            }
            Ok(Shift::Reg(typ, r))
        } else {
            Err(format!("bad shift amount '{rest}'"))
        }
    }

    fn op2(&self, parts: &[String]) -> Result<Op2, String> {
        match parts {
            [one] if one.trim().starts_with('#') => Ok(Op2::Imm(self.imm(one)?)),
            [one] => Ok(Op2::Reg(self.reg(one)?, Shift::None)),
            [rm, sh] => Ok(Op2::Reg(self.reg(rm)?, self.shift(sh, true)?)),
            [] => Err("missing operand".into()),
            _ => Err("too many operands".into()),
        }
    }
}

fn encode_shift(rm: u32, sh: Shift) -> u32 {
    match sh {
        Shift::None => rm,
        Shift::Imm(t, n) => (n << 7) | (t << 5) | rm,
        Shift::Reg(t, rs) => (rs << 8) | (t << 5) | 0x10 | rm,
        Shift::Rrx => (3 << 5) | rm,
    }
}

/// Encode a data-processing instruction, applying immediate substitutions
/// (MOV<->MVN, CMP<->CMN, ADD<->SUB, AND<->BIC, ADC<->SBC) when needed.
fn encode_dp(cond: u32, opcode: u32, s: bool, rn: u32, rd: u32, op2: Op2) -> Result<u32, String> {
    let s_bit = if s || (0x8..=0xB).contains(&opcode) { 1 << 20 } else { 0 };
    match op2 {
        Op2::Reg(rm, sh) => {
            Ok((cond << 28) | (opcode << 21) | s_bit | (rn << 16) | (rd << 12) | encode_shift(rm, sh))
        }
        Op2::Imm(v) => {
            let attempts: Vec<(u32, u32)> = {
                let mut a = vec![(opcode, v)];
                match opcode {
                    0xD => a.push((0xF, !v)),
                    0xF => a.push((0xD, !v)),
                    0xA => a.push((0xB, v.wrapping_neg())),
                    0xB => a.push((0xA, v.wrapping_neg())),
                    0x4 => a.push((0x2, v.wrapping_neg())),
                    0x2 => a.push((0x4, v.wrapping_neg())),
                    0x0 => a.push((0xE, !v)),
                    0xE => a.push((0x0, !v)),
                    0x5 => a.push((0x6, !v)),
                    0x6 => a.push((0x5, !v)),
                    _ => {}
                }
                a
            };
            for (op, val) in attempts {
                if let Some(enc) = encode_imm(val) {
                    return Ok((cond << 28)
                        | (1 << 25)
                        | (op << 21)
                        | s_bit
                        | (rn << 16)
                        | (rd << 12)
                        | enc);
                }
            }
            Err(format!(
                "immediate value 0x{v:X} ({}) cannot be encoded; try LDR Rd, ={}",
                v as i32, v as i32
            ))
        }
    }
}

fn parse_reglist(s: &str) -> Result<u32, String> {
    let inner = s
        .trim()
        .strip_prefix('{')
        .and_then(|x| x.strip_suffix('}'))
        .ok_or_else(|| format!("expected a register list {{...}}, found '{s}'"))?;
    let mut mask = 0u32;
    for item in inner.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some((a, b)) = item.split_once('-') {
            let a = parse_reg(a).ok_or_else(|| format!("bad register '{}'", a.trim()))?;
            let b = parse_reg(b).ok_or_else(|| format!("bad register '{}'", b.trim()))?;
            if a > b {
                return Err(format!("bad register range '{item}'"));
            }
            for r in a..=b {
                mask |= 1 << r;
            }
        } else {
            let r = parse_reg(item).ok_or_else(|| format!("bad register '{item}'"))?;
            mask |= 1 << r;
        }
    }
    if mask == 0 {
        return Err("empty register list".into());
    }
    Ok(mask)
}

// ---------------------------------------------------------------------------
// Assembler
// ---------------------------------------------------------------------------

/// Byte size an instruction mnemonic occupies.
fn instr_size(m: &Mnemonic) -> u32 {
    if m.base == Base::Adrl { 8 } else { 4 }
}

struct Pool {
    addr: u32,
    /// (source line, expression) per slot.
    entries: Vec<(usize, String)>,
}

struct Output {
    segments: Vec<(u32, Vec<u8>)>,
}

impl Output {
    fn emit(&mut self, addr: u32, bytes: &[u8]) {
        if let Some((start, data)) = self.segments.last_mut() {
            if start.wrapping_add(data.len() as u32) == addr {
                data.extend_from_slice(bytes);
                return;
            }
        }
        self.segments.push((addr, bytes.to_vec()));
    }
}

fn align_up(addr: u32, n: u32) -> u32 {
    let n = n.max(1);
    addr.wrapping_add(n - 1) / n * n
}

/// Assemble source text into a program, or a list of errors.
pub fn assemble(source: &str) -> Result<Program, Vec<AsmError>> {
    let mut errors = Vec::new();
    let mut stmts = Vec::new();
    for (i, line) in source.lines().enumerate() {
        match parse_line(i, line) {
            Ok(Some(s)) => stmts.push(s),
            Ok(None) => {}
            Err(e) => errors.push(e),
        }
    }

    // ---------------- Pass 1: addresses and symbols ----------------
    let mut symbols: HashMap<String, i64> = HashMap::new();
    let mut label_lines: HashMap<String, usize> = HashMap::new();
    // statement index -> address
    let mut stmt_addr: Vec<u32> = vec![0; stmts.len()];
    // statement index -> (pool index, slot index) for LDR =
    let mut lit_slot: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut pools: Vec<Pool> = Vec::new();
    let mut cur_pool: Vec<(usize, String)> = Vec::new();
    let mut cur_pool_stmts: Vec<(usize, usize)> = Vec::new(); // (stmt idx, slot)
    // statement index -> pool index flushed before/at this statement
    let mut pool_flush_at: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut end_pool_flush: Vec<usize> = Vec::new();
    let mut deferred_equ: Vec<(String, String, usize)> = Vec::new();

    let mut addr: u32 = 0;
    let mut entry: Option<u32> = None;
    let mut entry_pending = false;
    let mut first_code: Option<u32> = None;
    let mut pending_labels: Vec<(String, usize)> = Vec::new();

    let define = |symbols: &mut HashMap<String, i64>,
                  label_lines: &mut HashMap<String, usize>,
                  errors: &mut Vec<AsmError>,
                  name: &str,
                  val: i64,
                  line: usize| {
        if parse_reg(name).is_some() {
            errors.push(AsmError { line, msg: format!("'{name}' is a register name and cannot be a label") });
        } else if let Some(prev) = label_lines.get(name) {
            errors.push(AsmError {
                line,
                msg: format!("'{name}' is already defined on line {}", prev + 1),
            });
        } else {
            symbols.insert(name.to_string(), val);
            label_lines.insert(name.to_string(), line);
        }
    };

    macro_rules! flush_pool {
        ($at:expr) => {
            if !cur_pool.is_empty() {
                addr = align_up(addr, 4);
                let idx = pools.len();
                for (si, slot) in cur_pool_stmts.drain(..) {
                    lit_slot.insert(si, (idx, slot));
                }
                let entries = std::mem::take(&mut cur_pool);
                let n = entries.len() as u32;
                pools.push(Pool { addr, entries });
                addr = addr.wrapping_add(4 * n);
                match $at {
                    Some(si) => pool_flush_at.entry(si).or_default().push(idx),
                    None => end_pool_flush.push(idx),
                }
            }
        };
    }

    let mut ended = false;
    for (si, st) in stmts.iter().enumerate() {
        if ended {
            break;
        }
        let op = st.op.as_deref();
        // Labels on lines with no op wait for the next op (so they get the
        // aligned address of whatever follows).
        if op.is_none() {
            if let Some(l) = &st.label {
                pending_labels.push((l.clone(), st.line));
            }
            continue;
        }
        let op = op.unwrap();

        if op == "EQU" {
            let name = match &st.label {
                Some(n) => n.clone(),
                None => {
                    errors.push(AsmError { line: st.line, msg: "EQU needs a name, e.g. 'size EQU 10'".into() });
                    continue;
                }
            };
            let syms = |n: &str| symbols.get(n).copied();
            match eval_expr(&st.args, &syms) {
                Ok(v) => define(&mut symbols, &mut label_lines, &mut errors, &name, v, st.line),
                Err(ExprErr::Undefined(_)) => deferred_equ.push((name, st.args.clone(), st.line)),
                Err(e) => errors.push(AsmError { line: st.line, msg: e.msg() }),
            }
            continue;
        }

        if op == "ORIGIN" || op == "ORG" {
            flush_pool!(Some(si));
            let syms = |n: &str| symbols.get(n).copied();
            match eval_expr(&st.args, &syms) {
                Ok(v) => addr = v as u32,
                Err(e) => errors.push(AsmError { line: st.line, msg: format!("ORIGIN: {}", e.msg()) }),
            }
            stmt_addr[si] = addr;
            for (l, line) in pending_labels.drain(..) {
                define(&mut symbols, &mut label_lines, &mut errors, &l, addr as i64, line);
            }
            if let Some(l) = &st.label {
                define(&mut symbols, &mut label_lines, &mut errors, l, addr as i64, st.line);
            }
            continue;
        }

        let mnem = if is_directive(op) { None } else { parse_mnemonic(op) };

        // Alignment needed before this statement.
        match op {
            "DEFW" | "DCD" => addr = align_up(addr, 4),
            "DEFH" | "DCW" => addr = align_up(addr, 2),
            "ALIGN" => {
                let n = if st.args.is_empty() {
                    4
                } else {
                    let syms = |n: &str| symbols.get(n).copied();
                    match eval_expr(&st.args, &syms) {
                        Ok(v) if v > 0 && (v as u32).is_power_of_two() => v as u32,
                        _ => {
                            errors.push(AsmError { line: st.line, msg: "ALIGN needs a power of two".into() });
                            4
                        }
                    }
                };
                addr = align_up(addr, n);
            }
            "LTORG" => flush_pool!(Some(si)),
            _ => {
                if mnem.is_some() {
                    addr = align_up(addr, 4);
                }
            }
        }

        stmt_addr[si] = addr;
        for (l, line) in pending_labels.drain(..) {
            define(&mut symbols, &mut label_lines, &mut errors, &l, addr as i64, line);
        }
        if let Some(l) = &st.label {
            define(&mut symbols, &mut label_lines, &mut errors, l, addr as i64, st.line);
        }

        if let Some(m) = mnem {
            if first_code.is_none() {
                first_code = Some(addr);
            }
            if entry_pending {
                entry = Some(addr);
                entry_pending = false;
            }
            if m.base == Base::Ldr {
                let ops = split_operands(&st.args);
                if let Some(lit) = ops.get(1).and_then(|o| o.trim().strip_prefix('=')) {
                    cur_pool_stmts.push((si, cur_pool.len()));
                    cur_pool.push((st.line, lit.trim().to_string()));
                }
            }
            addr = addr.wrapping_add(instr_size(&m));
            continue;
        }

        match op {
            "ENTRY" => {
                if entry.is_some() || entry_pending {
                    errors.push(AsmError { line: st.line, msg: "ENTRY defined more than once".into() });
                } else {
                    entry_pending = true;
                }
            }
            "DEFW" | "DCD" => {
                let n = split_operands(&st.args).len() as u32;
                if n == 0 {
                    errors.push(AsmError { line: st.line, msg: "DEFW needs at least one value".into() });
                }
                addr = addr.wrapping_add(4 * n);
            }
            "DEFH" | "DCW" => {
                let n = split_operands(&st.args).len() as u32;
                addr = addr.wrapping_add(2 * n);
            }
            "DEFB" | "DCB" => {
                let mut n = 0u32;
                for item in split_operands(&st.args) {
                    if is_string_item(&item) {
                        match parse_string_lit(&item) {
                            Some(b) => n += b.len() as u32,
                            None => errors.push(AsmError { line: st.line, msg: format!("bad string {item}") }),
                        }
                    } else {
                        n += 1;
                    }
                }
                if n == 0 && split_operands(&st.args).is_empty() {
                    errors.push(AsmError { line: st.line, msg: "DEFB needs at least one value".into() });
                }
                addr = addr.wrapping_add(n);
            }
            "DEFS" | "SPACE" => {
                let syms = |n: &str| symbols.get(n).copied();
                match eval_expr(&st.args, &syms) {
                    Ok(v) if v >= 0 => addr = addr.wrapping_add(v as u32),
                    Ok(_) => errors.push(AsmError { line: st.line, msg: "DEFS size cannot be negative".into() }),
                    Err(e) => errors.push(AsmError { line: st.line, msg: format!("DEFS: {}", e.msg()) }),
                }
            }
            "END" => ended = true,
            _ => {}
        }
    }
    for (l, line) in pending_labels.drain(..) {
        define(&mut symbols, &mut label_lines, &mut errors, &l, addr as i64, line);
    }
    flush_pool!(None::<usize>);
    let _ = addr;

    // Resolve EQUs that referred forward.
    let mut progress = true;
    while progress && !deferred_equ.is_empty() {
        progress = false;
        let mut still = Vec::new();
        for (name, expr, line) in deferred_equ.drain(..) {
            let syms = |n: &str| symbols.get(n).copied();
            match eval_expr(&expr, &syms) {
                Ok(v) => {
                    define(&mut symbols, &mut label_lines, &mut errors, &name, v, line);
                    progress = true;
                }
                Err(ExprErr::Undefined(_)) => still.push((name, expr, line)),
                Err(e) => errors.push(AsmError { line, msg: e.msg() }),
            }
        }
        deferred_equ = still;
    }
    for (_, expr, line) in deferred_equ {
        let syms = |n: &str| symbols.get(n).copied();
        if let Err(e) = eval_expr(&expr, &syms) {
            errors.push(AsmError { line, msg: e.msg() });
        }
    }

    // ---------------- Pass 2: encode ----------------
    let mut out = Output { segments: Vec::new() };
    let mut prog = Program::default();

    let emit_pool = |pool: &Pool, out: &mut Output, prog: &mut Program, errors: &mut Vec<AsmError>| {
        for (i, (line, expr)) in pool.entries.iter().enumerate() {
            let ctx = Ctx { symbols: &symbols, addr: 0 };
            let v = match ctx.eval_u32(expr) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(AsmError { line: *line, msg: e });
                    0
                }
            };
            let a = pool.addr.wrapping_add(4 * i as u32);
            out.emit(a, &v.to_le_bytes());
            prog.listing.push(ListingRow { addr: a, bytes: v.to_le_bytes().to_vec(), line: None, is_code: false });
        }
    };

    for (si, st) in stmts.iter().enumerate() {
        if let Some(pidxs) = pool_flush_at.get(&si) {
            for &p in pidxs {
                emit_pool(&pools[p], &mut out, &mut prog, &mut errors);
            }
        }
        let Some(op) = st.op.as_deref() else { continue };
        if op == "END" {
            break;
        }
        let addr = stmt_addr[si];
        let ctx = Ctx { symbols: &symbols, addr };
        let err = |errors: &mut Vec<AsmError>, msg: String| errors.push(AsmError { line: st.line, msg });

        if !is_directive(op) {
            let Some(m) = parse_mnemonic(op) else { continue };
            let pool_info = lit_slot.get(&si).map(|&(p, s)| pools[p].addr.wrapping_add(4 * s as u32));
            match encode_instr(&ctx, &m, &st.args, pool_info) {
                Ok(words) => {
                    for (i, w) in words.iter().enumerate() {
                        let a = addr.wrapping_add(4 * i as u32);
                        out.emit(a, &w.to_le_bytes());
                        prog.addr_to_line.insert(a, st.line);
                        prog.listing.push(ListingRow {
                            addr: a,
                            bytes: w.to_le_bytes().to_vec(),
                            line: Some(st.line),
                            is_code: true,
                        });
                    }
                    prog.line_to_addr.entry(st.line).or_insert(addr);
                }
                Err(e) => err(&mut errors, format!("{}: {e}", st.op_raw.to_uppercase())),
            }
            continue;
        }

        let mut bytes: Vec<u8> = Vec::new();
        match op {
            "DEFW" | "DCD" => {
                for item in split_operands(&st.args) {
                    match ctx.eval_u32(&item) {
                        Ok(v) => bytes.extend_from_slice(&v.to_le_bytes()),
                        Err(e) => {
                            err(&mut errors, format!("DEFW: {e}"));
                            bytes.extend_from_slice(&[0; 4]);
                        }
                    }
                }
            }
            "DEFH" | "DCW" => {
                for item in split_operands(&st.args) {
                    match ctx.eval(&item) {
                        Ok(v) if (-0x8000..=0xFFFF).contains(&v) => bytes.extend_from_slice(&(v as u16).to_le_bytes()),
                        Ok(v) => {
                            err(&mut errors, format!("DEFH: value {v} does not fit in 16 bits"));
                            bytes.extend_from_slice(&[0; 2]);
                        }
                        Err(e) => {
                            err(&mut errors, format!("DEFH: {e}"));
                            bytes.extend_from_slice(&[0; 2]);
                        }
                    }
                }
            }
            "DEFB" | "DCB" => {
                for item in split_operands(&st.args) {
                    if is_string_item(&item) {
                        if let Some(b) = parse_string_lit(&item) {
                            bytes.extend_from_slice(&b);
                        }
                    } else {
                        match ctx.eval(&item) {
                            Ok(v) if (-128..=255).contains(&v) => bytes.push(v as u8),
                            Ok(v) => {
                                err(&mut errors, format!("DEFB: value {v} does not fit in a byte"));
                                bytes.push(0);
                            }
                            Err(e) => {
                                err(&mut errors, format!("DEFB: {e}"));
                                bytes.push(0);
                            }
                        }
                    }
                }
            }
            "DEFS" | "SPACE" => {
                if let Ok(v) = ctx.eval(&st.args) {
                    if (0..=16 * 1024 * 1024).contains(&v) {
                        bytes = vec![0; v as usize];
                    } else if v > 0 {
                        err(&mut errors, "DEFS: size too large".into());
                    }
                }
            }
            _ => {}
        }
        if !bytes.is_empty() {
            out.emit(addr, &bytes);
            for (i, chunk) in bytes.chunks(4).enumerate() {
                prog.listing.push(ListingRow {
                    addr: addr.wrapping_add(4 * i as u32),
                    bytes: chunk.to_vec(),
                    line: if i == 0 { Some(st.line) } else { None },
                    is_code: false,
                });
            }
            prog.line_to_addr.entry(st.line).or_insert(addr);
        }
    }
    for &p in &end_pool_flush {
        emit_pool(&pools[p], &mut out, &mut prog, &mut errors);
    }

    if !errors.is_empty() {
        errors.sort_by_key(|e| e.line);
        errors.dedup();
        return Err(errors);
    }

    prog.listing.sort_by_key(|r| r.addr);
    prog.segments = out.segments;
    prog.entry = entry.or(first_code).unwrap_or(0);
    prog.symbols = symbols.into_iter().map(|(k, v)| (k, v as u32)).collect();
    Ok(prog)
}

fn encode_instr(ctx: &Ctx, m: &Mnemonic, args: &str, lit_addr: Option<u32>) -> Result<Vec<u32>, String> {
    let ops = split_operands(args);
    let cond = m.cond;
    let s = m.extra == "S";
    let n = ops.len();
    let need = |k: usize| -> Result<(), String> {
        if n < k { Err(format!("expected {k} operands, found {n}")) } else { Ok(()) }
    };

    match m.base {
        Base::Dp(opc) => {
            match opc {
                0xD | 0xF => {
                    need(2)?;
                    let rd = ctx.reg(&ops[0])?;
                    Ok(vec![encode_dp(cond, opc, s, 0, rd, ctx.op2(&ops[1..])?)?])
                }
                0x8..=0xB => {
                    need(2)?;
                    let rn = ctx.reg(&ops[0])?;
                    Ok(vec![encode_dp(cond, opc, true, rn, 0, ctx.op2(&ops[1..])?)?])
                }
                _ => {
                    need(2)?;
                    let rd = ctx.reg(&ops[0])?;
                    // Allow the two-operand shorthand: ADD R0, #1 == ADD R0, R0, #1
                    // and ADD R0, R1, LSL #2 == ADD R0, R0, R1, LSL #2.
                    let is_shift = |s: &str| {
                        let w = s.split(|c: char| c.is_whitespace() || c == '#').next().unwrap_or("");
                        shift_type(w).is_some() || s.trim().eq_ignore_ascii_case("RRX")
                    };
                    let (rn, rest) = if n == 2 || (n == 3 && is_shift(&ops[2])) {
                        (rd, &ops[1..])
                    } else {
                        (ctx.reg(&ops[1])?, &ops[2..])
                    };
                    Ok(vec![encode_dp(cond, opc, s, rn, rd, ctx.op2(rest)?)?])
                }
            }
        }
        Base::Shift(t) => {
            // LSL Rd, Rm, #n | LSL Rd, Rm, Rs | LSL Rd, #n
            need(2)?;
            let rd = ctx.reg(&ops[0])?;
            let (rm, amt) = if n >= 3 { (ctx.reg(&ops[1])?, &ops[2]) } else { (rd, &ops[1]) };
            let name = ["LSL", "LSR", "ASR", "ROR"][t as usize];
            let sh = ctx.shift(&format!("{name} {amt}"), true)?;
            Ok(vec![encode_dp(cond, 0xD, s, 0, rd, Op2::Reg(rm, sh))?])
        }
        Base::Rrx => {
            need(1)?;
            let rd = ctx.reg(&ops[0])?;
            let rm = if n >= 2 { ctx.reg(&ops[1])? } else { rd };
            Ok(vec![encode_dp(cond, 0xD, s, 0, rd, Op2::Reg(rm, Shift::Rrx))?])
        }
        Base::Nop => Ok(vec![0xE1A0_0000]),
        Base::Mul | Base::Mla => {
            let acc = m.base == Base::Mla;
            need(if acc { 4 } else { 3 })?;
            let rd = ctx.reg(&ops[0])?;
            let rm = ctx.reg(&ops[1])?;
            let rs = ctx.reg(&ops[2])?;
            let rn = if acc { ctx.reg(&ops[3])? } else { 0 };
            Ok(vec![(cond << 28)
                | ((acc as u32) << 21)
                | ((s as u32) << 20)
                | (rd << 16)
                | (rn << 12)
                | (rs << 8)
                | 0x90
                | rm])
        }
        Base::Umull | Base::Umlal | Base::Smull | Base::Smlal => {
            need(4)?;
            let signed = matches!(m.base, Base::Smull | Base::Smlal);
            let acc = matches!(m.base, Base::Umlal | Base::Smlal);
            let lo = ctx.reg(&ops[0])?;
            let hi = ctx.reg(&ops[1])?;
            let rm = ctx.reg(&ops[2])?;
            let rs = ctx.reg(&ops[3])?;
            Ok(vec![(cond << 28)
                | (1 << 23)
                | ((signed as u32) << 22)
                | ((acc as u32) << 21)
                | ((s as u32) << 20)
                | (hi << 16)
                | (lo << 12)
                | (rs << 8)
                | 0x90
                | rm])
        }
        Base::B | Base::Bl => {
            need(1)?;
            let target = ctx.eval_u32(&ops[0])?;
            if target & 3 != 0 {
                return Err(format!("branch target 0x{target:X} is not word aligned"));
            }
            let off = (target as i64 - (ctx.addr as i64 + 8)) >> 2;
            if !(-(1 << 23)..(1 << 23)).contains(&off) {
                return Err("branch target out of range".into());
            }
            let l = if m.base == Base::Bl { 1 << 24 } else { 0 };
            Ok(vec![(cond << 28) | (0b101 << 25) | l | ((off as u32) & 0x00FF_FFFF)])
        }
        Base::Bx => {
            need(1)?;
            Ok(vec![(cond << 28) | 0x012F_FF10 | ctx.reg(&ops[0])?])
        }
        Base::Swi => {
            need(1)?;
            let a = ops[0].trim();
            let v = ctx.eval_u32(a.strip_prefix('#').unwrap_or(a))?;
            if v > 0xFF_FFFF {
                return Err("SWI number must fit in 24 bits".into());
            }
            Ok(vec![(cond << 28) | (0xF << 24) | v])
        }
        Base::Swp => {
            need(3)?;
            let rd = ctx.reg(&ops[0])?;
            let rm = ctx.reg(&ops[1])?;
            let base = ops[2].trim();
            let rn = base
                .strip_prefix('[')
                .and_then(|b| b.strip_suffix(']'))
                .and_then(parse_reg)
                .ok_or("SWP address must be [Rn]")?;
            let b = if m.extra == "B" { 1 << 22 } else { 0 };
            Ok(vec![(cond << 28) | (1 << 24) | b | (rn << 16) | (rd << 12) | 0x90 | rm])
        }
        Base::Mrs => {
            need(2)?;
            let rd = ctx.reg(&ops[0])?;
            let p = match ops[1].trim().to_ascii_uppercase().as_str() {
                "CPSR" | "CPSR_ALL" => 0,
                "SPSR" | "SPSR_ALL" => 1 << 22,
                _ => return Err("MRS source must be CPSR or SPSR".into()),
            };
            Ok(vec![(cond << 28) | 0x010F_0000 | p | (rd << 12)])
        }
        Base::Msr => {
            need(2)?;
            let dest = ops[0].trim().to_ascii_uppercase();
            let (psr, fields) = dest.split_once('_').unwrap_or((dest.as_str(), "ALL"));
            let p = match psr {
                "CPSR" => 0,
                "SPSR" => 1 << 22,
                _ => return Err("MSR destination must be CPSR or SPSR".into()),
            };
            let mask = match fields {
                "ALL" => 0b1001,
                "FLG" | "F" => 0b1000,
                "CTL" | "C" => 0b0001,
                other => {
                    let mut m = 0;
                    for ch in other.chars() {
                        m |= match ch {
                            'C' => 1,
                            'X' => 2,
                            'S' => 4,
                            'F' => 8,
                            _ => return Err(format!("bad PSR field '{other}'")),
                        };
                    }
                    m
                }
            };
            let src = ops[1].trim();
            let base = (cond << 28) | 0x0120_F000 | p | (mask << 16);
            if src.starts_with('#') {
                let v = ctx.imm(src)?;
                let enc = encode_imm(v).ok_or("immediate cannot be encoded")?;
                Ok(vec![base | (1 << 25) | enc])
            } else {
                Ok(vec![base | ctx.reg(src)?])
            }
        }
        Base::Adr | Base::Adrl => {
            need(2)?;
            let rd = ctx.reg(&ops[0])?;
            let target = ctx.eval_u32(&ops[1])?;
            let off = target.wrapping_sub(ctx.addr.wrapping_add(8)) as i32;
            let (opc, mag) = if off >= 0 { (0x4u32, off as u32) } else { (0x2u32, off.unsigned_abs()) };
            let mk = |rn: u32, v: u32| -> u32 {
                (cond << 28) | (1 << 25) | (opc << 21) | (rn << 16) | (rd << 12) | encode_imm(v).unwrap()
            };
            if m.base == Base::Adr {
                if encode_imm(mag).is_none() {
                    return Err("address out of range for ADR; use ADRL".into());
                }
                Ok(vec![mk(15, mag)])
            } else {
                // Split into two encodable chunks.
                for shift in (0..32).step_by(2) {
                    let lo = mag & (0xFFu32.checked_shl(shift).unwrap_or(0));
                    let hi = mag - lo;
                    if encode_imm(lo).is_some() && encode_imm(hi).is_some() {
                        return Ok(vec![mk(15, lo), mk(rd, hi)]);
                    }
                }
                Err("address out of range for ADRL".into())
            }
        }
        Base::Push | Base::Pop => {
            need(1)?;
            let list = parse_reglist(&ops.join(","))?;
            // STMFD SP!, {..} = STMDB ; LDMFD SP!, {..} = LDMIA
            let w = if m.base == Base::Push {
                (cond << 28) | 0x092D_0000
            } else {
                (cond << 28) | 0x08BD_0000
            };
            Ok(vec![w | list])
        }
        Base::Ldm | Base::Stm => {
            need(2)?;
            let load = m.base == Base::Ldm;
            let mut rn_s = ops[0].trim();
            let wb = rn_s.ends_with('!');
            if wb {
                rn_s = rn_s[..rn_s.len() - 1].trim();
            }
            let rn = ctx.reg(rn_s)?;
            let mut list_s = ops[1..].join(",");
            let user = list_s.trim().ends_with('^');
            if user {
                list_s = list_s.trim().trim_end_matches('^').to_string();
            }
            let list = parse_reglist(&list_s)?;
            // (P, U)
            let (p, u) = match (m.extra, load) {
                ("" | "IA", _) => (0, 1),
                ("IB", _) => (1, 1),
                ("DA", _) => (0, 0),
                ("DB", _) => (1, 0),
                ("FD", true) | ("EA", false) => (0, 1),
                ("ED", true) | ("FA", false) => (1, 1),
                ("FA", true) | ("ED", false) => (0, 0),
                ("EA", true) | ("FD", false) => (1, 0),
                _ => unreachable!(),
            };
            Ok(vec![(cond << 28)
                | (0b100 << 25)
                | (p << 24)
                | (u << 23)
                | ((user as u32) << 22)
                | ((wb as u32) << 21)
                | ((load as u32) << 20)
                | (rn << 16)
                | list])
        }
        Base::Ldr | Base::Str => encode_transfer(ctx, m, &ops, lit_addr),
    }
}

fn encode_transfer(ctx: &Ctx, m: &Mnemonic, ops: &[String], lit_addr: Option<u32>) -> Result<Vec<u32>, String> {
    let load = m.base == Base::Ldr;
    let cond = m.cond;
    if ops.len() < 2 {
        return Err("expected a register and an address".into());
    }
    let rd = ctx.reg(&ops[0])?;
    let halfword = matches!(m.extra, "H" | "SH" | "SB");
    let byte = matches!(m.extra, "B" | "BT");
    let t = matches!(m.extra, "T" | "BT");

    // LDR Rd, =value
    if let Some(lit) = ops[1].trim().strip_prefix('=') {
        if !load || m.extra != "" {
            return Err("'=value' can only be used with LDR".into());
        }
        let v = ctx.eval_u32(lit)?;
        if let Some(enc) = encode_imm(v) {
            return Ok(vec![(cond << 28) | (1 << 25) | (0xD << 21) | (rd << 12) | enc]);
        }
        if let Some(enc) = encode_imm(!v) {
            return Ok(vec![(cond << 28) | (1 << 25) | (0xF << 21) | (rd << 12) | enc]);
        }
        let pool = lit_addr.ok_or("internal error: no literal pool")?;
        let off = pool as i64 - (ctx.addr as i64 + 8);
        if off.abs() > 4095 {
            return Err("literal pool out of range; add an LTORG directive nearer this instruction".into());
        }
        let u = if off >= 0 { 1 << 23 } else { 0 };
        return Ok(vec![(cond << 28) | (0b01 << 26) | (1 << 24) | u | (1 << 20) | (15 << 16) | (rd << 12) | off.unsigned_abs() as u32]);
    }

    // Parse addressing mode.
    // (rn, pre, wb, offset) where offset is Imm(i64) or Reg(sign, rm, shift)
    enum Off {
        Imm(i64),
        Reg(bool, u32, Shift),
    }
    let first = ops[1].trim();
    let (rn, pre, wb, off): (u32, bool, bool, Off);
    if first.starts_with('[') {
        let close = first.find(']').ok_or("missing ']'")?;
        let inner = &first[1..close];
        let after = first[close + 1..].trim();
        let inner_ops = split_operands(inner);
        if inner_ops.is_empty() {
            return Err("missing base register".into());
        }
        rn = ctx.reg(&inner_ops[0])?;
        let parse_off = |parts: &[String]| -> Result<Off, String> {
            if parts.is_empty() {
                return Ok(Off::Imm(0));
            }
            let p0 = parts[0].trim();
            if let Some(e) = p0.strip_prefix('#') {
                if parts.len() > 1 {
                    return Err("unexpected operand after immediate offset".into());
                }
                return ctx.eval(e).map(Off::Imm);
            }
            let (neg, r) = if let Some(r) = p0.strip_prefix('-') {
                (true, r)
            } else {
                (false, p0.strip_prefix('+').unwrap_or(p0))
            };
            let rm = ctx.reg(r)?;
            let sh = if parts.len() > 1 { ctx.shift(&parts[1], false)? } else { Shift::None };
            if parts.len() > 2 {
                return Err("too many operands".into());
            }
            Ok(Off::Reg(neg, rm, sh))
        };
        if ops.len() > 2 {
            // Post-indexed: [Rn], offset
            if !inner_ops[1..].is_empty() || !after.is_empty() {
                return Err("bad post-indexed address".into());
            }
            pre = false;
            wb = false;
            off = parse_off(&ops[2..])?;
        } else {
            pre = true;
            wb = after == "!";
            if !after.is_empty() && !wb {
                return Err(format!("unexpected '{after}' after address"));
            }
            off = parse_off(&inner_ops[1..])?;
        }
    } else {
        if ops.len() > 2 {
            return Err("too many operands".into());
        }
        // PC-relative label.
        let target = ctx.eval_u32(first)?;
        rn = 15;
        pre = true;
        wb = false;
        off = Off::Imm(target as i64 - (ctx.addr as i64 + 8));
    }
    if t && pre {
        return Err("T suffix requires post-indexed addressing".into());
    }

    let l = (load as u32) << 20;
    let p = (pre as u32) << 24;
    let w = ((wb || (t && !pre)) as u32) << 21;
    if halfword {
        if !load && m.extra != "H" {
            return Err("only STRH is valid for stores".into());
        }
        let sh = match m.extra {
            "H" => 0b01,
            "SB" => 0b10,
            _ => 0b11,
        };
        let base = (cond << 28) | p | w | l | (rn << 16) | (rd << 12) | (1 << 7) | (sh << 5) | (1 << 4);
        match off {
            Off::Imm(v) => {
                if v.abs() > 255 {
                    return Err(format!("offset {v} out of range (-255..255) for halfword transfer"));
                }
                let u = if v >= 0 { 1 << 23 } else { 0 };
                let a = v.unsigned_abs() as u32;
                Ok(vec![base | (1 << 22) | u | ((a & 0xF0) << 4) | (a & 0xF)])
            }
            Off::Reg(neg, rm, sh) => {
                if !matches!(sh, Shift::None) {
                    return Err("halfword transfers cannot use shifted register offsets".into());
                }
                let u = if neg { 0 } else { 1 << 23 };
                Ok(vec![base | u | rm])
            }
        }
    } else {
        let b = (byte as u32) << 22;
        let base = (cond << 28) | (0b01 << 26) | p | w | b | l | (rn << 16) | (rd << 12);
        match off {
            Off::Imm(v) => {
                if v.abs() > 4095 {
                    return Err(format!("offset {v} out of range (-4095..4095)"));
                }
                let u = if v >= 0 { 1 << 23 } else { 0 };
                Ok(vec![base | u | v.unsigned_abs() as u32])
            }
            Off::Reg(neg, rm, sh) => {
                if matches!(sh, Shift::Reg(..)) {
                    return Err("register-specified shifts are not allowed in addresses".into());
                }
                let u = if neg { 0 } else { 1 << 23 };
                Ok(vec![base | (1 << 25) | u | encode_shift(rm, sh)])
            }
        }
    }
}
