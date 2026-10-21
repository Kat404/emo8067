//! Assembling a program.
//!
//! A program is a list of lines. The directives say where it is and what it
//! holds, the instructions are the bytes, and the names are collected as the
//! program is walked.
//!
//! The program is walked twice. The first time the names are collected and
//! the addresses counted; the second time the bytes are made, and by then a
//! jump knows where it is going. Most jumps keep their size wherever they
//! go, but a short conditional jump that is too far is replaced by the
//! reference's own workaround, and the replacement is longer: the first
//! walk counts the replacement, and the program is walked again and again
//! until the names stop moving.

use crate::assemble;
use crate::directives::{self, Directive};
use crate::groups;
use crate::line;
use crate::literals;
use crate::macros::{self, Macro};
use crate::operands::{Memory, Operand};
use crate::table;
use std::path::{Path, PathBuf};

/// What went wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The line has no instruction on it.
    Empty,
    /// The instruction is not in the document's table.
    UnknownInstruction { name: String },
    /// The operands do not fit.
    BadOperand { text: String },
    /// A name that the program does not define.
    UnknownName { name: String },
    /// A macro that calls itself for ever, in the reference's own words.
    RecursiveMacro { message: String },
    /// A conditional jump that is out of range, in the reference's own words.
    JumpOutOfRange { message: String },
    /// The program's addresses did not settle: the names moved on every
    /// walk, and the layout is not known.
    NoConvergence { message: String },
    /// An include file that cannot be brought in, in the reference's own
    /// words.
    Include { message: String },
    /// Something went wrong on a line of the source.
    AtLine { line: usize, error: Box<Error> },
}

/// A program, assembled.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Program {
    /// Where the program starts.
    pub origin: u16,
    /// The bytes.
    pub bytes: Vec<u8>,
    /// The names and the address they are at.
    pub names: Vec<(String, u16)>,
}

/// What a line does to the program.
enum Effect {
    /// Nothing at all.
    Nothing,
    /// A name at this address.
    Label(String),
    /// A name and its value.
    Constant,
    /// The program starts here.
    Origin(u16),
    /// The program ends here.
    End,
    /// These bytes go into the program, and the label they carry.
    Bytes(String, Vec<u8>),
    /// An instruction, the label it carries, the address it is at, and how
    /// long it is.
    Instruction(String, Vec<String>, u16, usize),
}

/// Assemble a program.
pub fn assemble_program(source: &str) -> Result<Program, Error> {
    // A macro is a line of the source that stands for other lines.
    let macros = macros::read(source);
    let known = known_names(&macros);
    // Every line keeps the number it has in the source, so that a refusal
    // can say where it happened.
    let source_lines: Vec<(usize, String)> = macros::strip(source)
        .into_iter()
        .filter(|(_, raw)| !raw.trim().is_empty())
        .collect();
    let source_lines = expand_macros(source_lines, &macros)?;
    // The labels come after the macros are expanded: the body of a macro
    // carries labels of its own, and the reference's own examples write
    // them without a colon.
    let source_lines: Vec<(usize, String)> = source_lines
        .into_iter()
        .map(|(number, raw)| (number, label_first(&raw, &known)))
        .filter(|(_, raw)| !raw.trim().is_empty())
        .collect();
    // A line with nothing on it - blank, or only a comment - is not code.
    let lines: Vec<(usize, line::Line)> = source_lines
        .iter()
        .filter_map(|(number, raw)| line::parse(raw).ok().map(|parsed| (*number, parsed)))
        .collect();
    // The segments the source declares. A segment is a name the program
    // gives a part of itself, and the flat program has one segment, so a
    // segment name is worth nothing as a number.
    let segments = segments_of(&lines);
    // The constants come first: a directive can count by a constant the
    // program declared later, so they are read before the walk that needs
    // them. A constant whose value needs an address - `size = ($ - x1) / 2` -
    // cannot be read yet, and the walk that counts the addresses reads it.
    let mut constants: Vec<(String, Vec<i64>)> = Vec::new();
    for (_, parsed) in &lines {
        if let Some((name, values)) = equ(parsed, &constants, &[], 0) {
            constants.push((name, values));
        }
    }
    // The names and the addresses, counted again and again until they stop
    // moving. A reference that names a label in its displacement -
    // `sum[bx]` - is a byte wider when the label is out of reach of a
    // signed byte, and the program has to know that before it counts.
    let mut names: Vec<(String, u16)> = Vec::new();
    let mut origin = 0u16;
    let mut settled = false;
    for _ in 0..MAX_PASSES {
        let counted = count(&lines, &mut constants, &segments, &names)?;
        settled = counted.names == names;
        names = counted.names;
        origin = counted.origin;
        if settled {
            break;
        }
    }
    // The names must stop moving: the layout the walk that counts settled
    // on is the layout the walk that makes the bytes produces, and that is
    // true only when the names are the same in two walks in a row.
    if !settled {
        return Err(Error::NoConvergence {
            message: format!("the program's addresses did not settle after {MAX_PASSES} passes"),
        });
    }
    // The second walk: the bytes, with the names known.
    let mut program = Program {
        origin,
        bytes: Vec::new(),
        names: names.clone(),
    };
    let mut address = origin;
    for (number, parsed) in &lines {
        match effect(parsed, &mut constants, &segments, &names, address, true).map_err(|error| {
            Error::AtLine {
                line: *number,
                error: Box::new(error),
            }
        })? {
            Effect::Nothing | Effect::Label(..) | Effect::Constant | Effect::Origin(..) => {}
            Effect::End => break,
            Effect::Bytes(_, bytes) => {
                program.bytes.extend_from_slice(&bytes);
                address = address.wrapping_add(bytes.len() as u16);
            }
            Effect::Instruction(_, texts, at, size) => {
                let bytes =
                    assemble_instruction(parsed, &texts, &constants, &segments, &names, at)?;
                program.bytes.extend_from_slice(&bytes);
                address = at.wrapping_add(size as u16);
            }
        }
    }
    Ok(program)
}

/// Assemble a file, with the includes it names.
///
/// The include is textual and happens before the macros are read, because
/// the library's macros must be visible to the whole program.
pub fn assemble_file(path: &Path, include_dirs: &[PathBuf]) -> Result<Program, Error> {
    let text = splice_includes(path, include_dirs)?;
    assemble_program(&text)
}

/// The source of a file, with its includes spliced in.
///
/// The text is the one the assembler reads, so a refusal that says which
/// line it happened on can be read against it.
pub fn splice_includes(path: &Path, include_dirs: &[PathBuf]) -> Result<String, Error> {
    let bytes = std::fs::read(path).map_err(|_| Error::Include {
        message: format!("file not found: {}", path.display()),
    })?;
    // Some of the reference's own files are not UTF-8; the bytes that are
    // not are in comments, and a lossy read leaves them alone.
    let text = String::from_utf8_lossy(&bytes);
    let directory = path.parent().unwrap_or(Path::new("."));
    splice_text(&text, directory, include_dirs, 0)
}

