"""Kitty kitten: context-aware window launch.

When the active window is an sshr remote session, launches a new sshr
window to the same host in the same working directory. Otherwise falls
back to launching a local window with cwd=current.
"""

from urllib.parse import urlparse


def main(args):
    pass


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
            remote_cwd = str(urlparse(osc7_url).path)

        cmd = ["sshr", "--remote-cwd", remote_cwd, str(sshr_host)] if remote_cwd else ["sshr", str(sshr_host)]
        boss.show_error("sshr debug", f"osc7_url={osc7_url!r}\nremote_cwd={remote_cwd!r}\ncmd={cmd!r}")
        tab.new_window(cmd=cmd)
    else:
        cwd = window.cwd_of_child
        if cwd:
            tab.new_window(cwd=cwd)
        else:
            tab.new_window()


