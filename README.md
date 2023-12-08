Uidb
----

Uidb is a TUI debugger for [Uiua](https://github.com/uiua-lang/uiua).

# Installation
## From source
1. Clone the repo:
```sh
git clone https://github.com/Froloket64/uidb.git --depth 1
cd uidb
```
2. Compile the source code using `cargo`:
```sh
cargo build --release
```
The binary path is `target/release/uidb`.

If you want to just run the program immediately after compilation, you can use
```sh
cargo run
```

# Usage
## Invocation
Run Uidb like so:
```sh
$ uidb <file>
```
where `<file>` is a path to the file containing Uiua code you want to debug.

For more options, see
```sh
$ uidb --help
```
## Interface
To step forward, use <kbd>h</kbd>.
To step backward, use <kbd>l</kbd>.
