use clap::Parser;
use rand::prelude::*;
use std::io::Write;
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

    fn choose(self, rng: &mut ThreadRng) -> Choice {
        let weight_sum = self.choices.iter().map(|c| c.weight).sum::<f64>() + self.top.weight;
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

fn generate(mut prompt: &str, rng: &mut ThreadRng) -> Result<String, ParseError> {
    let mut stack = Stack::new();
    let mut global_index = 0;
    let parse_weight = |text, global_index| {
        parse_weight(text)
            .map_err(|err| ParseError::InvalidWeightSpecifier(global_index + err.index, err))
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
                        let (pre, weight) = parse_weight(pre, global_index)?;
                        stack.top.top.text.push_str(pre);
                        stack.top.top.weight = weight;
                        stack.top.push(Choice::new());
                    }
                    Some('{') => {
                        stack.top.top.text.push_str(pre);
                        stack.push(global_index);
                    }
                    Some('}') => {
                        let (pre, weight) = parse_weight(pre, global_index)?;
                        stack.top.top.text.push_str(pre);
                        stack.top.top.weight = weight;
                        match stack.pop() {
                            None => return Err(ParseError::UnexpectedClosingBrace(global_index)),
                            Some(frame) => stack.top.top.text.push_str(&frame.choose(rng).text),
                        }
                    }
                    _ => unreachable!(),
                }
                prompt = &post[1..];
            }
        }
    }
    let (prompt, weight) = parse_weight(prompt, global_index)?;
    stack.top.top.text.push_str(prompt);
    stack.top.top.weight = weight;
    if !stack.stack.is_empty() {
        Err(ParseError::UnclosedBrace(stack.top.start_index))
    } else {
        Ok(stack.top.choose(rng).text)
    }
}

#[derive(Parser)]
/// Handy tool for generating prompts from a random template
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
    prompt: String,

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
}

fn main() {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let args = Args::parse();
        let mut out = (!args.dry_run)
            .then(|| File::create(args.out))
            .transpose()?;
        let mut rng = rand::thread_rng();
        for _ in 0..args.num {
            let prompt = generate(&args.prompt, &mut rng)?;
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