/// How many includes deep the splicing goes before it gives up.
const MAX_INCLUDE_DEPTH: usize = 16;

/// The source with the includes spliced in.
fn splice_text(
    text: &str,
    directory: &Path,
    include_dirs: &[PathBuf],
    depth: usize,
) -> Result<String, Error> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(Error::Include {
            message: "recursive include file?".to_string(),
        });
    }
    let mut out = String::new();
    for line in text.lines() {
        let Some(name) = include_name(line) else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let found = find_include(name, directory, include_dirs).ok_or_else(|| Error::Include {
            message: format!("file not found: {name}"),
        })?;
        let bytes = std::fs::read(&found).map_err(|_| Error::Include {
            message: format!("file not found: {name}"),
        })?;
        let included = String::from_utf8_lossy(&bytes);
        let nested = splice_text(
            &included,
            found.parent().unwrap_or(directory),
            include_dirs,
            depth + 1,
        )?;
        out.push_str(&nested);
    }
    Ok(out)
}

/// The name of the file an include line names.
fn include_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let (word, rest) = trimmed.split_once(char::is_whitespace)?;
    if !word.eq_ignore_ascii_case("INCLUDE") {
        return None;
    }
    let rest = rest.trim();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

/// The file an include names: the directory of the source first, and then the
/// include directories the caller passes, in order. The first one there
/// wins.
fn find_include(name: &str, directory: &Path, include_dirs: &[PathBuf]) -> Option<PathBuf> {
    let mut candidates = vec![directory.join(name)];
    for include_dir in include_dirs {
        candidates.push(include_dir.join(name));
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// What one walk of the program found.
struct Counted {
    /// The names and the address they are at.
    names: Vec<(String, u16)>,
    /// Where the program starts.
    origin: u16,
}

/// How many times the program is walked before the names are believed.
const MAX_PASSES: usize = 8;

/// Walk the program once, to count the addresses.
fn count(
    lines: &[(usize, line::Line)],
    constants: &mut Vec<(String, Vec<i64>)>,
    segments: &[String],
    known: &[(String, u16)],
) -> Result<Counted, Error> {
    let mut origin = 0u16;
    let mut address = 0u16;
    let mut names: Vec<(String, u16)> = Vec::new();
    for (number, parsed) in lines {
        match effect(parsed, constants, segments, known, address, false).map_err(|error| {
            Error::AtLine {
                line: *number,
                error: Box::new(error),
            }
        })? {
            Effect::Nothing => {}
            Effect::Label(name) => names.push((name, address)),
            Effect::Constant => {}
            Effect::Origin(start) => {
                origin = start;
                address = start;
            }
            Effect::End => break,
            Effect::Bytes(label, bytes) => {
                if !label.is_empty() {
                    names.push((label, address));
                }
                address = address.wrapping_add(bytes.len() as u16);
            }
            Effect::Instruction(label, _, at, size) => {
                if !label.is_empty() {
                    names.push((label, at));
                }
                address = at.wrapping_add(size as u16);
            }
        }
    }
    Ok(Counted { names, origin })
}

/// The names a line may begin with and still be an instruction.
///
/// The table's own mnemonics, the groups' operations, the directives and
/// the macros the source defines.
fn known_names(macros: &[Macro]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for cell in table::forms() {
        for part in cell.mnemonic.split('/') {
            names.push(part.trim().to_string());
        }
    }
    // The guide's jumps carry two names, and the source may use either:
    // `JE/JZ short` answers to JE and to JZ. The reference has a few more
    // of its own: `JC` is the carry jump and `JNC` is the one that is not.
    for name in [
        "JZ", "JNZ", "JNAE", "JNB", "JAE", "JC", "JNC", "JNBE", "JA", "JNA", "JBE", "JNG", "JLE",
        "JNGE", "JL", "JGE", "JNL", "JG", "JNLE", "JPE", "JPO", "LOOPNZ", "LOOPZ",
    ] {
        names.push(name.to_string());
    }
    // The guide writes the string instructions with their width apart -
    // `MOVS b` - and the reference writes them as one word: `MOVSB`.
    for name in [
        "MOVSB", "MOVSW", "CMPSB", "CMPSW", "STOSB", "STOSW", "LODSB", "LODSW", "SCASB", "SCASW",
        "XLATB",
    ] {
        names.push(name.to_string());
    }
    // The reference's own examples push all the registers and pop them
    // again: `PUSHA` and `POPA`.
    for name in ["PUSHA", "POPA"] {
        names.push(name.to_string());
    }
    // The prefixes come before the instruction, and a line that begins with
    // one is an instruction and not a label.
    for name in [
        "REPE", "REPZ", "REPNE", "REPNZ", "REP", "LOCK", "CS", "DS", "ES", "SS",
    ] {
        names.push(name.to_string());
    }
    // The MASM-style sections, which the reference's own examples write
    // with a dot in front: `.data`, `.code`, `.stack 64h`.
    for name in [".DATA", ".CODE", ".STACK", ".MODEL", ".EXIT"] {
        names.push(name.to_string());
    }
    for group in groups::forms() {
        for name in group.names() {
            names.push(name.to_string());
        }
    }
    for name in directives::NAMES {
        names.push((*name).to_string());
    }
    for mac in macros {
        names.push(mac.name.clone());
    }
    names
}

/// Give a leading name a colon, when the name is a label and not an
/// instruction.
///
/// The reference writes a label without a colon when the instruction follows
/// it on the same line: `str1 db 'abc'`. The parser reads a label with a
/// colon, so the colon is put in here - the one place that knows which names
/// are instructions.
fn label_first(raw: &str, known: &[String]) -> String {
    let raw = raw.trim_end_matches('\r');
    let trimmed = raw.trim_start();
    // The reference's own build directives are not code, and a comment is
    // not code either.
    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return String::new();
    }
    let Some((first, rest)) = trimmed.split_once(char::is_whitespace) else {
        return raw.to_string();
    };
    let name = first.trim_end_matches(':');
    if name.is_empty() || rest.trim().is_empty() {
        return raw.to_string();
    }
    // The name is already a label: `start:`, or a label and the instruction
    // that follows it on the same line, `nsp:mov [di-1], 0`. A segment
    // override is not a label: `cs:mov ax, 1`.
    if first.contains(':') {
        let name = first.split(':').next().unwrap_or_default();
        if !known.iter().any(|known| known.eq_ignore_ascii_case(name)) {
            return raw.to_string();
        }
    }
    if known.iter().any(|known| known.eq_ignore_ascii_case(name)) {
        return raw.to_string();
    }
    format!("{name}:{rest}")
}

/// The next number an expansion is given, so that two expansions of the
/// same macro land on different labels.
fn next_id() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Replace every call to a macro with its body.
fn expand_macros(
    lines: Vec<(usize, String)>,
    macros: &[Macro],
) -> Result<Vec<(usize, String)>, Error> {
    expand_at(lines, macros, 0)
}

/// The same, one pass deeper.
fn expand_at(
    lines: Vec<(usize, String)>,
    macros: &[Macro],
    depth: usize,
) -> Result<Vec<(usize, String)>, Error> {
    if depth > macros::MAX_DEPTH {
        return Err(Error::RecursiveMacro {
            message: format!("recursive MACRO definition is over {}", macros::MAX_DEPTH),
        });
    }
    let mut out: Vec<(usize, String)> = Vec::new();
    for (number, raw) in lines {
        let Ok(parsed) = line::parse(&raw) else {
            out.push((number, raw));
            continue;
        };
        match macros
            .iter()
            .find(|mac| mac.name.eq_ignore_ascii_case(&parsed.mnemonic))
        {
            Some(mac) => {
                let body: Vec<(usize, String)> = macros::expand(mac, &parsed.operands, next_id())
                    .into_iter()
                    .map(|line| (number, line))
                    .collect();
                out.extend(expand_at(body, macros, depth + 1)?);
            }
            None => out.push((number, raw)),
        }
    }
    Ok(out)
}

/// The segments the source declares.
///
/// The reference writes them two ways: `data segment ... ends` and the
/// MASM-style `.data` and `.code`. A segment is a name, and the flat
/// program this assembler makes has one segment, so the name is worth
/// nothing as a number.
fn segments_of(lines: &[(usize, line::Line)]) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    for (_, parsed) in lines {
        let mnemonic = parsed.mnemonic.trim_start_matches('.').to_ascii_uppercase();
        if directives::is_segment(&mnemonic) {
            // `.data` is the segment called `data`.
            segments.push(mnemonic.to_ascii_lowercase());
        }
        if mnemonic == "SEGMENT" {
            if let Some(label) = &parsed.label {
                segments.push(label.to_ascii_lowercase());
            }
        }
    }
    segments
}

