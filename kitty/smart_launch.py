"""Kitty kitten: context-aware window launch.

When the active window is an sshr remote session, launches a new sshr
window to the same host in the same working directory. Otherwise falls
back to launching a local window with cwd=current.
"""

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
        remote_cwd = window.cwd_of_child or ""

        cmd = ["sshr"]
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


