# sshr

Resilient SSH sessions with automatic reconnection and persistent shells.

sshr wraps SSH with:

- **Persistent sessions** via [shpool](https://github.com/shell-pool/shpool) — your shell survives connection drops
- **Automatic reconnection** — prompts to reconnect when the connection is lost
- **SSH multiplexing** — reuses a single TCP connection for fast new windows
- **Auto-upload** — ships a shpool binary to remotes that don't have it installed
- **Shell detection** — automatically uses fish if available on the remote
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
sshr -a myhost

# Start in a specific directory
sshr --remote-cwd ~/projects myhost

# List remote sessions
sshr list myhost

# Kill specific sessions
sshr kill myhost s0 s1

# Kill all detached sessions
sshr clean myhost
```

## Kitty Integration

Copy `kitty/smart_launch.py` and `kitty/smart_close.py` to `~/.config/kitty/`, then add to `kitty.conf`:

```conf
map cmd+enter kitten smart_launch.py
map kitty_mod+enter kitten smart_launch.py
map cmd+x kitten smart_close.py
map kitty_mod+x kitten smart_close.py
```

This makes `cmd+enter` context-aware: in an sshr window it opens a new sshr session to the same host in the same directory; in a local window it opens a local shell in the current directory. `cmd+x` kills the remote shpool session when closing an sshr window.

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
