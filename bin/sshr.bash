#!/usr/bin/env bash
set -u

# Resolve sshr's install directory for finding bundled shpool binaries
SSHR_DIR="$(cd "$(dirname "$0")/.." && pwd)"

CONTROL_DIR="$HOME/.ssh/sshr-sockets"

# Subcommands
case "${1:-}" in
    list|ls)
        # List remote shpool sessions on a host
        if [ -z "${2:-}" ]; then
            echo "Usage: sshr list <host>" >&2
            exit 1
        fi
        HOST="$2"
        mkdir -p "$CONTROL_DIR"
        CONTROL_PATH="$CONTROL_DIR/%r@%h:%p"
        SSH_MUX_OPTS="-o ControlMaster=auto -o ControlPath=$CONTROL_PATH -o ControlPersist=10m"
        ssh $SSH_MUX_OPTS "$HOST" '
            for dir in $(echo "$PATH" | tr ":" " ") "$HOME/.nix-profile/bin" "$HOME/.local/bin"; do
                if [ -x "$dir/shpool" ]; then "$dir/shpool" list; exit; fi
            done
            echo "shpool not found on remote" >&2
        ' 2>/dev/null
        exit
        ;;
    kill)
        # Kill specific remote shpool sessions
        if [ -z "${2:-}" ]; then
            echo "Usage: sshr kill <host> [session...]" >&2
            echo "  If no sessions specified, lists them for selection." >&2
            exit 1
        fi
        HOST="$2"
        shift 2
        mkdir -p "$CONTROL_DIR"
        CONTROL_PATH="$CONTROL_DIR/%r@%h:%p"
        SSH_MUX_OPTS="-o ControlMaster=auto -o ControlPath=$CONTROL_PATH -o ControlPersist=10m"
        SESSIONS="$*"
        if [ -z "$SESSIONS" ]; then
            echo "Sessions on $HOST:"
            ssh $SSH_MUX_OPTS "$HOST" '
                for dir in $(echo "$PATH" | tr ":" " ") "$HOME/.nix-profile/bin" "$HOME/.local/bin"; do
                    if [ -x "$dir/shpool" ]; then "$dir/shpool" list; exit; fi
                done
            ' 2>/dev/null
            printf "Sessions to kill (space-separated): "
            read -r SESSIONS
            [ -z "$SESSIONS" ] && exit 0
        fi
        ssh $SSH_MUX_OPTS "$HOST" '
            for dir in $(echo "$PATH" | tr ":" " ") "$HOME/.nix-profile/bin" "$HOME/.local/bin"; do
                if [ -x "$dir/shpool" ]; then "$dir/shpool" kill '"$SESSIONS"'; exit; fi
            done
        '
        exit
        ;;
    clean)
        # Kill all detached (inactive) remote shpool sessions
        if [ -z "${2:-}" ]; then
            echo "Usage: sshr clean <host>" >&2
            exit 1
        fi
        HOST="$2"
        mkdir -p "$CONTROL_DIR"
        CONTROL_PATH="$CONTROL_DIR/%r@%h:%p"
        SSH_MUX_OPTS="-o ControlMaster=auto -o ControlPath=$CONTROL_PATH -o ControlPersist=10m"
        ssh $SSH_MUX_OPTS "$HOST" '
            for dir in $(echo "$PATH" | tr ":" " ") "$HOME/.nix-profile/bin" "$HOME/.local/bin"; do
                if [ -x "$dir/shpool" ]; then
                    DETACHED=$("$dir/shpool" list 2>/dev/null | awk "NR>1 && \$3==\"detached\" {print \$1}")
                    if [ -z "$DETACHED" ]; then
                        echo "No detached sessions."
                    else
                        echo "Killing detached sessions: $DETACHED"
                        "$dir/shpool" kill $DETACHED
                    fi
                    exit
                fi
            done
            echo "shpool not found on remote" >&2
        '
        exit
        ;;
    help|--help|-h)
        cat <<'USAGE'
Usage: sshr [command] [options] <host> [ssh-args...]

Resilient SSH sessions with automatic reconnection and persistent
shells via shpool. Multiplexes SSH connections for fast new windows.

Commands:
  <host>                  Connect to host (default)
  list   <host>           List remote shpool sessions
  kill   <host> [name..] Kill remote shpool sessions
  clean  <host>           Kill all detached sessions

Options:
  -a, --attach            Attach to an existing session
  --remote-cwd <path>     Start in the given remote directory
  --shell <path>          Shell to use on remote (default: auto-detect fish)

Environment:
  SSHR_SHPOOL_DIR         Directory containing shpool-<os>-<arch> binaries
                          for auto-upload (default: <sshr-install>/shpool/bin)
