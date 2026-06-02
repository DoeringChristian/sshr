# sshr

Resilient SSH sessions with automatic reconnection and persistent shells.

sshr wraps SSH with:

- **Persistent sessions** via [shpool](https://github.com/shell-pool/shpool) — your shell survives connection drops
- **Automatic reconnection** — prompts to reconnect when the connection is lost
- **SSH multiplexing** — reuses a single TCP connection for fast new windows
- **Auto-upload** — ships a shpool binary to remotes that don't have it installed
- **Shell-agnostic** — works with any login shell (bash, zsh, fish, etc.) and injects OSC 7 CWD reporting
- **Kitty integration** — optional kittens for smart window launch/close

## Install

### Nix

```bash
nix profile install github:DoeringChristian/sshr
```

### Manual

Clone and add `bin/` to your PATH:

```bash
git clone https://github.com/DoeringChristian/sshr.git
export PATH="$PWD/sshr/bin:$PATH"
```

## Usage

```bash
# Connect to a host (creates a new shpool session)
sshr myhost

# Attach to an existing session
sshr attach myhost

# Start in a specific directory
sshr --remote-cwd ~/projects myhost

# Use a specific shell on the remote
sshr --shell /bin/zsh myhost

# List remote sessions
sshr list myhost

# Kill specific sessions
sshr kill myhost s0 s1

# Kill all detached sessions
sshr clean myhost
```

## Shell Support

sshr deploys lightweight init files to the remote (`~/.local/share/sshr/init/`) that add OSC 7 CWD reporting to your shell. This enables features like opening new windows in the same remote directory. Supported shells:

- **bash** — init via `ENV` + POSIX mode
- **zsh** — init via `ZDOTDIR`
- **fish** — init via `XDG_DATA_DIRS`
- **other** — launched directly (no OSC 7 injection)

By default sshr uses the remote's login shell. To always use a specific shell, set it in `~/.config/sshr/config`:

```
shell = /usr/bin/fish
```

The `--shell` flag overrides the config file.

## Kitty Integration

sshr works in any terminal, but ships optional kittens for kitty users. Copy `kitty/smart_launch.py` and `kitty/smart_close.py` to `~/.config/kitty/`, then add to `kitty.conf`:

```conf
map cmd+enter kitten smart_launch.py
map kitty_mod+enter kitten smart_launch.py
map cmd+x kitten smart_close.py
map kitty_mod+x kitten smart_close.py
```

**smart_launch** (`cmd+enter`) is context-aware: in an sshr window it opens a new sshr session to the same host in the same directory; in a local window it opens a local shell in the current directory.

**smart_close** (`cmd+x`) kills the remote shpool session when closing an sshr window.

## Pre-built shpool Binaries

sshr can auto-upload a shpool binary to remotes that don't have it installed. To build binaries for a platform, run `shpool/build.sh` on that platform:

```bash
# On each target machine:
bash shpool/build.sh
```

This builds a portable shpool binary and places it in `shpool/bin/`. On Linux, it produces a statically-linked musl binary.

You can also set `SSHR_SHPOOL_DIR` to point to a custom directory containing the binaries.

## License

MIT
