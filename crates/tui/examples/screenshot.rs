//! A screenshot of the TUI, as an SVG.
//!
//! The TUI is drawn into a ratatui `TestBackend`, and the buffer that
//! comes back is a picture of the screen: every cell says what it draws
//! and how it draws it. This tool turns that picture into an SVG, so the
//! screen can be looked at instead of read out of the code.
//!
//! The command is:
//!
//! ```text
//! cargo run -p emo8067-tui --example screenshot -- <cols> <rows> <out.svg> [program.asm]
//! ```
//!
//! The program is optional: without one the TUI opens empty, and an empty
//! screen is a screen too. With `--text` the grid is printed as text and
//! no SVG is written, which is the same screen read out of the buffer.
//!
//! The SVG is the picture of a terminal: the cells are nine pixels wide
//! and eighteen high, the text is Nimbus Mono PS at fifteen pixels, and
//! the colours are the sixteen the terminal knows. `rsvg-convert` turns
//! the SVG into a PNG.

use std::path::PathBuf;
use std::process::ExitCode;

use emo8067_tui::app::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};

/// The usage of the tool, as it is printed when the arguments are wrong.
const USAGE: &str = "\
screenshot — the emo8067 TUI as an SVG

USAGE:
    screenshot <cols> <rows> <out.svg> [program.asm]

OPTIONS:
    --text           print the grid as text instead of writing the SVG
    -h, --help       print this text
";

/// The width of a cell, in pixels.
///
/// The face advances nine pixels at fifteen, so the grid and the text
/// are the same width and every character lands in its own cell.
const CELL_WIDTH: u32 = 9;

/// The height of a cell, in pixels.
const CELL_HEIGHT: u32 = 18;

/// The size of the text, in pixels.
const FONT_SIZE: u32 = 15;

/// How far below the top of its cell the baseline sits.
///
/// The face is fifteen pixels high and the cell is eighteen, so the
/// baseline goes where the box drawing lands in the middle of the cell
/// and the descenders stay inside it.
const BASELINE: u32 = 13;

/// The name the SVG declares the font under.
const FONT_NAME: &str = "emo8067 mono";

/// The family the text is written in.
///
/// The first name is the one the SVG declares for itself, the second is
/// the family that is on this machine, and the last is the generic one
/// for a machine that has neither.
const FONT_FAMILY: &str = "'emo8067 mono', 'Nimbus Mono PS', monospace";

/// The regular face of the font, on this machine.
const FONT_REGULAR: &str = "/usr/share/fonts/gsfonts/NimbusMonoPS-Regular.otf";

/// The bold face of the font, on this machine.
const FONT_BOLD: &str = "/usr/share/fonts/gsfonts/NimbusMonoPS-Bold.otf";

/// The italic face of the font, on this machine.
const FONT_ITALIC: &str = "/usr/share/fonts/gsfonts/NimbusMonoPS-Italic.otf";

/// The bold italic face of the font, on this machine.
const FONT_BOLD_ITALIC: &str = "/usr/share/fonts/gsfonts/NimbusMonoPS-BoldItalic.otf";

/// A colour, as the SVG writes it: red, green and blue.
type Rgb = (u8, u8, u8);

/// The canvas: the background of the terminal the screen is drawn for.
const CANVAS: Rgb = (0x00, 0x00, 0x00);

/// The colour of the text when the cell does not name one: the
/// terminal's own foreground, which is the xterm white.
const FOREGROUND: Rgb = (0xe5, 0xe5, 0xe5);

/// The sixteen named colours, in the order the palette has them.
///
/// These are the xterm values of the standard ANSI colours: the black of
/// the canvas is the black of the palette, and the white of the palette
/// is the foreground a cell gets when it does not name one.
const NAMED: [Rgb; 16] = [
    (0x00, 0x00, 0x00), // black
    (0xcd, 0x00, 0x00), // red
    (0x00, 0xcd, 0x00), // green
    (0xcd, 0xcd, 0x00), // yellow
    (0x00, 0x00, 0xee), // blue
    (0xcd, 0x00, 0xcd), // magenta
    (0x00, 0xcd, 0xcd), // cyan
    (0xe5, 0xe5, 0xe5), // white
    (0x7f, 0x7f, 0x7f), // bright black
    (0xff, 0x00, 0x00), // bright red
    (0x00, 0xff, 0x00), // bright green
    (0xff, 0xff, 0x00), // bright yellow
    (0x5c, 0x5c, 0xff), // bright blue
    (0xff, 0x00, 0xff), // bright magenta
    (0x00, 0xff, 0xff), // bright cyan
    (0xff, 0xff, 0xff), // bright white
];