/// What a line does, with the names and the constants the program has.
fn effect(
    parsed: &line::Line,
    constants: &mut Vec<(String, Vec<i64>)>,
    segments: &[String],
    names: &[(String, u16)],
    address: u16,
    strict: bool,
) -> Result<Effect, Error> {
    // The reference writes the name first and the directive after it:
    // `SIZE EQU 10`. The two are put back the right way round.
    let swapped = swap_if_directive(parsed);
    // A constant: `SIZE EQU 10`, `count equ 10`, `precision = 30`. Its
    // value may need an address - `size = ($ - x1) / 2` - and the walk that
    // counts the addresses is the one that knows it.
    if let Some((name, values)) = equ(&swapped, constants, names, address) {
        constants.retain(|(known, _)| !known.eq_ignore_ascii_case(&name));
        constants.push((name.clone(), values.clone()));
        let _ = (name, values);
        return Ok(Effect::Constant);
    }
    // A constant whose value the program cannot read yet - `size = ($ - x1)
    // / 2`, whose value needs an address - is not an instruction. The walk
    // that counts the addresses reads it.
    if swapped.mnemonic == "=" || swapped.mnemonic.eq_ignore_ascii_case("EQU") {
        return Ok(Effect::Nothing);
    }
    // A directive. The names it names are the program's own: `len dup(0)`
    // counts by a constant the program declared.
    let folded: Vec<String> = fold_directive(&swapped.operands, constants, names, address);
    if let Some(directive) = directives::parse(&swapped.mnemonic, &folded) {
        return Ok(match directive {
            Directive::Org(origin) => Effect::Origin(origin),
            Directive::Db(values) => Effect::Bytes(
                swapped.label.clone().unwrap_or_default(),
                bytes_of(&values, 1),
            ),
            Directive::Dw(values) => Effect::Bytes(
                swapped.label.clone().unwrap_or_default(),
                bytes_of(&values, 2),
            ),
            Directive::Dd(values) => Effect::Bytes(
                swapped.label.clone().unwrap_or_default(),
                bytes_of(&values, 4),
            ),
            Directive::Equ(..) => Effect::Constant,
            Directive::Label(name) => Effect::Label(name),
            Directive::End => Effect::End,
            // A directive that says nothing to the bytes may still carry
            // the name of a procedure: `start proc far`.
            Directive::Ignored => match &swapped.label {
                Some(name) => Effect::Label(name.clone()),
                None => Effect::Nothing,
            },
        });
    }
    // A line that carries nothing but a label has no instruction.
    if swapped.mnemonic.is_empty() {
        return Ok(match &swapped.label {
            Some(name) => Effect::Label(name.clone()),
            None => Effect::Nothing,
        });
    }
    // The prefixes come before the instruction, so the opcode is not where
    // the line starts: a jump's displacement is measured from the end of
    // the whole of it.
    let prefixes = swapped.prefix_bytes();
    let opcode = address.wrapping_add(prefixes.len() as u16);
    // The reference's own examples push all the registers and pop them
    // again. The 8086 has neither instruction - the guide's row 60 is the
    // conditional jumps' own alias, and the captures from a real P80C86A-2
    // show `60h` running `JO` - and the reference's own documentation calls
    // it an 80186 instruction and gives the sequence it stands for.
    if let Some(bytes) = push_all(&swapped.mnemonic) {
        let mut all = prefixes;
        all.extend(bytes);
        return Ok(Effect::Bytes(
            swapped.label.clone().unwrap_or_default(),
            all,
        ));
    }
    // An instruction. Its size is what the assembler says, and a jump's
    // size does not depend on where it goes, so a target of zero is enough
    // to count it.
    let jump = is_jump(&swapped.mnemonic);
    let texts: Vec<String> = swapped.operands.clone();
    let operands = operands_of(&Context {
        texts: &texts,
        mnemonic: &swapped.mnemonic,
        qualifier: swapped.qualifier.as_deref(),
        constants,
        segments,
        names,
        address,
        jump,
        strict,
    })?;
    // A jump whose target the program has not declared yet: the first walk
    // cannot know how far it is, so the jump is counted as it is and read
    // again in the next walk, when the name is known.
    let operands: Vec<Operand> = if strict {
        operands
    } else {
        operands
            .into_iter()
            .map(|operand| match operand {
                Operand::Name(_) => Operand::Relative(i64::from(address)),
                other => other,
            })
            .collect()
    };
    // A conditional jump whose target is too far is replaced by the
    // reference's own workaround, and the replacement is what the program
    // counts.
    if jump {
        if let Some(bytes) = replacement(&swapped.mnemonic, &operands, opcode)? {
            let size = prefixes.len() + bytes.len();
            let mut all = prefixes;
            all.extend(bytes);
            return Ok(Effect::Instruction(
                swapped.label.clone().unwrap_or_default(),
                texts,
                address,
                size,
            ));
        }
    }
    // The first walk does not know where a jump goes yet, so it does not
    // check the range of its displacement.
    let bytes = match assemble::assemble_parts_at(&swapped, &operands, address, strict) {
        Ok(bytes) => bytes,
        Err(error) => {
            if strict {
                return Err(bad(&swapped, error));
            }
            // The first walk cannot read every line: a name the program
            // declares later is not known yet, and the line is counted
            // again in the next walk, when the name is known.
            return Ok(Effect::Nothing);
        }
    };
    let size = bytes.len();
    Ok(Effect::Instruction(
        swapped.label.clone().unwrap_or_default(),
        texts,
        address,
        size,
    ))
}

