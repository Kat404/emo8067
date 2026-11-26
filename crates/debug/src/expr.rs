//! The expression evaluator.
//!
//! The reference's own window, "stop on condition", takes an expression and
//! stops the machine when it comes out true. Its documentation
//! (`re/docs-txt/stop_on_condition.txt`) gives the language:
//!
//! ```text
//!  expression can be decimal, binary or hexadecimal, for example:
//!   5
//!   101b
//!   34h
//!  h suffix must be added to any a hexadecimal values in the expression
//!  boolean logic and mathematical operators can be used, for example:
//!   1234h + 1
//!   0FFh - 101b
//!   5 << 2
//! ~ not (inverts all bits).
//! * multiply.
//! / divide.
//! % modulus.
//! + sum.
//! - subtract (and unary -).
//! << shift left.
//! >> shift right.
//! & bitwise AND.
//! ^ bitwise XOR.
//! | bitwise OR.
//!  offset and segment values (for memory byte / word) are always in hex.
//!  no suffix is requred.
//! ```
//!
//! The evaluator itself is not in the Visual Basic program: it is in
//! `diasm.dll`, the DLL that also holds the disassembler, and its five
//! refusals are the reference's words (`re/out/diasm/strings.tsv`):
//!
//! - `expression too complex`
//! - `bad operator in stack!`
//! - `bad syntax`
//! - `missing an operand`
//! - `bad expression`
//!
//! The reference's own expression has no comparison in it, so a breakpoint's
//! condition cannot be written in it; the comparisons and the logical
//! operators this module adds are ours, and they are marked as such.

use std::fmt;

/// A value the evaluator computes with.
///
/// The reference's evaluator works in 32-bit **signed** words: its own
/// operator stack holds `int`, and its division, its modulus and its right
/// shift are the signed ones. Its registers and its memory are 16-bit and
/// reach it zero-extended, so a register is never negative - only an
/// expression can make a negative one.
pub type Value = i32;

/// What the evaluator reads while it computes.
///
/// The two things an expression can name are a register and a word of
/// memory, and the machine behind the expression decides what they are.
pub trait Context {
    /// The value of a register, by the name the source wrote: `AX`, `AL`,
    /// `CS`.
    fn register(&self, name: &str) -> Option<Value>;

    /// The word at a logical address.
    fn word(&self, segment: u16, offset: u16) -> Option<Value>;
}

/// What went wrong.
///
/// The five refusals are the reference's own, word for word.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The expression is longer than the evaluator's stack.
    TooComplex,
    /// The operator is one the evaluator does not have.
    BadOperator,
    /// The text does not parse.
    BadSyntax,
    /// An operator has no operand.
    MissingOperand,
    /// The expression does not compute.
    BadExpression,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::TooComplex => "expression too complex",
            Error::BadOperator => "bad operator in stack!",
            Error::BadSyntax => "bad syntax",
            Error::MissingOperand => "missing an operand",
            Error::BadExpression => "bad expression",
        })
    }
}

/// How deep an expression may nest.
///
/// The reference's own limit is the size of its operator stack, which the
/// strings do not say; this one is ours.
const MAX_DEPTH: usize = 64;

/// The registers an expression can name.
///
/// The reference's own list (`re/FINDINGS.md`): the thirteen sixteen-bit
/// registers and the eight byte registers.
const REGISTERS: [&str; 21] = [
    "AX", "BX", "CX", "DX", "SP", "BP", "SI", "DI", "CS", "DS", "ES", "SS", "IP", "AH", "AL", "BH",
    "BL", "CH", "CL", "DH", "DL",
];

/// An operator, and what binds tighter than what.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Not,
    Mul,
    Div,
    Mod,
    Add,
    /// The `-`. It is a prefix on its own and a sum between two values, and
    /// the position in the expression says which.
    Minus,
    Shl,
    Shr,
    And,
    Xor,
    Or,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    AndAnd,
    OrOr,
}

/// One thing the expression is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Token {
    Number(Value),
    Register(&'static str),
    Memory(u16, u16),
    Operator(Op),
    Open,
    Close,
}

/// The expression, taken apart.
struct Parser<'a, 'c> {
    text: &'a str,
    at: usize,
    depth: usize,
    context: &'c dyn Context,
}

impl<'a, 'c> Parser<'a, 'c> {
    /// A parser over the text, reading from the machine.
    fn new(text: &'a str, context: &'c dyn Context) -> Self {
        Parser {
            text,
            at: 0,
            depth: 0,
            context,
        }
    }