/// What the command line asks for: the size of the screen, where the
/// picture goes, and the program the TUI opens.
struct Options {
    /// How many columns the screen has.
    cols: u16,
    /// How many rows the screen has.
    rows: u16,
    /// Where the SVG is written.
    out: PathBuf,
    /// The program the TUI opens, when the command line names one.
    program: Option<String>,
    /// Whether the grid is printed as text instead of written as an SVG.
    text: bool,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(refusal) => {
            eprintln!("screenshot: {refusal}");
            ExitCode::from(2)
        }
    }
}

/// The tool, with the arguments it was given.
fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{USAGE}");
        return Ok(());
    }
    let options = parse(args)?;
    let buffer = draw(&options)?;
    if options.text {
        print!("{}", text(&buffer));
        return Ok(());
    }
    let picture = svg(&buffer);
    std::fs::write(&options.out, picture)
        .map_err(|error| format!("{}: {error}", options.out.display()))?;
    let width = u32::from(options.cols) * CELL_WIDTH;
    let height = u32::from(options.rows) * CELL_HEIGHT;
    println!(
        "wrote {} ({width}x{height} pixels, {}x{} cells)",
        options.out.display(),
        options.cols,
        options.rows
    );
    Ok(())
}

/// The command line, read into what the tool needs.
///
/// The flag may come anywhere: the words that are not the flag are the
/// columns, the rows and the file, and the program is the fourth one.
fn parse(args: &[String]) -> Result<Options, String> {
    let mut text = false;
    let mut words: Vec<&str> = Vec::new();
    for arg in args {
        if arg == "--text" {
            text = true;
        } else {
            words.push(arg);
        }
    }
    if words.len() < 3 || words.len() > 4 {
        return Err(USAGE.to_string());
    }
    let cols = count(words[0], "columns")?;
    let rows = count(words[1], "rows")?;
    if cols == 0 || rows == 0 {
        return Err("the screen has no cells".to_string());
    }
    Ok(Options {
        cols,
        rows,
        out: PathBuf::from(words[2]),
        program: words.get(3).map(|word| word.to_string()),
        text,
    })
}

/// A number of cells, read from the command line.
fn count(word: &str, what: &str) -> Result<u16, String> {
    word.parse::<u16>()
        .map_err(|_| format!("`{word}` is not a number of {what}"))
}

/// The screen the TUI draws, as the buffer of cells it is made of.
///
/// The buffer is what a terminal would show: every cell says what it
/// draws and how it draws it, and the tool reads it back.
fn draw(options: &Options) -> Result<Buffer, String> {
    let app = App::new(options.program.clone())?;
    let backend = TestBackend::new(options.cols, options.rows);
    // The test backend cannot fail: its error is the uninhabited type,
    // so there is nothing of it to report.
    let mut terminal = Terminal::new(backend).expect("the backend");
    terminal
        .draw(|frame| emo8067_tui::ui::draw(&app, frame))
        .expect("the drawing");
    Ok(terminal.backend().buffer().clone())
}

/// The cells of the screen, in the order they are drawn.
fn cells(buffer: &Buffer) -> impl Iterator<Item = (u16, u16, &Cell)> {
    let area = buffer.area;
    (0..area.height)
        .flat_map(move |y| (0..area.width).map(move |x| (x, y)))
        .filter_map(move |(x, y)| buffer.cell((x, y)).map(|cell| (x, y, cell)))
}

/// The screen as an SVG: the picture a terminal would show.
///
/// The grid is the canvas: a rectangle for the whole screen, and a
/// rectangle for every cell whose background is not the canvas. The text
/// is one element per cell, in the cell's own column and row, in the
/// colour the cell names.
fn svg(buffer: &Buffer) -> String {
    let area = buffer.area;
    let width = u32::from(area.width) * CELL_WIDTH;
    let height = u32::from(area.height) * CELL_HEIGHT;
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"
    ));
    out.push_str(&faces());
    out.push_str(&format!(
        "  <rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"{}\"/>\n",
        paint(CANVAS)
    ));
    out.push_str(&grounds(buffer));
    out.push_str(&texts(buffer));
    out.push_str(&lines(buffer));
    out.push_str("</svg>\n");
    out
}

