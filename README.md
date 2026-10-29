# mapfmt

> [!NOTE]
> This project is AI-assisted. Claude (Opus 5.5) wrote most of it in Claude
> Code, and I made my own changes on top. It's a personal project with no
> guarantees. Issues and PRs are welcome.

Reindents MapServer [Mapfiles](https://mapserver.org/mapfile/). It uses
[tree-sitter-mapfile](https://github.com/lnus/tree-sitter-mapfile) and its
indent rules, the same ones the Helix and Neovim queries use.

It only changes whitespace: each line gets the indent of its block depth,
trailing whitespace is stripped, and everything else stays as it is (line
breaks, spacing inside a line, comments, keyword case, encoding, CRLF).
Multi-line strings and comments are left alone. Unlike
[mappyfile](https://github.com/geographika/mappyfile)'s formatter, it doesn't
validate. A file that doesn't parse is left unchanged and reported, unless you
pass `--lenient`: then it's reindented anyway, using tree-sitter's error
recovery, and the error is a warning. That suits template files with
placeholders the grammar doesn't know, like `DATA $setlater`. Only whitespace
ever changes, but the indentation near the error may be off.

## Usage

```sh
mapfmt < in.map > out.map     # stdin to stdout
mapfmt *.map                  # reindent files in place
mapfmt --check *.map          # list files that would change
mapfmt --lenient *.map        # reindent files with parse errors too
mapfmt --indent 2 file.map    # spaces per level (default 4)
```

Exit codes: `0` success, `1` `--check` found files to change, `2` a parse or
I/O error.

## Install

With Cargo (needs a C compiler for the grammar):

```sh
cargo install --git https://github.com/lnus/mapfmt
```

With Nix:

```sh
nix run github:lnus/mapfmt -- --help
```

or add `packages.<system>.default` from the flake to your config.

## Development

`nix develop` (or `direnv allow`) gives a Rust toolchain. Then:

```sh
cargo test
cargo clippy --all-targets
```

To try a grammar change before it's tagged, point Cargo at a local checkout
with a `[patch."https://github.com/lnus/tree-sitter-mapfile"]` entry. Before
committing, remove it and update the tag in `Cargo.toml` and the hash in
`flake.nix`.
