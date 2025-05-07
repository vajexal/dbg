from tests import Step


def test_run(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    printf("hello world\\n");
    return 0;
}
""",
        steps=[
            Step("b 5", "breakpoint set"),
            Step("r"),
            Step("r", "invalid command"),
            Step("loc", "t.c:5"),
            Step("c", "hello world"),
            Step("r", "invalid command"),
            Step("q"),
        ],
        filename="t"
    )


def test_quit_started_program(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    printf("hello world\\n");
    return 0;
}
""",
        steps=[
            Step("q"),
        ]
    )


def test_quit_running_program(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    printf("hello world\\n");
    return 0;
}
""",
        steps=[
            Step("b 5", "breakpoint set"),
            Step("r"),
            Step("q"),
        ]
    )
