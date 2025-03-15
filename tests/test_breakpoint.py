from tests import Step


def test_breakpoints(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    int x = 0;
    x = 1;
    return 0;
}
""",
        steps=[
            Step("b 5", "breakpoint set"),
            Step("b 6", "breakpoint set"),
            Step("b 6", "breakpoint already exist"),
            Step("l", ["t.c:5", "t.c:6"]),
            Step("rm t.c:6", "breakpoint removed"),
            Step("l", "t.c:5"),
            Step("disable t.c:5", "breakpoint disabled"),
            Step("enable t.c:5", "breakpoint enabled"),
            Step("clear"),
            Step("r"),
            Step("stop", "invalid command"),  # assert program completed
            Step("q"),
        ],
        filename="t"
    )


def test_run_through_disabled_breakpoint(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    int x = 0;
    x = 1;
    return 0;
}
""",
        steps=[
            Step("b 5", "breakpoint set"),
            Step("disable t.c:5", "breakpoint disabled"),
            Step("r"),
            Step("stop", "invalid command"),  # assert program completed
            Step("q"),
        ],
        filename="t"
    )


def test_stop_at_reenabled_breakpoint(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    int x = 0;
    x = 1;
    return 0;
}
""",
        steps=[
            Step("b 5", "breakpoint set"),
            Step("disable t.c:5", "breakpoint disabled"),
            Step("enable t.c:5", "breakpoint enabled"),
            Step("r"),
            Step("stop"),  # assert program running
            Step("q"),
        ],
        filename="t"
    )


def test_breakpoint_by_func_name(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    printf("hello world\\n");
    return 0;
}
""",
        steps=[
            Step("b main", "breakpoint set"),
            Step("r"),
            Step("stop"),  # assert program running
            Step("q"),
        ],
        filename="t"
    )