/// Assemble one instruction, with the names the program has.
fn assemble_instruction(
    parsed: &line::Line,
    texts: &[String],
    constants: &[(String, Vec<i64>)],
    segments: &[String],
    names: &[(String, u16)],
    address: u16,
) -> Result<Vec<u8>, Error> {
    // The prefixes come before the instruction, so the opcode is not where
    // the line starts.
    let prefixes = parsed.prefix_bytes();
    let opcode = address.wrapping_add(prefixes.len() as u16);
    let jump = is_jump(&parsed.mnemonic);
    let operands = operands_of(&Context {
        texts,
        mnemonic: &parsed.mnemonic,
        qualifier: parsed.qualifier.as_deref(),
        constants,
        segments,
        names,
        address,
        jump,
        strict: true,
    })?;
    // A conditional jump whose target is too far is replaced by the
    // reference's own workaround.
    if jump {
        if let Some(bytes) = replacement(&parsed.mnemonic, &operands, opcode)? {
            let mut all = prefixes;
            all.extend(bytes);
            return Ok(all);
        }
    }
    let result = assemble::assemble_parts(parsed, &operands, address);
    result.map_err(|error| bad(parsed, error))
}

/// The operands of a line, with the program's own names and constants in
/// them.
///
/// The reference's own dialect decides what a name means:
///
/// * the target of a jump is a place to go to,
/// * the destination of an instruction is an address - `mov m, al` writes
///   to the variable `m`,
/// * the source of an instruction is a value - `mov dx, msg` loads the
///   address of `msg`, which is what the reference's own example says it
///   does,
/// * `offset name` is that address as a value, and `seg name` is the
///   segment it is in.
struct Context<'a> {
    /// The operands, as the source wrote them.
    texts: &'a [String],
    /// The instruction's name.
    mnemonic: &'a str,
    /// The size the source asked for, when it said.
    qualifier: Option<&'a str>,
    /// The program's constants.
    constants: &'a [(String, Vec<i64>)],
    /// The segment registers the source named.
    segments: &'a [String],
    /// The program's names and their addresses.
    names: &'a [(String, u16)],
    /// The address the instruction is at.
    address: u16,
    /// Whether the instruction jumps.
    jump: bool,
    /// Whether every name is known.
    strict: bool,
}

fn operands_of(context: &Context<'_>) -> Result<Vec<Operand>, Error> {
    let Context {
        texts,
        mnemonic,
        qualifier,
        constants,
        segments,
        names,
        address,
        jump,
        strict,
    } = *context;
    // A move into an eight-bit register reads the value at a name; a move
    // into a sixteen-bit one takes the name's address for a value.
    let byte = mnemonic.eq_ignore_ascii_case("MOV")
        && texts
            .first()
            .map(|text| is_byte_register(text.trim()))
            .unwrap_or(false);
    let mut out = Vec::with_capacity(texts.len());
    for (index, text) in texts.iter().enumerate() {
        out.push(resolve(
            text, index, mnemonic, qualifier, constants, segments, names, address, jump, byte,
            strict,
        )?);
    }
    // The guide's `XCHG` puts the register in the REG field and the
    // reference in the R/M field, and the reference's own example writes
    // them the other way round: `xchg matrix[bx][si], dl`.
    if mnemonic.eq_ignore_ascii_case("XCHG")
        && out.len() == 2
        && matches!(out[0], Operand::Memory(_))
        && matches!(out[1], Operand::Register(_))
    {
        out.swap(0, 1);
    }
    Ok(out)
}

