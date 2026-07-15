# tkeep

`tkeep` is a lightweight terminal session keep-alive tool. It keeps a shell session running in the background and lets you re-attach later, without the learning cost of a full multiplexer.

- Persistent shell sessions you can re-attach by name.
- Native terminal scrolling — no pane/window multiplexing.
- Simple subcommands, no keybindings to memorize.
- Terminal output history is replayed on attach.

For pane/window multiplexing, use `tmux` or `zellij`; use `tkeep` when you just want "keep this session alive and come back later".

## Installation

```bash
# From crates.io
cargo install tkeep

# From source
git clone https://github.com/shizhilvren/tkeep.git
cd tkeep && cargo build --release
```

## Quick Start

```bash
tkeep new build          # start a session and attach
# ...close terminal / lose SSH at any time...
tkeep attach build       # re-attach later
tkeep ls                 # list active sessions
```

Every subcommand has a single-letter alias: `n`, `a`, `l`, `w`, `k`.

## Commands

### `tkeep new <NAME>` (alias `n`)

Create a new session and attach to it.

| Option | Description |
| --- | --- |
| `-s, --shell <SHELL>` | Shell binary to run (default: `$SHELL`) |
| `--args <ARGS>...` | Extra args passed to the shell |
| `--history <SIZE>` | History buffer size (default: `10000`) |
| `--no-attach` | Start the server only; don't attach |

```bash
tkeep new work -s zsh
tkeep new logs --history 50000
tkeep new ci --no-attach && tkeep attach ci
```

### `tkeep attach <NAME>` (alias `a`)

Attach to an existing session. `-r/--replay` (default on) replays the history buffer.

### `tkeep ls` (alias `l`)

List active sessions with their uptime, computed from the session's runtime file.

```text
Active sessions:
- build (uptime: 01h 23m 45s)
- work  (uptime: 2d 04h 10m 02s)
```

Prints `No active sessions found.` when nothing is running.

### `tkeep where` (alias `w`)

Report whether the current shell is running inside a tkeep session. Always exits `0`.

```text
$ tkeep where
You are inside tkeep session: work
```

### `tkeep kill <NAME>` (alias `k`)

Terminate a session. Sends `SIGTERM` by default; pass `-f`/`--force` for `SIGKILL`.

```bash
tkeep kill work
tkeep kill stuck-session -f
```

## Runtime Files

Per-session files (`.sock`, `.pid`, `.out`, `.err`) live under a versioned runtime directory — the system runtime directory on Linux, falling back to `$HOME`. Overrides: `TKEEP_CONFIG`, `TKEEP_DATA`. Run `tkeep --version` to see the resolved paths.

## License

MIT
