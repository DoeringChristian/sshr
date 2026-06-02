"""Kitty kitten: context-aware window launch.

When the active window is an sshr remote session, launches a new sshr
window to the same host in the same working directory. Otherwise falls
back to launching a local window with cwd=current.

Reads login_shell from kitty's ssh.conf and passes it as --shell.
"""

import os
from urllib.parse import urlparse


def main(args):
    pass


def get_login_shell():
    conf = os.path.expanduser("~/.config/kitty/ssh.conf")
    if not os.path.exists(conf):
        return None
    with open(conf) as f:
        for line in f:
            line = line.strip()
            if line.startswith("login_shell "):
                return line.split(None, 1)[1]
    return None


from kittens.tui.handler import result_handler


@result_handler(no_ui=True)
def handle_result(args, answer, target_window_id, boss):
    window = boss.window_id_map.get(target_window_id)
    if window is None:
        return

    tab = boss.active_tab
    if tab is None:
        return

    sshr_host = window.user_vars.get("sshr_host", "")

    if sshr_host:
        remote_cwd = ""
        osc7_url = window.screen.last_reported_cwd
        if osc7_url:
            url = osc7_url.decode() if isinstance(osc7_url, bytes) else osc7_url
            remote_cwd = urlparse(url).path

        cmd = ["sshr"]
        shell = get_login_shell()
        if shell:
            cmd.extend(["--shell", shell])
        if remote_cwd:
            cmd.extend(["--remote-cwd", remote_cwd])
        cmd.append(sshr_host)
        tab.new_window(cmd=cmd)
    else:
        cwd = window.cwd_of_child
        if cwd:
            tab.new_window(cwd=cwd)
        else:
            tab.new_window()