/// One operand, as the source wrote it.
#[allow(clippy::too_many_arguments)]
fn resolve(
    text: &str,
    index: usize,
    mnemonic: &str,
    qualifier: Option<&str>,
    constants: &[(String, Vec<i64>)],
    segments: &[String],
    names: &[(String, u16)],
    address: u16,
    jump: bool,
    byte: bool,
    strict: bool,
) -> Result<Operand, Error> {
    // The source may write a label with its colon: `jne xor_cursor:`.
    let trimmed = text.trim().trim_end_matches(':').trim();
    // The size the source wrote in front of the operand: `w. week`.
    let (trimmed, width) = take_size(trimmed);
    // An expression: `len-1`, `100+w`, `($ - x1) / 2`, `offset msg_end -
    // offset msg`. It comes before the words `offset` and `seg`, because
    // those may be a term of an expression and not the whole of it.
    if let Some(value) = expression(trimmed, constants, names, address) {
        if mnemonic.eq_ignore_ascii_case("LEA") {
            // `LEA` takes the address of what it is given, and an
            // expression is an address.
            return Ok(Operand::Memory(Memory {
                label: None,
                segment: None,
                base: None,
                index: None,
                displacement: Some(value),
                width: None,
            }));
        }
        return Ok(Operand::Immediate(value));
    }
    // `offset name`: the address of the name, as a value. The name may be
    // a reference: `offset s1[2]`.
    if let Some(rest) = strip_keyword(trimmed, "OFFSET") {
        return match address_of(rest, constants, segments, names, address) {
            Some(value) => Ok(Operand::Immediate(value)),
            None => unknown(rest, strict),
        };
    }
    // `seg name`: the segment the name is in. The program this assembler
    // makes has one segment, so the segment is the program.
    if let Some(rest) = strip_keyword(trimmed, "SEG") {
        return match address_of(rest, constants, segments, names, address) {
            Some(_) => Ok(Operand::Immediate(0)),
            None => unknown(rest, strict),
        };
    }
    // A constant the program declared.
    if let Some(values) = constant(trimmed, constants) {
        return Ok(Operand::Immediate(values));
    }
    // A register or a reference.
    if is_register(trimmed)
        || trimmed.contains('[')
        || trimmed.to_ascii_uppercase().contains("PTR")
        || assemble::has_segment(trimmed)
    {
        let operand = assemble::operand(trimmed).map_err(|_| Error::BadOperand {
            text: trimmed.to_string(),
        })?;
        // A name inside the reference: `num1[bx]`, `[num1]`. The name is
        // the displacement, and the program knows what it is worth.
        return Ok(match operand {
            Operand::Memory(memory) => {
                let mut memory = memory;
                if memory.width.is_none() {
                    memory.width = width;
                }
                if let Some(label) = memory.label.clone() {
                    match names
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(&label))
                    {
                        Some((_, at)) => {
                            memory.displacement =
                                Some(i64::from(*at) + memory.displacement.unwrap_or(0));
                            memory.label = None;
                        }
                        None => {
                            if strict {
                                return Err(Error::UnknownName { name: label });
                            }
                            // The first walk does not know the name yet,
                            // and the size of the reference does not depend
                            // on where it is.
                            memory.displacement = Some(memory.displacement.unwrap_or(0));
                            memory.label = None;
                        }
                    }
                }
                Operand::Memory(memory)
            }
            other => other,
        });
    }
    // A number.
    if let Ok(value) = literals::parse(trimmed) {
        return Ok(Operand::Immediate(value));
    }
    // A name: a label the program defines, or a segment it declares.
    if is_segment_name(trimmed, segments) {
        return Ok(Operand::Immediate(0));
    }
    // A far pointer: `0FFFFh:0000h`.
    if let Some((segment, offset)) = trimmed.split_once(':') {
        if let (Ok(segment), Ok(offset)) = (
            literals::parse(segment.trim()),
            literals::parse(offset.trim()),
        ) {
            return Ok(Operand::Far(offset as u16, segment as u16));
        }
    }
    let known = names
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(trimmed))
        .map(|(_, at)| i64::from(*at));
    match known {
        Some(at) => Ok(shape(at, index, mnemonic, qualifier, jump, byte)),
        None => {
            if strict {
                return Err(Error::UnknownName {
                    name: trimmed.to_string(),
                });
            }
            // The first walk does not know every name yet. A jump's size
            // depends on how far its target is, so an unknown target is
            // kept as the name it is: the walk counts the jump as it is
            // and reads it again in the next walk, when the name is known.
            if jump {
                return Ok(Operand::Name(trimmed.to_string()));
            }
            // The size of an instruction does not depend on the value of
            // its operand, so a name that is not known yet is counted with
            // a zero.
            Ok(shape(0, index, mnemonic, qualifier, jump, byte))
        }
    }
}

/// The operand a name becomes.
///
/// The destination of an instruction is an address, and the source is a
/// value. `LEA` takes an address either way, and so does a far call.
///
/// The reference's own examples make one more distinction: a name that is
/// the source of a move into an eight-bit register is the value at the
/// name - `mov cl, tc` reads the byte `tc` - while a name that is the
/// source of a move into a sixteen-bit register is the address of the
/// name - `mov dx, msg` loads the offset of `msg`, which is what the
/// reference's own comment says it does.
fn shape(
    at: i64,
    index: usize,
    mnemonic: &str,
    qualifier: Option<&str>,
    jump: bool,
    byte: bool,
) -> Operand {
    let takes_address = index == 0
        || mnemonic.eq_ignore_ascii_case("LEA")
        || qualifier.is_some_and(|qualifier| qualifier.eq_ignore_ascii_case("FAR"))
        || (byte && index > 0);
    if jump && !qualifier.is_some_and(|qualifier| qualifier.eq_ignore_ascii_case("FAR")) {
        return Operand::Relative(at);
    }
    if takes_address {
        return Operand::Memory(Memory {
            label: None,
            segment: None,
            base: None,
            index: None,
            displacement: Some(at),
            width: None,
        });
    }
    Operand::Immediate(at)
}

/// The address of a name, as a value.
///
/// The name may be a reference: `s1[2]` is the label plus two.
fn address_of(
    text: &str,
    constants: &[(String, Vec<i64>)],
    segments: &[String],
    names: &[(String, u16)],
    address: u16,
) -> Option<i64> {
    let trimmed = text.trim();
    if let Some(values) = constant(trimmed, constants) {
        return Some(values);
    }
    if is_segment_name(trimmed, segments) {
        return Some(0);
    }
    if let Some(at) = names
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(trimmed))
        .map(|(_, at)| i64::from(*at))
    {
        return Some(at);
    }
    if let Ok(value) = literals::parse(trimmed) {
        return Some(value);
    }
    if trimmed.contains('[') {
        let parsed = assemble::operand(trimmed);
        if let Ok(Operand::Memory(memory)) = parsed {
            let base = match &memory.label {
                Some(label) => names
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(label))
                    .map(|(_, at)| i64::from(*at))?,
                None => 0,
            };
            return Some(base + memory.displacement.unwrap_or(0));
        }
    }
    let _ = address;
    None
}

/// The size the source wrote in front of its operand: `w. week`.
///
/// The reference writes the size as a word and a dot, and the reference's
/// own complaint about a width says so: "add 'b.' before ...".
fn take_size(text: &str) -> (&str, Option<crate::modrm::Width>) {
    let trimmed = text.trim_start();
    let upper = trimmed.to_ascii_uppercase();
    for (word, width) in [
        ("B.", crate::modrm::Width::Byte),
        ("W.", crate::modrm::Width::Word),
    ] {
        if upper.starts_with(word) {
            return (trimmed[word.len()..].trim_start(), Some(width));
        }
    }
    (text, None)
}

/// A name the program does not know yet.
///
/// The first walk does not know every name: it is counted with a zero and
/// read again in the next walk, when the program knows it.
fn unknown(name: &str, strict: bool) -> Result<Operand, Error> {
    if strict {
        return Err(Error::UnknownName {
            name: name.trim().to_string(),
        });
    }
    Ok(Operand::Immediate(0))
}

/// The value of a constant the program declared.
fn constant(text: &str, constants: &[(String, Vec<i64>)]) -> Option<i64> {
    constants
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(text.trim()))
        .and_then(|(_, values)| values.first().copied())
}

/// Whether a name is a segment the program declares.
fn is_segment_name(text: &str, segments: &[String]) -> bool {
    let trimmed = text.trim();
    if trimmed.starts_with('@') {
        return true;
    }
    segments
        .iter()
        .any(|segment| segment.eq_ignore_ascii_case(trimmed))
}

