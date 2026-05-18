# tkeep

`tkeep` is a terminal session keep-alive tool.

It keeps your shell session alive in the background and lets you re-attach later, while still behaving like a normal terminal experience.

Unlike terminal multiplexers such as `tmux` or `zellij`, `tkeep` does not focus on pane/window multiplexing. It focuses on session persistence and uses native terminal scrolling behavior, so you can keep your usual terminal habits.

You also do not need to memorize keybindings or command modes. This reduces learning cost and makes `tkeep` easier to adopt for users who only want persistent sessions.

## Why tkeep

- Keep long-running shell sessions alive in background.
- Re-attach by session name from another terminal.
- Use native terminal scrolling experience instead of multiplexer-style pane/window management.
- No key mapping memorization required; use simple subcommands only.
- Preserve terminal output history for replay on attach.
- Lightweight command-line workflow.

## Positioning

`tkeep` is not a replacement for full-featured multiplexers.

- Prefer your terminal emulator's native split feature; choose `tmux`/`zellij` only when you need advanced session orchestration.
- If you mainly need "keep this terminal session alive and re-attach later" with minimal cognitive load, `tkeep` is designed for that workflow.

## Installation

### Install from crates.io

```bash
cargo install tkeep
```

### Build from source

```bash
git clone https://github.com/shizhilvren/tkeep.git
cd tkeep
cargo build --release
```

## Quick Start

### 1) Start a new session

```bash
tkeep new my-session
```

Alias form:

```bash
tkeep n my-session
```

### 2) Attach to the session

```bash
tkeep attach my-session
```

Alias form:

```bash
tkeep a my-session
```

## Command Usage

### `tkeep`

```text
Usage: tkeep [COMMAND]

Commands:
  new     Create a new terminal session
  attach  Attach to an existing terminal session
  help    Print this message or the help of the given subcommand(s)
```

### `tkeep new`

```text
Usage: tkeep new [OPTIONS] <NAME>

Arguments:
  <NAME>              Session name

Options:
  -s, --shell <SHELL> Shell binary to run (default: bash)
      --args <ARGS>   Extra args passed to shell command
      --history <SIZE>  History buffer size (default: 10000)
  -h, --help          Print help
```

Examples:

```bash
# Start with zsh
tkeep new work -s zsh

# Increase history buffer
tkeep new logs --history 50000
```

### `tkeep attach`

```text
Usage: tkeep attach [OPTIONS] <NAME>

Arguments:
  <NAME>      Session name

Options:
  -r, --replay  Replay history when attaching
  -h, --help    Print help
```

## Runtime Files

`tkeep` stores per-session runtime files (such as `.sock`, `.pid`, `.out`, `.err`) in a versioned runtime directory.

On Linux, it prefers the system runtime directory and falls back to your home directory when needed.

## Environment Variables

`tkeep` recognizes these environment variables for config/data path override:

- `TKEEP_CONFIG`
- `TKEEP_DATA`

You can also run `tkeep --version` to view detected config and data directories.

## Typical Workflow

```bash
# Start a session
tkeep new build

# Detach: just close terminal/SSH

# Re-attach later
tkeep attach build
```

## License

MIT
