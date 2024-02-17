# Promptifier

Simple utility for generating prompts from a random template.

Prompts in the form `a random {prompt|word}` choose a random word from the curly
braces to select, separated by the pipes. The above could generate `a random prompt` or
`a random word`.

Curly braces can be nested: `this {{large |}cake|{loud|tiny} boat} is not very nice`
can generate `this cake is not very nice`, `this loud boat is not very nice`,
`this large cake is not very nice`, etc.

Choices may also be weighted: `{ball:1|box:3}` is 3x as likely to generate `box` as it is
to generate `ball`. Weights can be any positive integer or decimal value.

```
Usage: promptifier.exe [OPTIONS] [PROMPT]

Arguments:
  [PROMPT]  Source prompt to parse

Options:
  -i, --input-file <INPUT_FILE>
          File to take source prompt from
  -n, --num <NUM>
          Number of prompts to generate [default: 1]
  -o, --out <OUT>
          Output file [default: prompts.txt]
  -v, --verbose
          Print generated prompts to console
  -d, --dry-run
          Don't save the generated prompts; not very useful without --verbose
  -g, --choice-guidance <CHOICE_GUIDANCE>
          Specify a guidance heuristic to use when making choices, overriding random selection [possible values: shortest, longest, least-likely, most-likely]
  -e, --ignore-invalid-weight-literals
          Ignore improperly formatted weights and interpret the full text, including the malformed weight specifier, as a choice with a weight of 1. Useful when combining with emphasis syntax common in diffusion UIs. Does not ignore errors produced from negative weights
  -h, --help
          Print help
```