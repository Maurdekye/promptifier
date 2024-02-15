# Promptifier

Simple utility for generating prompts from a random template.

Prompts in the form `a random {prompt|word}` choose a random word from the curly
braces to select, separated by the pipes. The above could generate `a random prompt` or
`a random word`.

Curly braces can be nested: `this {{large |}cake|{loud|tiny} boat} is not very nice`
can generate `this cake is not very nice`, `this loud boat is not very nice`,
`this large cake is not very nice`, etc.

Choices may also be weighted: `{ball:1|box:3}` is 3x as likely to generate `box` as it is
to generate `ball`.

```
Usage: promptifier.exe [OPTIONS] <PROMPT>

Arguments:
  <PROMPT>  Source prompt to parse

Options:
  -n, --num <NUM>  Number of prompts to generate [default: 1]
  -o, --out <OUT>  Output file [default: prompts.txt]
  -v, --verbose    Print generated prompts to console
  -d, --dry-run    Don't save the generated prompts; not very useful without --verbose
  -h, --help       Print help (see more with '--help')
```