    /// The rest of the text, from where the parser is.
    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    /// Skip the spaces.
    fn spaces(&mut self) {
        let rest = self.rest();
        let spaces = rest.len() - rest.trim_start().len();
        self.at += spaces;
    }

    /// The next token, if there is one.
    fn token(&mut self) -> Result<Option<Token>, Error> {
        self.spaces();
        let rest = self.rest();
        let Some(first) = rest.chars().next() else {
            return Ok(None);
        };
        // A word: a register, a number, or the two halves of a memory
        // reference. The word runs to the next space, and the colon is part
        // of it, because `SSSS:OOOO` is one thing.
        if first.is_ascii_alphanumeric() || first == '_' {
            let word = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == ':')
                .collect::<String>();
            // `SSSS:OOOO`: the two halves are hex and need no suffix.
            if word.contains(':') {
                if let Some((left, right)) = word.split_once(':')
                    && let (Ok(segment), Ok(offset)) = (
                        u16::from_str_radix(left, 16),
                        u16::from_str_radix(right, 16),
                    )
                {
                    self.at += word.len();
                    return Ok(Some(Token::Memory(segment, offset)));
                }
                return Err(Error::BadSyntax);
            }
            let upper = word.to_ascii_uppercase();
            // A register comes first: `AH` ends in a `H` and `BH` in a `B`,
            // and the 8086 has both.
            if let Some(name) = REGISTERS.iter().find(|known| **known == upper) {
                self.at += word.len();
                return Ok(Some(Token::Register(name)));
            }
            // A number: decimal, `101b` or `34h`. The `h` suffix is what
            // tells a hexadecimal value from a decimal one, and the reference
            // asks for it: "h suffix must be added to any a hexadecimal
            // values". A hexadecimal value may start with a letter, so the
            // suffix is what tells a number from a register.
            if first.is_ascii_digit()
                || matches!(
                    word.chars().last(),
                    Some('h') | Some('H') | Some('b') | Some('B')
                )
            {
                let (token, taken) = self.number(rest)?;
                self.at += taken;
                return Ok(Some(token));
            }
            return Err(Error::BadExpression);
        }
        // An operator.
        let (op, taken) = match rest {
            _ if rest.starts_with("<<") => (Op::Shl, 2),
            _ if rest.starts_with(">>") => (Op::Shr, 2),
            _ if rest.starts_with("==") => (Op::Equal, 2),
            _ if rest.starts_with("!=") => (Op::NotEqual, 2),
            _ if rest.starts_with("<=") => (Op::LessEqual, 2),
            _ if rest.starts_with(">=") => (Op::GreaterEqual, 2),
            _ if rest.starts_with("&&") => (Op::AndAnd, 2),
            _ if rest.starts_with("||") => (Op::OrOr, 2),
            _ => match first {
                '~' => (Op::Not, 1),
                '-' => (Op::Minus, 1),
                '*' => (Op::Mul, 1),
                '/' => (Op::Div, 1),
                '%' => (Op::Mod, 1),
                '+' => (Op::Add, 1),
                '&' => (Op::And, 1),
                '^' => (Op::Xor, 1),
                '|' => (Op::Or, 1),
                '<' => (Op::Less, 1),
                '>' => (Op::Greater, 1),
                '(' => {
                    self.at += 1;
                    return Ok(Some(Token::Open));
                }
                ')' => {
                    self.at += 1;
                    return Ok(Some(Token::Close));
                }
                // A single `=` or `!` is not an operator this evaluator has:
                // the equality is `==` and the inequality is `!=`.
                '=' | '!' => return Err(Error::BadOperator),
                _ => return Err(Error::BadSyntax),
            },
        };
        self.at += taken;
        Ok(Some(Token::Operator(op)))
    }

    /// A number, and how many characters it took.
    ///
    /// The suffix tells the base: `b` is binary, `h` is hexadecimal, and no
    /// suffix is decimal. The suffix is the **last** character, because the
    /// hexadecimal digits include the letters: `0FFh` is a number whose
    /// digits are `0FF`, and `101b` is a number whose digits are `101`.
    fn number(&self, rest: &'a str) -> Result<(Token, usize), Error> {
        let word = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>();
        if word.is_empty() {
            return Err(Error::BadSyntax);
        }
        // The two ways a hexadecimal value is written: the reference's own
        // `1234h` and the `0x1234` the research's breakpoint examples use.
        let (digits, base) =
            if let Some(rest) = word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
                (rest, 16)
            } else if let Some(rest) = word.strip_prefix("0b").or_else(|| word.strip_prefix("0B")) {
                (rest, 2)
            } else {
                match word.chars().last() {
                    Some('b') | Some('B') => (&word[..word.len() - 1], 2),
                    Some('h') | Some('H') => (&word[..word.len() - 1], 16),
                    _ => (word.as_str(), 10),
                }
            };
        let digits = digits.replace('_', "");
        if digits.is_empty() {
            return Err(Error::BadSyntax);
        }
        let value = u64::from_str_radix(&digits, base).map_err(|_| Error::BadExpression)?;
        Ok((Token::Number(value as Value), word.len()))
    }

    /// The operator that comes next, if the next token is one.
    ///
    /// A refusal is not swallowed: the text that follows the operator is
    /// part of the expression, and a character that cannot be there is the
    /// expression's own refusal.
    fn peek_operator(&mut self) -> Result<Option<Op>, Error> {
        let saved = self.at;
        let token = self.token();
        self.at = saved;
        match token {
            Ok(Some(Token::Operator(op))) => Ok(Some(op)),
            Ok(_) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The whole expression: the loosest operator and everything under it.
    fn expression(&mut self) -> Result<Value, Error> {
        self.or_expression()
    }

    /// `||`
    fn or_expression(&mut self) -> Result<Value, Error> {
        let mut left = self.and_expression()?;
        while let Some(Op::OrOr) = self.peek_operator()? {
            self.token()?;
            let right = self.and_expression()?;
            left = apply(Op::OrOr, left, right);
        }
        Ok(left)
    }

    /// `&&`
    fn and_expression(&mut self) -> Result<Value, Error> {
        let mut left = self.equality()?;
        while let Some(Op::AndAnd) = self.peek_operator()? {
            self.token()?;
            let right = self.equality()?;
            left = apply(Op::AndAnd, left, right);
        }
        Ok(left)
    }

    /// `==` and `!=`
    fn equality(&mut self) -> Result<Value, Error> {
        let mut left = self.comparison()?;
        while let Some(op @ (Op::Equal | Op::NotEqual)) = self.peek_operator()? {
            self.token()?;
            let right = self.comparison()?;
            left = apply(op, left, right);
        }
        Ok(left)
    }

    /// `<`, `>`, `<=` and `>=`
    fn comparison(&mut self) -> Result<Value, Error> {
        let mut left = self.bit_or()?;
        while let Some(op @ (Op::Less | Op::Greater | Op::LessEqual | Op::GreaterEqual)) =
            self.peek_operator()?
        {
            self.token()?;
            let right = self.bit_or()?;
            left = apply(op, left, right);
        }
        Ok(left)
    }

    /// `|`
    fn bit_or(&mut self) -> Result<Value, Error> {
        let mut left = self.bit_xor()?;
        while let Some(Op::Or) = self.peek_operator()? {
            self.token()?;
            let right = self.bit_xor()?;
            left = apply(Op::Or, left, right);
        }
        Ok(left)
    }

    /// `^`
    fn bit_xor(&mut self) -> Result<Value, Error> {
        let mut left = self.bit_and()?;
        while let Some(Op::Xor) = self.peek_operator()? {
            self.token()?;
            let right = self.bit_and()?;
            left = apply(Op::Xor, left, right);
        }
        Ok(left)
    }

    /// `&`
    fn bit_and(&mut self) -> Result<Value, Error> {
        let mut left = self.shift()?;
        while let Some(Op::And) = self.peek_operator()? {
            self.token()?;
            let right = self.shift()?;
            left = apply(Op::And, left, right);
        }
        Ok(left)
    }

    /// `<<` and `>>`
    fn shift(&mut self) -> Result<Value, Error> {
        let mut left = self.additive()?;
        while let Some(op @ (Op::Shl | Op::Shr)) = self.peek_operator()? {
            self.token()?;
            let right = self.additive()?;
            left = apply(op, left, right);
        }
        Ok(left)
    }

    /// `+` and `-`
    fn additive(&mut self) -> Result<Value, Error> {
        let mut left = self.multiplicative()?;
        while let Some(op @ (Op::Add | Op::Minus)) = self.peek_operator()? {
            self.token()?;
            let right = self.multiplicative()?;
            left = apply(op, left, right);
        }
        Ok(left)
    }

    /// `*`, `/` and `%`
    fn multiplicative(&mut self) -> Result<Value, Error> {
        let mut left = self.unary()?;
        while let Some(op @ (Op::Mul | Op::Div | Op::Mod)) = self.peek_operator()? {
            self.token()?;
            let right = self.unary()?;
            left = apply(op, left, right);
        }
        Ok(left)
    }

    /// `~` and the unary `-`
    fn unary(&mut self) -> Result<Value, Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return Err(Error::TooComplex);
        }
        let result = self.unary_inner();
        self.depth -= 1;
        result
    }

    /// The body of [`Parser::unary`].
    fn unary_inner(&mut self) -> Result<Value, Error> {
        match self.token()? {
            Some(Token::Operator(op @ (Op::Not | Op::Minus))) => {
                let value = self.unary()?;
                Ok(match op {
                    Op::Not => !value,
                    Op::Minus => value.wrapping_neg(),
                    _ => unreachable!(),
                })
            }
            Some(Token::Open) => {
                // An expression that is not there at all is a syntax
                // problem, not a computation that failed.
                let value = self.expression().map_err(|error| match error {
                    Error::BadExpression => Error::BadSyntax,
                    other => other,
                })?;
                match self.token()? {
                    Some(Token::Close) => Ok(value),
                    _ => Err(Error::BadSyntax),
                }
            }
            Some(Token::Number(value)) => Ok(value),
            Some(Token::Register(name)) => self.context.register(name).ok_or(Error::BadExpression),
            Some(Token::Memory(segment, offset)) => self
                .context
                .word(segment, offset)
                .ok_or(Error::BadExpression),
            Some(Token::Operator(_)) | Some(Token::Close) => Err(Error::MissingOperand),
            None => Err(Error::BadExpression),
        }
    }
}

