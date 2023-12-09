use clap::{arg, command};
use crossterm::{
    event::{self, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use lazy_static::lazy_static;
use ratatui::{
    prelude::*,
    widgets::{
        block::{Block, BorderType},
        Borders, Paragraph,
    },
};
use uiua::{
    self,
    ast::{Item, Word},
    CodeSpan, Uiua, Value,
};

use std::{
    fs::File,
    io::{stdout, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

// TODO:
// -? Use `std::cell::LazyCell` instead of `lazy_static!`
// - Use lazy span compilation (per line?)
// - Add editing support using `tui-textarea`
// -? Use history to save previously computed results
// - Split the project into multiple files
// - Remove some of `uiua` crate's features

#[cfg(debug_assertions)]
lazy_static! {
    static ref LOG_FILE: Mutex<File> =
        Mutex::new(File::create(LOG_FILE_NAME).expect("failed to create log file")); // TODO: Handle gently
}

const HIGHLIGHT_STYLE: Style = Style::new().bg(Color::Rgb(80, 73, 69));
const LOG_FILE_NAME: &'static str = "./log.txt";

// I am emotionally bruised
fn prepare_ast(ast: Vec<Item>) -> Vec<CodeSpan> {
    ast.into_iter()
        .filter(|i| match i {
            Item::Words(..) | Item::Binding(..) => true,
            _ => false,
        })
        .flat_map(|i| match i {
            Item::Words(v) => v
                .into_iter()
                .flat_map(|v1| {
                    v1.iter()
                        .filter(|s| match s.value {
                            Word::Spaces => false,
                            _ => true,
                        })
                        // TODO: Remove excessive cloning
                        .map(|i| i.span.clone())
                        .rev()
                        .collect::<Vec<_>>()
                })
                .collect(),
            Item::Binding(b) => vec![b.span().clone()],
            _ => unreachable!(),
        })
        .collect()
}

fn highlight_multiline_token<'a>(src: &'a String, span: &CodeSpan) -> Vec<Line<'a>> {
    src.lines()
        .enumerate()
        .map(|(i, l)| {
            // ~TODO: Use `unlikely`
            if i as u32 >= span.start.line - 1 && i as u32 <= span.end.line - 1 {
                Line::styled(l, HIGHLIGHT_STYLE)
            } else {
                Line::raw(l)
            }
        })
        .collect()
}

fn highlight_token<'a>(src: &'a String, span: &CodeSpan) -> Vec<Line<'a>> {
    let mut char_count = 0_usize;

    src.lines()
        .enumerate()
        .map(|(i, l)| {
            if i as u32 == span.start.line - 1 {
                let pos = char_count;
                char_count += l.len() + 1;

                #[cfg(debug_assertions)]
                let _ = writeln!(
                    LOG_FILE.lock().unwrap(),
                    "{i} {}-{}\t:: {} ({pos})",
                    span.start.byte_pos,
                    span.end.byte_pos,
                    l.len(),
                );

                Line::from(vec![
                    Span::raw(
                        std::str::from_utf8(
                            &l.bytes()
                                .take(span.start.byte_pos as usize - pos)
                                .collect::<Box<[u8]>>(),
                        )
                        .unwrap()
                        .to_string(),
                    ),
                    Span::styled(
                        std::str::from_utf8(
                            &l.bytes()
                                .skip(span.start.byte_pos as usize - pos)
                                .take((span.end.byte_pos - span.start.byte_pos) as usize)
                                .collect::<Box<[u8]>>(),
                        )
                        .unwrap()
                        .to_string(),
                        HIGHLIGHT_STYLE,
                    ),
                    Span::raw(
                        std::str::from_utf8(
                            &l.bytes()
                                .skip(span.end.byte_pos as usize - pos)
                                .take(l.len() - (span.end.byte_pos as usize - pos))
                                .collect::<Box<[u8]>>(),
                        )
                        .unwrap()
                        .to_string(),
                    ),
                ])
            } else {
                char_count += l.len() + 1;

                Line::raw(l)
            }
        })
        .collect()
}

fn main() -> std::io::Result<()> {
    let matches = command!()
        .arg(arg!([file] "Path to file to debug").required(true))
        .get_matches();

    let file: PathBuf;

    match matches.get_one::<String>("file") {
        Some(f) => {
            file = Path::new(f).canonicalize().expect("file not found").into();
        }
        None => unreachable!(),
    }

    let src = std::fs::read_to_string(file.clone()).expect("failed to read file contents");
    let mut uiua = Uiua::with_native_sys();

    let (ast, ..) = uiua::parse(&src, None);

    let spans = prepare_ast(ast);

    let spans_str: Vec<String> = spans
        .clone()
        .into_iter()
        .map(|s| s.as_str().to_owned())
        .collect();

    // // OPTIM: Use history to save resourses when stepping back
    // // let mut history = Vec::<Vec<Value>>::new();

    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let block = Block::default()
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let mut span_i = 0;
    let mut src_highlighted = Text::from(src.clone());
    let mut stack = <Vec<Value>>::new();

    loop {
        let src_pane = Paragraph::new(src_highlighted.clone()).block(block.clone().title("Code"));

        let stack_pane = Paragraph::new(
            stack
                .iter()
                .map(|v| format!("{}", v))
                .collect::<Box<_>>()
                .join("\n"),
        )
        .block(block.clone().title("Stack"));

        terminal.draw(|f| {
            let area = f.size();

            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);

            f.render_widget(src_pane, layout[0]);
            f.render_widget(stack_pane, layout[1]);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let event::Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('h') => {
                            if span_i < spans.len() - 1 {
                                span_i += 1;

                                let span = &spans[span_i];

                                // OPTIM: Reimpl without `mut`s
                                let lines: Vec<Line> = if span.start.line != span.end.line {
                                    highlight_multiline_token(&src, &span)
                                } else {
                                    highlight_token(&src, &span)
                                };

                                src_highlighted = Text::from(lines);

                                uiua.load_str(&spans_str[0..=span_i].join("\n"))
                                    .expect("failed to execute Uiua src_pane");

                                stack = uiua.take_stack();
                            }
                        }
                        KeyCode::Char('l') => {
                            if span_i > 0 {
                                span_i -= 1;

                                let span = &spans[span_i];

                                // OPTIM: Reimpl without `mut`s
                                let lines: Vec<Line> = if span.start.line != span.end.line {
                                    highlight_multiline_token(&src, &span)
                                } else {
                                    highlight_token(&src, &span)
                                };

                                src_highlighted = Text::from(lines);

                                uiua.load_str(&spans_str[0..=span_i].join("\n"))
                                    .expect("failed to execute Uiua src_pane");

                                stack = uiua.take_stack();
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
