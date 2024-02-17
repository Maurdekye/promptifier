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

    fn choose(self, rng: &mut ThreadRng, guidance: &Option<ChoiceGuidance>) -> Choice {
        match guidance {
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
            Some(guidance) => {
                let mut options: Vec<_> = self.choices.into_iter().chain(once(self.top)).collect();
                match guidance {
                    ChoiceGuidance::Longest | ChoiceGuidance::Shortest => {
                        options.sort_by_key(|c| c.text.len())
                    }
                    ChoiceGuidance::MostLikely | ChoiceGuidance::LeastLikely => {
                        options.sort_by(|a, b| {
                            a.weight
                                .partial_cmp(&b.weight)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                    }
                }
                match guidance {
                    ChoiceGuidance::Longest | ChoiceGuidance::MostLikely => options.pop().unwrap(),
                    ChoiceGuidance::Shortest | ChoiceGuidance::LeastLikely => {
                        options.swap_remove(0)
                    }
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
#[error("'{specifier}': {parse_error}")]
struct ParseWeightError {
    specifier: String,
    index: usize,
    parse_error: ParseWeightErrorKind,
}

#[derive(Clone, Debug, Error)]
enum ParseWeightErrorKind {
    #[error("{0}")]
    FloatParse(#[from] ParseFloatError),
    #[error("Weights cannot be negative")]
    NegativeWeight,
}

fn parse_weight(
    maybe_weighted: &str,
    ignore_invalid_weight_literals: bool,
) -> Result<(&str, f64), ParseWeightError> {
    let Some((text, weight_text)) = maybe_weighted.rsplit_once(":") else {
        return Ok((maybe_weighted, 1.0));
    };
    let maybe_weight = weight_text.parse().map_err(|parse_error| ParseWeightError {
        specifier: weight_text.to_string(),
        index: text.len() + 1,
        parse_error: ParseWeightErrorKind::FloatParse(parse_error),
    });
    match maybe_weight {
        Ok(weight) if weight >= 0.0 => Ok((text, weight)),
        Ok(_) => Err(ParseWeightError {
            specifier: weight_text.to_string(),
            index: text.len() + 1,
            parse_error: ParseWeightErrorKind::NegativeWeight,
        }),
        _ if ignore_invalid_weight_literals => Ok((maybe_weighted, 1.0)),
        Err(err) => Err(err),
    }
}
#[derive(ValueEnum, Clone, Debug, Serialize)]
enum ChoiceGuidance {
    Shortest,
    Longest,
    LeastLikely,
    MostLikely,
}

struct GenerationOptions {
    choice_guidance: Option<ChoiceGuidance>,
    ignore_invalid_weight_literals: bool,
}

fn generate(
    mut prompt: &str,
    rng: &mut ThreadRng,
    options: &GenerationOptions,
) -> Result<String, ParseError> {
    let mut stack = Stack::new();
    let mut global_index = 0;
    let parse_weight_and_apply = |text, stack: &mut Stack, global_index| {
        let (text, weight) = parse_weight(text, options.ignore_invalid_weight_literals)
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
                                .push_str(&frame.choose(rng, &options.choice_guidance).text),
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
        Ok(stack.top.choose(rng, &options.choice_guidance).text)
    }
}

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
#[derive(Parser)]
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
    #[clap(short, long, action)]
    verbose: bool,

    /// Don't save the generated prompts; not very useful without --verbose
    #[clap(short, long, action)]
    dry_run: bool,

    /// Specify a guidance heuristic to use when making choices, overriding random selection
    #[clap(short = 'g', long)]
    choice_guidance: Option<ChoiceGuidance>,

    /// Ignore improperly formatted weights and interpret the full text with a weight of 1.
    /// Useful when combining with emphasis syntax common in diffusion UIs. Does not ignore
    /// errors produced from negative weights.
    #[clap(short = 'e', long, action)]
    ignore_invalid_weight_literals: bool,
}

fn main() {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let Args {
            prompt,
            input_file,
            num,
            out,
            verbose,
            dry_run,
            choice_guidance,
            ignore_invalid_weight_literals,
        } = Args::parse();
        let prompt = match (prompt, input_file) {
            (Some(prompt), _) => prompt,
            (_, Some(file)) => fs::read_to_string(file)?,
            _ => Err("No prompt source specified")?,
        };
        let mut out = (!dry_run).then(|| File::create(out)).transpose()?;
        let mut rng = rand::thread_rng();
        let options = GenerationOptions {
            choice_guidance,
            ignore_invalid_weight_literals,
        };
        for _ in 0..num {
            let prompt = generate(&prompt, &mut rng, &options)?;
            if verbose {
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
