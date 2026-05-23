"""Kitty kitten: context-aware window close.

When the active window is an sshr remote session, kills the remote
shpool session before closing the kitty window. Otherwise just closes
the window normally.
"""

import subprocess


def main(args):
    pass


from kittens.tui.handler import result_handler


@result_handler(no_ui=True)
def handle_result(args, answer, target_window_id, boss):
    window = boss.window_id_map.get(target_window_id)
    if window is None:
        return

    user_vars = window.user_vars
    sshr_host = user_vars.get("sshr_host", "")
    sshr_session = user_vars.get("sshr_session", "")
    sshr_tool = user_vars.get("sshr_tool", "")

    if sshr_host and sshr_session and "shpool" in sshr_tool:
        # Kill the remote shpool session in the background
        subprocess.Popen(
            ["ssh", sshr_host, f"{sshr_tool} kill {sshr_session}"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    boss.close_window()