/// The word the source wrote before a name: `offset msg`, `seg msg`.
fn strip_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let (word, rest) = text.split_once(char::is_whitespace)?;
    if !word.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() { None } else { Some(rest) }
}

/// Whether a word names an eight-bit register.
fn is_byte_register(text: &str) -> bool {
    matches!(
        text.to_ascii_uppercase().as_str(),
        "AL" | "CL" | "DL" | "BL" | "AH" | "CH" | "DH" | "BH"
    )
}

/// Whether a word names a register.
fn is_register(text: &str) -> bool {
    matches!(
        text.trim().to_ascii_uppercase().as_str(),
        "AL" | "CL"
            | "DL"
            | "BL"
            | "AH"
            | "CH"
            | "DH"
            | "BH"
            | "AX"
            | "CX"
            | "DX"
            | "BX"
            | "SP"
            | "BP"
            | "SI"
            | "DI"
            | "CS"
            | "DS"
            | "ES"
            | "SS"
            | "IP"
    )
}

/// The error the assembler gave, in the program's own words.
fn bad(parsed: &line::Line, error: assemble::Error) -> Error {
    match error {
        assemble::Error::UnknownInstruction { name } => Error::UnknownInstruction { name },
        assemble::Error::Type { message } => Error::BadOperand {
            text: message.to_string(),
        },
        assemble::Error::OutOfRange { message } => Error::JumpOutOfRange {
            message: message.to_string(),
        },
        _ => Error::BadOperand {
            text: parsed.operands.join(", "),
        },
    }
}

/// Put a directive back where it belongs.
///
/// The reference writes `SIZE EQU 10`: the name stands where the
/// instruction goes, and the directive where its first operand goes. The
/// parser cannot tell, so the swap is made here, where the table is.
fn swap_if_directive(parsed: &line::Line) -> line::Line {
    let Some(first) = parsed.operands.first() else {
        return parsed.clone();
    };
    let trimmed = first.trim();
    // The directive is the first word of the operand, and what follows it
    // is the directive's own operand: `db len dup(0)`.
    let (word, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim().to_string()),
        None => (trimmed, String::new()),
    };
    if !word.eq_ignore_ascii_case("EQU")
        && !word.eq_ignore_ascii_case("DB")
        && !word.eq_ignore_ascii_case("DW")
        && !word.eq_ignore_ascii_case("DD")
        && !word.eq_ignore_ascii_case("ORG")
        && !word.eq_ignore_ascii_case("PROC")
        && !word.eq_ignore_ascii_case("ENDP")
    {
        return parsed.clone();
    }
    let mut operands = Vec::new();
    if !rest.is_empty() {
        operands.push(rest);
    }
    operands.extend(parsed.operands.iter().skip(1).cloned());
    // The name of a procedure is the label of the line: `start proc far`.
    let label = if word.eq_ignore_ascii_case("PROC") && parsed.label.is_none() {
        Some(parsed.mnemonic.trim_end_matches(':').to_string())
    } else {
        parsed.label.clone()
    };
    line::Line {
        prefixes: parsed.prefixes.clone(),
        label,
        mnemonic: word.to_string(),
        operands,
        qualifier: parsed.qualifier.clone(),
    }
}

/// Put a name and its value back together: `SIZE EQU 10`.
///
/// The value may be more than one number - `new_line equ 0Dh,0Ah,'$'` -
/// and it may need an address - `size = ($ - x1) / 2` - which is why the
/// names and the address come with it.
fn equ(
    parsed: &line::Line,
    constants: &[(String, Vec<i64>)],
    names: &[(String, u16)],
    address: u16,
) -> Option<(String, Vec<i64>)> {
    // The name first: `SIZE EQU 10`, where the name stands where the
    // instruction goes.
    if let Some(value) = parsed.operands.first().and_then(|first| {
        let upper = first.trim().to_ascii_uppercase();
        let value = upper.strip_prefix("EQU")?.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    }) {
        let name = parsed.mnemonic.trim_end_matches(':').to_string();
        if !name.is_empty() {
            let values = values_of(&value, constants, names, address)?;
            return Some((name, values));
        }
    }
    // The name is the label and the directive is the instruction:
    // `count: equ 10`, and `precision = 30`, which the reference writes
    // with an equals sign instead of the word.
    if parsed.mnemonic.eq_ignore_ascii_case("EQU") || parsed.mnemonic == "=" {
        if let Some(name) = &parsed.label {
            let values = values_of_operands(&parsed.operands, constants, names, address)?;
            if !values.is_empty() {
                return Some((name.clone(), values));
            }
        }
    }
    // The directive first: `EQU SIZE, 10`.
    if parsed.mnemonic.eq_ignore_ascii_case("EQU") {
        let name = parsed
            .operands
            .first()?
            .trim()
            .trim_end_matches(',')
            .to_string();
        let value = parsed.operands.get(1)?.trim();
        if !name.is_empty() && !value.is_empty() {
            return Some((name, values_of(value, constants, names, address)?));
        }
    }
    None
}

/// The values a constant holds.
fn values_of_operands(
    operands: &[String],
    constants: &[(String, Vec<i64>)],
    names: &[(String, u16)],
    address: u16,
) -> Option<Vec<i64>> {
    let mut values = Vec::new();
    for text in operands {
        values.extend(values_of(text, constants, names, address)?);
    }
    Some(values)
}

/// The values one operand of a constant holds.
fn values_of(
    text: &str,
    constants: &[(String, Vec<i64>)],
    names: &[(String, u16)],
    address: u16,
) -> Option<Vec<i64>> {
    let trimmed = text.trim();
    // A string is a run of bytes.
    if let Some(string) = string_of(trimmed) {
        return Some(string.bytes().map(i64::from).collect());
    }
    // A name the program knows.
    if let Some((_, values)) = constants
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(trimmed))
    {
        return Some(values.clone());
    }
    if let Some(value) = expression(trimmed, constants, names, address) {
        return Some(vec![value]);
    }
    literals::parse(trimmed).ok().map(|value| vec![value])
}

/// The characters of a string the source wrote, if it wrote one.
fn string_of(text: &str) -> Option<String> {
    let trimmed = text.trim();
    for quote in ['"', '\''] {
        if trimmed.len() >= 2 && trimmed.starts_with(quote) && trimmed.ends_with(quote) {
            let inner = &trimmed[1..trimmed.len() - 1];
            // A single character between the quotes is a character, and a
            // longer run is a string.
            if inner.chars().count() > 1 {
                return Some(inner.to_string());
            }
        }
    }
    None
}