USAGE
        exit
        ;;
esac

ATTACH=false
REMOTE_CWD=""
REMOTE_SHELL=""

# Parse options
while [ $# -gt 0 ]; do
    case "$1" in
        -a|--attach)
            ATTACH=true
            shift
            ;;
        --remote-cwd)
            REMOTE_CWD="$2"
            shift 2
            ;;
        --shell)
            REMOTE_SHELL="$2"
            shift 2
            ;;
        -*)
            echo "Unknown option: $1" >&2
            echo "Run 'sshr help' for usage." >&2
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

if [ $# -lt 1 ]; then
    echo "Usage: sshr [command] [options] <host> [ssh-args...]" >&2
    echo "Run 'sshr help' for usage." >&2
    exit 1
fi

HOST="$1"
shift

# Tell kitty this window is an sshr session
if [ -n "${KITTY_WINDOW_ID:-}" ]; then
    printf '\e]1337;SetUserVar=%s=%s\a' "sshr_host" "$(printf '%s' "$HOST" | base64)"
fi

# SSH connection multiplexing — reuse a single TCP connection across
# multiple sshr invocations to the same host
mkdir -p "$CONTROL_DIR"
CONTROL_PATH="$CONTROL_DIR/%r@%h:%p"
SSH_MUX_OPTS="-o ControlMaster=auto -o ControlPath=$CONTROL_PATH -o ControlPersist=10m"

# Probe for session tools and preferred shell on the remote in a single SSH
# call. Checks PATH and common locations since non-interactive SSH sessions
# may not have ~/.nix-profile/bin in PATH.
# Output: two lines — session tool path, then shell path (or "none")
probe_remote() {
    ssh $SSH_MUX_OPTS "$HOST" "$@" '
        find_cmd() {
            for tool in $@; do
                for dir in $(echo "$PATH" | tr ":" " ") "$HOME/.nix-profile/bin" "$HOME/.local/bin"; do
                    if [ -x "$dir/$tool" ]; then
                        echo "$dir/$tool"
                        return
                    fi
                done
            done
            echo none
        }
        find_cmd shpool abduco
        find_cmd fish
    ' 2>/dev/null
}

PROBE_OUTPUT="$(probe_remote "$@")"
TOOL_PATH="$(echo "$PROBE_OUTPUT" | sed -n '1p')"
TOOL_NAME="$(basename "$TOOL_PATH")"
FISH_PATH="$(echo "$PROBE_OUTPUT" | sed -n '2p')"

# Use explicitly specified shell, detected fish, or fall back to default
if [ -n "$REMOTE_SHELL" ]; then
    SHELL_PATH="$REMOTE_SHELL"
elif [ "$FISH_PATH" != "none" ]; then
    SHELL_PATH="$FISH_PATH"
else
    SHELL_PATH=""
fi

# If no session tool found, try to upload shpool binary
if [ "$TOOL_NAME" = "none" ]; then
    REMOTE_PLATFORM="$(ssh $SSH_MUX_OPTS "$HOST" 'uname -sm' 2>/dev/null)"
    REMOTE_OS="$(echo "$REMOTE_PLATFORM" | awk '{print tolower($1)}')"
    REMOTE_ARCH="$(echo "$REMOTE_PLATFORM" | awk '{print $2}')"
    case "$REMOTE_ARCH" in
        amd64) REMOTE_ARCH="x86_64" ;;
        arm64) REMOTE_ARCH="aarch64" ;;
    esac

    # Check repo layout, then Nix install layout
    if [ -d "$SSHR_DIR/shpool/bin" ]; then
        DEFAULT_SHPOOL_DIR="$SSHR_DIR/shpool/bin"
    else
        DEFAULT_SHPOOL_DIR="$SSHR_DIR/share/sshr/shpool/bin"
    fi
    SHPOOL_BIN_DIR="${SSHR_SHPOOL_DIR:-$DEFAULT_SHPOOL_DIR}"
    LOCAL_BINARY="$SHPOOL_BIN_DIR/shpool-${REMOTE_OS}-${REMOTE_ARCH}"

    if [ -f "$LOCAL_BINARY" ]; then
        echo "Uploading shpool to $HOST..."
        ssh $SSH_MUX_OPTS "$HOST" 'mkdir -p ~/.local/bin' 2>/dev/null
        scp -o "ControlPath=$CONTROL_PATH" "$LOCAL_BINARY" "$HOST:~/.local/bin/shpool" >/dev/null
        ssh $SSH_MUX_OPTS "$HOST" 'chmod +x ~/.local/bin/shpool' 2>/dev/null
        TOOL_PATH='$HOME/.local/bin/shpool'
        TOOL_NAME="shpool"
        echo "Done."
    fi