/// The font the grid is written in, and the faces of it.
///
/// The four faces are the ones that are on this machine: Nimbus Mono PS
/// of the URW set, in the regular, bold, italic and bold italic faces.
/// The declaration names the files, so a renderer that reads it uses
/// exactly these; a renderer that reads no declaration — `rsvg-convert`
/// among them — finds the family by its name, and the family is
/// installed.
fn faces() -> String {
    let mut out = String::new();
    out.push_str("  <style>\n");
    for (weight, style, file) in [
        ("normal", "normal", FONT_REGULAR),
        ("bold", "normal", FONT_BOLD),
        ("normal", "italic", FONT_ITALIC),
        ("bold", "italic", FONT_BOLD_ITALIC),
    ] {
        out.push_str(&format!(
            "    @font-face {{ font-family: '{FONT_NAME}'; src: url('file://{file}'); font-weight: {weight}; font-style: {style}; }}\n"
        ));
    }
    out.push_str("  </style>\n");
    out
}

/// The backgrounds of the cells that are not the canvas.
///
/// A cell whose background is the canvas leaves the canvas alone: the
/// rectangle would only draw the canvas over itself.
fn grounds(buffer: &Buffer) -> String {
    let mut out = String::new();
    for (x, y, cell) in cells(buffer) {
        let (_, ground) = colours(cell);
        if ground == CANVAS {
            continue;
        }
        out.push_str(&format!(
            "  <rect x=\"{}\" y=\"{}\" width=\"{CELL_WIDTH}\" height=\"{CELL_HEIGHT}\" fill=\"{}\"/>\n",
            u32::from(x) * CELL_WIDTH,
            u32::from(y) * CELL_HEIGHT,
            paint(ground)
        ));
    }
    out
}

/// The text of the screen: one element per cell that draws a character.
///
/// The element sits in the cell's own column and row: the x is the
/// column and the y is the baseline of the row, so the characters land
/// in the cells they belong to.
fn texts(buffer: &Buffer) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  <g font-family=\"{FONT_FAMILY}\" font-size=\"{FONT_SIZE}\" xml:space=\"preserve\">\n"
    ));
    for (x, y, cell) in cells(buffer) {
        let symbol = cell.symbol();
        if symbol == " " {
            continue;
        }
        let (ink, _) = colours(cell);
        out.push_str(&format!(
            "    <text x=\"{}\" y=\"{}\" fill=\"{}\"{}>{}</text>\n",
            u32::from(x) * CELL_WIDTH,
            u32::from(y) * CELL_HEIGHT + BASELINE,
            paint(ink),
            face(cell),
            escape(symbol)
        ));
    }
    out.push_str("  </g>\n");
    out
}

/// The lines the modifiers draw: the underline and the crossed out.
///
/// The underline is under the descenders and the crossed out is through
/// the middle of the letters, the way a terminal draws them.
fn lines(buffer: &Buffer) -> String {
    let mut out = String::new();
    out.push_str("  <g fill=\"none\" stroke-width=\"1\">\n");
    for (x, y, cell) in cells(buffer) {
        let (ink, _) = colours(cell);
        let left = u32::from(x) * CELL_WIDTH;
        let top = u32::from(y) * CELL_HEIGHT;
        if cell.modifier.contains(Modifier::UNDERLINED) {
            let under = top + BASELINE + 3;
            out.push_str(&format!(
                "    <line x1=\"{left}\" y1=\"{under}\" x2=\"{}\" y2=\"{under}\" stroke=\"{}\"/>\n",
                left + CELL_WIDTH,
                paint(ink)
            ));
        }
        if cell.modifier.contains(Modifier::CROSSED_OUT) {
            let through = top + BASELINE - 3;
            out.push_str(&format!(
                "    <line x1=\"{left}\" y1=\"{through}\" x2=\"{}\" y2=\"{through}\" stroke=\"{}\"/>\n",
                left + CELL_WIDTH,
                paint(ink)
            ));
        }
    }
    out.push_str("  </g>\n");
    out
}