/// Apply an operator to two values.
///
/// The shift masks its count to five bits, which is what the reference's
/// evaluator does with its shift and what the 8086 does with its own.
fn apply(op: Op, left: Value, right: Value) -> Value {
    match op {
        Op::Not => !left,
        Op::Mul => left.wrapping_mul(right),
        Op::Div => left.checked_div(right).unwrap_or(0),
        Op::Mod => left.checked_rem(right).unwrap_or(0),
        Op::Add => left.wrapping_add(right),
        Op::Minus => left.wrapping_sub(right),
        Op::Shl => left.wrapping_shl((right & 0x1F) as u32),
        Op::Shr => left.wrapping_shr((right & 0x1F) as u32),
        Op::And => left & right,
        Op::Xor => left ^ right,
        Op::Or => left | right,
        Op::Equal => i32::from(left == right),
        Op::NotEqual => i32::from(left != right),
        Op::Less => i32::from(left < right),
        Op::Greater => i32::from(left > right),
        Op::LessEqual => i32::from(left <= right),
        Op::GreaterEqual => i32::from(left >= right),
        Op::AndAnd => i32::from(left != 0 && right != 0),
        Op::OrOr => i32::from(left != 0 || right != 0),
    }
}

/// Check that an expression reads, without computing it.
///
/// The registers and the memory are the machine's, and a machine that is
/// not there yet has none of them: the check is about the shape of the
/// expression, which is what a breakpoint's condition needs before the
/// machine reaches it.
pub fn check(text: &str) -> Result<(), Error> {
    struct Nothing;
    impl Context for Nothing {
        fn register(&self, _name: &str) -> Option<Value> {
            Some(0)
        }
        fn word(&self, _segment: u16, _offset: u16) -> Option<Value> {
            Some(0)
        }
    }
    evaluate(text, &Nothing).map(|_| ())
}

/// Evaluate an expression against a machine.
///
/// The expression is the reference's own language: a literal in decimal, in
/// binary (`101b`) or in hexadecimal with its `h` suffix (`34h`), a
/// register, a word of memory written `SSSS:OOOO`, and the operators `~`,
/// `-`, `*`, `/`, `%`, `+`, `<<`, `>>`, `&`, `^` and `|`.
///
/// The comparisons and the logical operators are this project's addition:
/// the reference's expression has none, and a breakpoint's condition needs
/// them.
pub fn evaluate(text: &str, context: &impl Context) -> Result<Value, Error> {
    let mut parser = Parser::new(text, context);
    let value = parser.expression()?;
    parser.spaces();
    if !parser.rest().is_empty() {
        return Err(Error::BadSyntax);
    }
    Ok(value)
}
