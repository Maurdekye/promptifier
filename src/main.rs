use clap::{Parser, ValueEnum};
use rand::prelude::*;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::iter::once;
use std::num::ParseFloatError;
use std::{fs::File, path::PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Error)]
enum ParseError {
    #[error("Unexpected closing brace at char {0}")]
    UnexpectedClosingBrace(usize),
    #[error("Unclosed open brace at char {0}")]
    UnclosedBrace(usize),
    #[error("Invalid weight specifier at char {0}: {1}")]
    InvalidWeightSpecifier(usize, ParseWeightError),
}

#[derive(Clone, Debug)]
struct Choice {
    text: String,
    weight: f64,
}

impl Choice {
    fn new() -> Self {
        Self {
            text: String::new(),
            weight: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Frame {
    start_index: usize,
    choices: Vec<Choice>,
    top: Choice,
}

impl Frame {
    fn new(start_index: usize) -> Self {
        Self {
            start_index,
            choices: Vec::new(),
            top: Choice::new(),
        }
    }

    fn push(&mut self, choice: Choice) {
        self.choices.push(std::mem::replace(&mut self.top, choice));
    }

    fn choose(self, rng: &mut ThreadRng, target_length: &Option<LengthTarget>) -> Choice {
        match target_length {
            None => {
                let weight_sum =
                    self.choices.iter().map(|c| c.weight).sum::<f64>() + self.top.weight;
                let weighted_index: f64 = rng.gen();
                let mut weighted_index = weighted_index * weight_sum;
                for choice in self.choices {
                    if weighted_index <= choice.weight {
                        return choice;
                    }
                    weighted_index -= choice.weight;
                }
                self.top
            }
            Some(target) => {
                let mut options: Vec<_> = self.choices.into_iter().chain(once(self.top)).collect();
                options.sort_by_key(|c| c.text.len());
                match target {
                    LengthTarget::Longest => options.pop().unwrap(),
                    LengthTarget::Shortest => options.swap_remove(0),
                }
            }
        }
    }
}

struct Stack {
    stack: Vec<Frame>,
    top: Frame,
}

impl Stack {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            top: Frame::new(0),
        }
    }

    fn push(&mut self, start_index: usize) {
        self.stack
            .push(std::mem::replace(&mut self.top, Frame::new(start_index)));
    }

    fn pop(&mut self) -> Option<Frame> {
        self.stack
            .pop()
            .map(|frame| std::mem::replace(&mut self.top, frame))
    }
}

#[derive(Clone, Debug, Error)]
#[error("Invalid weight specifier '{specifier}': {parse_error}")]
struct ParseWeightError {
    specifier: String,
    index: usize,
    parse_error: ParseFloatError,
}

fn parse_weight(maybe_weighted: &str) -> Result<(&str, f64), ParseWeightError> {
    let Some((text, weight_text)) = maybe_weighted.split_once(":") else {
        return Ok((maybe_weighted, 1.0));
    };
    let weight: f64 = weight_text
        .parse()
        .map_err(|parse_error| ParseWeightError {
            specifier: weight_text.to_string(),
            index: text.len() + 1,
            parse_error,
        })?;
    Ok((text, weight))
}
#[derive(ValueEnum, Clone, Debug, Serialize)]
enum LengthTarget {
    Shortest,
    Longest,
}

struct GenerationOptions {
    target_length: Option<LengthTarget>,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            target_length: None,
        }
    }
}

fn generate(
    mut prompt: &str,
    rng: &mut ThreadRng,
    options: GenerationOptions,
) -> Result<String, ParseError> {
    let mut stack = Stack::new();
    let mut global_index = 0;
    let parse_weight_and_apply = |text, stack: &mut Stack, global_index| {
        let (text, weight) = parse_weight(text)
            .map_err(|err| ParseError::InvalidWeightSpecifier(global_index + err.index, err))?;
        stack.top.top.text.push_str(text);
        stack.top.top.weight = weight;
        Ok(())
    };
    loop {
        match prompt.find(&['|', '{', '}']) {
            None => break,
            Some(index) => {
                global_index += index;
                let pre = &prompt[..index];
                let post = &prompt[index..];
                match post.chars().next() {
                    None => break,
                    Some('|') => {
                        parse_weight_and_apply(pre, &mut stack, global_index)?;
                        stack.top.push(Choice::new());
                    }
                    Some('{') => {
                        stack.top.top.text.push_str(pre);
                        stack.push(global_index);
                    }
                    Some('}') => {
                        parse_weight_and_apply(pre, &mut stack, global_index)?;
                        match stack.pop() {
                            None => return Err(ParseError::UnexpectedClosingBrace(global_index)),
                            Some(frame) => stack
                                .top
                                .top
                                .text
                                .push_str(&frame.choose(rng, &options.target_length).text),
                        }
                    }
                    _ => unreachable!(),
                }
                prompt = &post[1..];
            }
        }
    }
    parse_weight_and_apply(prompt, &mut stack, global_index)?;
    if !stack.stack.is_empty() {
        Err(ParseError::UnclosedBrace(stack.top.start_index))
    } else {
        Ok(stack.top.choose(rng, &options.target_length).text)
    }
}

#[derive(Parser)]
/// Simple utility for generating prompts from a random template.
///
/// Prompts in the form `a random {prompt|word}` choose a random word from the curly
/// braces to select, separated by the pipes. The above could generate `a random prompt` or
/// `a random word`.
///
/// Curly braces can be nested: `this {{large |}cake|{loud|tiny} boat} is not very nice`
/// can generate `this cake is not very nice`, `this loud boat is not very nice`,
/// `this large cake is not very nice`, etc.
///
/// Choices may also be weighted: `{ball:1|box:3}` is 3x as likely to generate `box` as it is
/// to generate `ball`.
struct Args {
    /// Source prompt to parse
    prompt: Option<String>,

    /// File to take source prompt from
    #[clap(short, long)]
    input_file: Option<PathBuf>,

    /// Number of prompts to generate
    #[clap(short, long, default_value_t = 1)]
    num: usize,

    /// Output file
    #[clap(short, long, default_value = "prompts.txt")]
    out: PathBuf,

    /// Print generated prompts to console
    #[clap(short, long, action, default_value_t = false)]
    verbose: bool,

    /// Don't save the generated prompts; not very useful without --verbose
    #[clap(short, long, action, default_value_t = false)]
    dry_run: bool,

    /// Attempt to generate the longest or shortest possible prompt
    #[clap(short, long)]
    length_target: Option<LengthTarget>,
}

fn main() {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let args = Args::parse();
        let prompt = match (args.prompt, args.input_file) {
            (Some(prompt), _) => prompt,
            (_, Some(file)) => fs::read_to_string(file)?,
            _ => Err("No prompt source specified")?,
        };
        let mut out = (!args.dry_run)
            .then(|| File::create(args.out))
            .transpose()?;
        let mut rng = rand::thread_rng();
        for _ in 0..args.num {
            let prompt = generate(
                &prompt,
                &mut rng,
                GenerationOptions {
                    target_length: args.length_target.clone(),
                },
            )?;
            if args.verbose {
                println!("{prompt}");
            }
            if let Some(out) = &mut out {
                writeln!(out, "{prompt}")?;
            }
        }
        Ok(())
    })();
    if let Err(err) = result {
        eprintln!("{err}");
    }
}