/// The screen as text: one line per row of cells.
///
/// The row is what the drawing is made of, and the spaces at the end of
/// it are left out: they say nothing about the screen.
fn text(buffer: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buffer.area.height {
        let row: String = (0..buffer.area.width)
            .filter_map(|x| buffer.cell((x, y)))
            .map(|cell| cell.symbol())
            .collect();
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}

/// The colours of a cell: the text and the ground under it.
///
/// The reversed modifier swaps them, and the dim modifier takes the text
/// halfway towards the ground it is on.
fn colours(cell: &Cell) -> (Rgb, Rgb) {
    let mut ink = colour_of(cell.fg, FOREGROUND);
    let mut ground = colour_of(cell.bg, CANVAS);
    if cell.modifier.contains(Modifier::REVERSED) {
        std::mem::swap(&mut ink, &mut ground);
    }
    if cell.modifier.contains(Modifier::DIM) {
        ink = dimmed(ink, ground);
    }
    (ink, ground)
}

/// The colour a cell names, as the SVG writes it.
///
/// A named colour is the one of the palette, an RGB colour is the one it
/// is given, and a reset is the colour the role falls back to: the
/// foreground of the terminal for the text, the canvas for the ground.
fn colour_of(colour: Color, fallback: Rgb) -> Rgb {
    match colour {
        Color::Reset => fallback,
        Color::Black => NAMED[0],
        Color::Red => NAMED[1],
        Color::Green => NAMED[2],
        Color::Yellow => NAMED[3],
        Color::Blue => NAMED[4],
        Color::Magenta => NAMED[5],
        Color::Cyan => NAMED[6],
        Color::Gray => NAMED[7],
        Color::DarkGray => NAMED[8],
        Color::LightRed => NAMED[9],
        Color::LightGreen => NAMED[10],
        Color::LightYellow => NAMED[11],
        Color::LightBlue => NAMED[12],
        Color::LightMagenta => NAMED[13],
        Color::LightCyan => NAMED[14],
        Color::White => NAMED[15],
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Indexed(index) => indexed(index),
    }
}

/// The colour of the 256-colour palette, at an index.
///
/// The first sixteen are the named ones, the next are a cube of six
/// shades each, and the last are the greys.
fn indexed(index: u8) -> Rgb {
    match index {
        0..=15 => NAMED[usize::from(index)],
        16..=231 => {
            let level = |shade: u8| match shade {
                0 => 0,
                1 => 95,
                2 => 135,
                3 => 175,
                4 => 215,
                _ => 255,
            };
            let cube = index - 16;
            (level(cube / 36), level((cube / 6) % 6), level(cube % 6))
        }
        _ => {
            let grey = (8 + 10 * u16::from(index - 232)) as u8;
            (grey, grey, grey)
        }
    }
}

/// A colour halfway towards the ground it is on.
///
/// The dim modifier takes the colour of the text down, the way a
/// terminal does it: the text goes towards the background it is drawn
/// on, and not towards black, so a dim text on a coloured ground stays
/// readable.
fn dimmed(colour: Rgb, ground: Rgb) -> Rgb {
    // The mix is done in a wider number: two bright colours added
    // together do not fit in a byte.
    let mix = |one: u8, other: u8| ((u16::from(one) + u16::from(other)) / 2) as u8;
    (
        mix(colour.0, ground.0),
        mix(colour.1, ground.1),
        mix(colour.2, ground.2),
    )
}

/// The face the text of a cell is drawn in: the bold one, the italic
/// one, or both.
fn face(cell: &Cell) -> &'static str {
    let bold = cell.modifier.contains(Modifier::BOLD);
    let italic = cell.modifier.contains(Modifier::ITALIC);
    match (bold, italic) {
        (true, true) => " font-weight=\"bold\" font-style=\"italic\"",
        (true, false) => " font-weight=\"bold\"",
        (false, true) => " font-style=\"italic\"",
        (false, false) => "",
    }
}

/// The colour, as the SVG writes it: six hexadecimal digits.
fn paint(colour: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.0, colour.1, colour.2)
}

/// The text of a cell, with the characters XML reserves escaped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}
