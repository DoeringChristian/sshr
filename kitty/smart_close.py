"""Kitty kitten: context-aware window close.

Closes the window. If it's an sshr session, sshr handles cleanup
via its signal handler and WAL.
"""


def main(args):
    pass


from kittens.tui.handler import result_handler


@result_handler(no_ui=True)
def handle_result(args, answer, target_window_id, boss):
    boss.close_window()