fi

# List existing sessions on the remote
list_sessions() {
    case "$TOOL_NAME" in
        shpool)
            ssh $SSH_MUX_OPTS "$HOST" "$@" "$TOOL_PATH list 2>/dev/null" 2>/dev/null | tail -n +2
            ;;
        abduco)
            ssh $SSH_MUX_OPTS "$HOST" "$@" "$TOOL_PATH 2>&1 | tail -n +2" 2>/dev/null
            ;;
    esac
}

# Pick an existing session or bail out
pick_session() {
    SESSIONS="$(list_sessions "$@")"
    if [ -z "$SESSIONS" ]; then
        echo "No existing sessions on $HOST." >&2
        exit 1
    fi
    echo "Sessions on $HOST:"
    echo "$SESSIONS" | awk '{ printf "  [%d] %s\n", NR, $0 }'
    NUM="$(echo "$SESSIONS" | wc -l | tr -d ' ')"
    printf "Select session: "
    read -r CHOICE
    if echo "$CHOICE" | grep -qE '^[0-9]+$' && [ "$CHOICE" -ge 1 ] && [ "$CHOICE" -le "$NUM" ]; then
        SESSION="$(echo "$SESSIONS" | sed -n "${CHOICE}p" | awk '{print $1}')"
    else
        echo "Invalid selection." >&2
        exit 1
    fi
}

# Generate a unique session name
new_session_name() {
    EXISTING="$(list_sessions "$@")"
    I=0
    while true; do
        NAME="s$I"
        if ! echo "$EXISTING" | awk '{print $1}' | grep -qx "$NAME"; then
            echo "$NAME"
            return
        fi
        I=$((I + 1))
    done
}

if [ "$ATTACH" = true ] && [ "$TOOL_NAME" != "none" ]; then
    pick_session "$@"
elif [ "$TOOL_NAME" != "none" ]; then
    SESSION="$(new_session_name "$@")"
fi

# Tell kitty the session name and tool path (now that they've been determined)
if [ -n "${KITTY_WINDOW_ID:-}" ] && [ -n "${SESSION:-}" ]; then
    printf '\e]1337;SetUserVar=%s=%s\a' "sshr_session" "$(printf '%s' "$SESSION" | base64)"
    printf '\e]1337;SetUserVar=%s=%s\a' "sshr_tool" "$(printf '%s' "$TOOL_PATH" | base64)"
fi

build_cmd() {
    case "$TOOL_NAME" in
        shpool)
            local CMD="$TOOL_PATH attach $SESSION"
            if [ -n "$SHELL_PATH" ]; then
                CMD="$CMD -c '$SHELL_PATH -C \"set -gx SSH_CONNECTION 1\"'"
            fi
            if [ -n "$REMOTE_CWD" ]; then
                CMD="$CMD -d $(printf '%q' "$REMOTE_CWD")"
            fi
            echo "$CMD"
            ;;
        abduco)
            local SH="\"\$SHELL\""
            if [ -n "$SHELL_PATH" ]; then
                SH="$SHELL_PATH"
            fi
            if [ -n "$REMOTE_CWD" ]; then
                echo "cd $(printf '%q' "$REMOTE_CWD") && $TOOL_PATH -A $SESSION $SH"
            else
                echo "$TOOL_PATH -A $SESSION $SH"
            fi
            ;;
        *)
            if [ -n "$SHELL_PATH" ] && [ -n "$REMOTE_CWD" ]; then
                echo "$SHELL_PATH -C 'cd $(printf '%q' "$REMOTE_CWD")'"
            elif [ -n "$SHELL_PATH" ]; then
                echo "$SHELL_PATH"
            elif [ -n "$REMOTE_CWD" ]; then
                echo "cd $(printf '%q' "$REMOTE_CWD") && \"\$SHELL\""
            else
                echo ""
            fi
            ;;
    esac
}

REMOTE_CMD="$(build_cmd)"

connect() {
    if [ -n "$REMOTE_CMD" ]; then
        ssh $SSH_MUX_OPTS "$HOST" "$@" -t "$REMOTE_CMD"
    else
        ssh $SSH_MUX_OPTS "$HOST" "$@"
    fi
}

echo "Connecting to $HOST${TOOL_NAME:+ (using $TOOL_NAME, session: $SESSION)}..."
while true; do
    connect "$@"
    STATUS=$?

    # Normal exit (user typed exit/logout)
    if [ $STATUS -eq 0 ]; then
        break
    fi

    echo ""
    echo "Connection to $HOST lost. Press any key to reconnect (Ctrl-C to quit)..."
    read -r -s -n 1 || break
    echo "Reconnecting..."
done