/// The operands of a directive, with the program's own names in them.
fn fold_directive(
    operands: &[String],
    constants: &[(String, Vec<i64>)],
    names: &[(String, u16)],
    address: u16,
) -> Vec<String> {
    let mut out = Vec::new();
    for text in operands {
        let trimmed = text.trim();
        // A constant that holds more than one number stands for all of
        // them: `db new_line`.
        if let Some((_, values)) = constants
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(trimmed))
        {
            if values.len() > 1 {
                for value in values {
                    out.push(value.to_string());
                }
                continue;
            }
        }
        let folded = fold(trimmed, constants, names, address);
        out.push(fold_inside(&folded, constants));
    }
    out
}

/// Replace the operands that name a constant or a label with its value.
fn fold(
    text: &str,
    constants: &[(String, Vec<i64>)],
    names: &[(String, u16)],
    address: u16,
) -> String {
    if let Some(value) = constant(text, constants) {
        return value.to_string();
    }
    // An operand that carries arithmetic: `len-1`, `100+w`, `($ - x1) / 2`.
    if let Some(value) = expression(text, constants, names, address) {
        return value.to_string();
    }
    text.to_string()
}

/// The value of an expression, as the program's names and constants give it.
///
/// The reference's own examples carry arithmetic in a constant: `size =
/// ($ - x1) / 2`. The `$` is the place counter, and the names are the
/// program's own.
fn expression(
    text: &str,
    constants: &[(String, Vec<i64>)],
    names: &[(String, u16)],
    address: u16,
) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let upper = text.to_ascii_uppercase();
    // A name on its own is a name, not an expression: the program decides
    // what a name means, and it is not always a value. The place counter
    // and the words `offset` and `seg` are values on their own.
    let keyword = upper == "$"
        || upper.starts_with("OFFSET ")
        || upper.starts_with("SEG ")
        || upper.contains("<<")
        || upper.contains(">>");
    if !keyword && !text.contains(['+', '*', '/', '%']) && !is_arithmetic(text) {
        return None;
    }
    let mut parser = Expression {
        text,
        at: 0,
        constants,
        names,
        address,
    };
    let value = parser.sum()?;
    parser.spaces();
    if parser.at != parser.text.len() {
        return None;
    }
    Some(value)
}

/// Whether the text is a subtraction or a negative number, which the
/// expression needs to tell apart from a name.
fn is_arithmetic(text: &str) -> bool {
    let text = text.trim();
    match text.split_once('-') {
        Some((left, right)) => {
            !left.trim().is_empty()
                && !right.trim().is_empty()
                && !right.trim().starts_with('-')
                && !text.starts_with('-')
        }
        None => false,
    }
}

/// A little expression reader: the arithmetic the reference's own
/// constants carry.
struct Expression<'a> {
    /// The text being read.
    text: &'a str,
    /// How far it has been read.
    at: usize,
    /// The constants the program declared.
    constants: &'a [(String, Vec<i64>)],
    /// The names the program declared.
    names: &'a [(String, u16)],
    /// The place counter.
    address: u16,
}

impl Expression<'_> {
    /// Skip the spaces.
    fn spaces(&mut self) {
        while self.at < self.text.len() && self.text[self.at..].starts_with(char::is_whitespace) {
            self.at += 1;
        }
    }

    /// The value of a sum.
    fn sum(&mut self) -> Option<i64> {
        let mut value = self.shift()?;
        loop {
            self.spaces();
            let Some(sign) = self.text[self.at..].chars().next() else {
                return Some(value);
            };
            if sign != '+' && sign != '-' {
                return Some(value);
            }
            self.at += 1;
            let right = self.shift()?;
            value = if sign == '+' {
                value + right
            } else {
                value - right
            };
        }
    }

    /// The value of a shift.
    fn shift(&mut self) -> Option<i64> {
        let mut value = self.product()?;
        loop {
            self.spaces();
            let Some(shift) = self.text[self.at..].get(..2) else {
                return Some(value);
            };
            if shift != "<<" && shift != ">>" {
                return Some(value);
            }
            self.at += 2;
            let right = self.product()?;
            value = if shift == "<<" {
                value << right
            } else {
                value >> right
            };
        }
    }

    /// The value of a product.
    fn product(&mut self) -> Option<i64> {
        let mut value = self.term()?;
        loop {
            self.spaces();
            let Some(sign) = self.text[self.at..].chars().next() else {
                return Some(value);
            };
            if sign != '*' && sign != '/' && sign != '%' {
                return Some(value);
            }
            self.at += 1;
            let right = self.term()?;
            value = match sign {
                '*' => value * right,
                '/' => {
                    if right == 0 {
                        return None;
                    }
                    value / right
                }
                _ => {
                    if right == 0 {
                        return None;
                    }
                    value % right
                }
            };
        }
    }

    /// The value of a term: a number, a name, the place counter, or
    /// something in brackets.
    fn term(&mut self) -> Option<i64> {
        self.spaces();
        let first = self.text[self.at..].chars().next()?;
        if first == '(' {
            self.at += 1;
            let value = self.sum()?;
            self.spaces();
            if !self.text[self.at..].starts_with(')') {
                return None;
            }
            self.at += 1;
            return Some(value);
        }
        if first == '-' {
            self.at += 1;
            return Some(-self.term()?);
        }
        if first == '$' {
            self.at += 1;
            return Some(i64::from(self.address));
        }
        if first.is_ascii_digit() {
            let start = self.at;
            while self.at < self.text.len()
                && (self.text[self.at..].starts_with(|c: char| c.is_ascii_alphanumeric())
                    || self.text[self.at..].starts_with('_'))
            {
                self.at += 1;
            }
            let text = &self.text[start..self.at];
            return literals::parse(text).ok();
        }
        if first.is_alphabetic() || first == '_' || first == '@' {
            let start = self.at;
            while self.at < self.text.len()
                && (self.text[self.at..].starts_with(|c: char| c.is_alphanumeric())
                    || self.text[self.at..].starts_with('_')
                    // A `LOCAL` name carries the number of its expansion in
                    // it: `s_dcl$68`.
                    || self.text[self.at..].starts_with('$'))
            {
                self.at += 1;
            }
            let name = &self.text[start..self.at];
            // `offset name` is the address of the name, and `seg name` is
            // the segment it is in: the flat program has one segment.
            if name.eq_ignore_ascii_case("offset") {
                self.spaces();
                return self.term();
            }
            if name.eq_ignore_ascii_case("seg") {
                self.spaces();
                self.term()?;
                return Some(0);
            }
            if let Some(value) = constant(name, self.constants) {
                return Some(value);
            }
            return self
                .names
                .iter()
                .find(|(known, _)| known.eq_ignore_ascii_case(name))
                .map(|(_, at)| i64::from(*at));
        }
        None
    }
}

