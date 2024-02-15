use clap::Parser;
use rand::prelude::*;
use std::io::Write;
use std::{fs::File, path::PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Error)]
enum ParseError {
    #[error("Unexpected closing brace at char {0}")]
    UnexpectedClosingBrace(usize),
    #[error("Unclosed open brace at char {0}")]
    UnclosedBrace(usize),
    #[error("Stack unexpectedly empty? (should not happen)")]
    StackEmptyError,
}

fn generate(prompt: &str, rng: &mut ThreadRng) -> Result<String, ParseError> {
    let mut stack = vec![(0, vec![String::new()])];
    for (i, char) in prompt.chars().enumerate() {
        match char {
            '|' => stack
                .last_mut()
                .ok_or(ParseError::StackEmptyError)?
                .1
                .push(String::new()),
            '{' => stack.push((i, vec![String::new()])),
            '}' => match (stack.pop(), stack.last_mut()) {
                (None, _) | (_, None) => return Err(ParseError::UnexpectedClosingBrace(i)),
                (Some((_, frame)), Some((_, parent_frame))) => {
                    if let (Some(choice), Some(current_string)) =
                        (frame.choose(rng), parent_frame.last_mut())
                    {
                        current_string.push_str(choice);
                    }
                }
            },
            c => {
                let frame = &mut stack.last_mut().ok_or(ParseError::StackEmptyError)?.1;
                match frame.last_mut() {
                    Some(string) => string.push(c),
                    None => frame.push(String::from(c)),
                }
            }
        }
    }
    match stack.pop() {
        None => Err(ParseError::StackEmptyError),
        Some((i, mut frame)) => {
            if !stack.is_empty() {
                Err(ParseError::UnclosedBrace(i))
            } else {
                let random: usize = rng.gen();
                Ok(frame.swap_remove(random % frame.len()))
            }
        }
    }
}

#[derive(Parser)]
struct Args {
    /// Source prompt to parse
    prompt: String,

    /// Number of prompts to generate
    #[clap(short, long, default_value_t = 1)]
    num: usize,

    /// Output file
    #[clap(short, long, default_value = "prompts.txt")]
    out: PathBuf,

    /// Print generated prompts to console
    #[clap(short, long, action, default_value_t=false)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut out = File::create(args.out)?;
    let mut rng = rand::thread_rng();
    for _ in 0..args.num {
        let prompt = generate(&args.prompt, &mut rng)?;
        if args.verbose {
            println!("{prompt}");
        }
        writeln!(out, "{prompt}")?;
    }
    Ok(())
}