/// The bytes of `PUSHA` and `POPA`.
///
/// The reference's own documentation gives the sequence: push AX, CX, DX,
/// BX, BP, SI and DI, and pop them the other way round. The stack the
/// reference's `PUSHA` leaves behind has the original SP in it as well,
/// which no sequence of pushes can give, so the sequence is what this
/// assembler writes.
fn push_all(mnemonic: &str) -> Option<Vec<u8>> {
    match mnemonic.to_ascii_uppercase().as_str() {
        "PUSHA" => Some(vec![0x50, 0x51, 0x52, 0x53, 0x55, 0x56, 0x57]),
        "POPA" => Some(vec![0x5F, 0x5E, 0x5D, 0x5B, 0x5A, 0x59, 0x58]),
        _ => None,
    }
}

/// Whether an instruction jumps, so that its target is not folded away.
fn is_jump(mnemonic: &str) -> bool {
    let upper = mnemonic.to_ascii_uppercase();
    upper.starts_with('J')
        || upper == "LOOP"
        || upper == "LOOPE"
        || upper == "LOOPZ"
        || upper == "LOOPNE"
        || upper == "LOOPNZ"
        || upper == "CALL"
}

/// The conditional jumps the reference replaces when their target is too
/// far.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Conditional {
    /// The guide's 70h-7Fh row: the opposite instruction and a big jump.
    Short(u8),
    /// `JCXZ`: `OR CX, CX`, `JNZ` and a big jump.
    Jcxz,
}

/// The conditional jump a mnemonic is, when it is one the reference
/// replaces.
///
/// The guide's 70h-7Fh row is the short conditional jump, and `JCXZ` is the
/// one that tests `CX`. The `LOOP` family is not one of them: the
/// reference's table gives it a companion too, but the replacement for
/// `LOOPE` and `LOOPNE` would decrement `CX` twice and test the wrong
/// condition, so it keeps the refusal.
fn conditional(mnemonic: &str) -> Option<Conditional> {
    let opcode = assemble::options_for_pub(mnemonic)
        .into_iter()
        .find(|(_, form, _)| form.eq_ignore_ascii_case("short"))
        .map(|(opcode, _, _)| opcode)?;
    match opcode {
        0x70..=0x7F => Some(Conditional::Short(opcode)),
        0xE3 => Some(Conditional::Jcxz),
        _ => None,
    }
}

/// The bytes of a conditional jump whose target is too far, or `None` when
/// the jump is in reach.
///
/// The reference does not refuse the jump: it replaces it with the opposite
/// instruction and a big unconditional jump, so the program keeps running.
fn replacement(
    mnemonic: &str,
    operands: &[Operand],
    address: u16,
) -> Result<Option<Vec<u8>>, Error> {
    let Some(kind) = conditional(mnemonic) else {
        return Ok(None);
    };
    let target = match operands.first() {
        Some(Operand::Relative(target)) | Some(Operand::Immediate(target)) => *target,
        _ => return Ok(None),
    };
    // The short form is two bytes, so the displacement is measured from the
    // end of it.
    let displacement = target - i64::from(address) - 2;
    if (-128..=127).contains(&displacement) {
        return Ok(None);
    }
    Ok(Some(match kind {
        Conditional::Short(opcode) => short_replacement(opcode, target, address),
        Conditional::Jcxz => jcxz_replacement(target, address)?,
    }))
}

/// The replacement for a short conditional jump: the opposite instruction
/// over the big jump that follows it.
///
/// The reference's own words: "short conditional jumps are replaced with
/// the opposite instruction and one normal jump", five bytes instead of
/// two.
fn short_replacement(opcode: u8, target: i64, address: u16) -> Vec<u8> {
    let mut bytes = vec![opcode ^ 1, 0x03];
    bytes.extend_from_slice(&near_jump(target, address + 2));
    bytes
}

/// The replacement for `JCXZ`: `OR CX, CX`, `JNZ` over the big jump.
///
/// The reference's own words: the instruction "is assembled into OR CX, CX
/// and two JMPs".
fn jcxz_replacement(target: i64, address: u16) -> Result<Vec<u8>, Error> {
    let mut bytes = or_cx_cx(address)?;
    bytes.extend_from_slice(&jnz_over(address + 2)?);
    bytes.extend_from_slice(&near_jump(target, address + 4));
    Ok(bytes)
}

/// The bytes of `OR CX, CX`.
///
/// The guide's own table gives the form and the matcher gives the bytes,
/// so they are the ones the guide gives and the other `OR`s in a program
/// agree.
fn or_cx_cx(address: u16) -> Result<Vec<u8>, Error> {
    let line = instruction("OR", &["CX", "CX"]);
    let operands = [
        Operand::Register("CX".to_string()),
        Operand::Register("CX".to_string()),
    ];
    assemble::assemble_parts(&line, &operands, address).map_err(|error| bad(&line, error))
}

/// The bytes of `JNZ` over the three bytes that follow it.
///
/// The jump is at `address`, it is two bytes long, and it skips the three
/// bytes of the big jump that comes after it.
fn jnz_over(address: u16) -> Result<Vec<u8>, Error> {
    let line = instruction("JNZ", &[]);
    let target = Operand::Relative(i64::from(address) + 5);
    assemble::assemble_parts(&line, &[target], address).map_err(|error| bad(&line, error))
}

/// The big unconditional jump: `E9h` and the displacement, measured from
/// the end of the jump itself.
fn near_jump(target: i64, address: u16) -> [u8; 3] {
    let displacement = (target - i64::from(address) - 3) as i16;
    let mut bytes = [0xE9, 0, 0];
    bytes[1..].copy_from_slice(&displacement.to_le_bytes());
    bytes
}

/// A line of the assembler's own making: the mnemonic and the operands.
fn instruction(mnemonic: &str, operands: &[&str]) -> line::Line {
    line::Line {
        prefixes: Vec::new(),
        label: None,
        mnemonic: mnemonic.to_string(),
        operands: operands.iter().map(|operand| operand.to_string()).collect(),
        qualifier: None,
    }
}

/// Replace a name inside a value, as in `len dup(0)`.
fn fold_inside(text: &str, constants: &[(String, Vec<i64>)]) -> String {
    let Some((count, value)) = text.split_once(" dup").or_else(|| text.split_once(" DUP")) else {
        return text.to_string();
    };
    let upper = count.trim().to_ascii_uppercase();
    let folded = constant(&upper, constants).map(|value| value.to_string());
    match folded {
        Some(folded) => format!("{folded} dup{value}"),
        None => text.to_string(),
    }
}

/// The bytes of a data directive's values.
fn bytes_of(values: &[i64], width: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in values {
        let of = value.to_le_bytes();
        bytes.extend_from_slice(&of[..width]);
    }
    bytes
}
