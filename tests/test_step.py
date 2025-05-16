from tests import Step


def test_step(debugger):
    debugger(
        code="""#include <stdio.h>

int foo(int x) { return x * 2; }

int bar(int x)
{
    return foo(x);
}

int main()
{
    int y = bar(5);
    printf("%d\\n", y);
    return 0;
}
""",
        steps=[
            Step("b 7", "breakpoint set"),
            Step("r"),
            Step("loc", "t.c:7"),
            Step("step"),
            Step("loc", "t.c:13"),  # check that function call and prologue are skipped
            Step("step"),
            Step("loc", "t.c:14"),
            Step("c"),
            Step("q"),
        ],
        filename="t"
    )


def test_step_in(debugger):
    debugger(
        code="""#include <stdio.h>

int foo(int x) { return x * 2; }

int bar(int x)
{
    return foo(x);
}

int main()
{
    int y = bar(5);
    printf("%d\\n", y);
    return 0;
}
""",
        steps=[
            Step("b 12", "breakpoint set"),
            Step("r"),
            Step("loc", "t.c:12"),
            Step("step-in"),
            Step("loc", "t.c:7"),
            Step("step-in"),
            Step("loc", "t.c:3"),
            Step("step-in"),
            Step("loc", "t.c:13"),
            Step("c"),
            Step("q"),
        ],
        filename="t"
    )


def test_step_out(debugger):
    debugger(
        code="""#include <stdio.h>

int foo(int x)
{
    return x * 2;
}

int main()
{
    int y = foo(5);
    printf("%d\\n", y);
    return 0;
}
""",
        steps=[
            Step("b 5", "breakpoint set"),
            Step("r"),
            Step("loc", "t.c:5"),
            Step("step-out"),
            Step("loc", "t.c:11"),
            Step("step-out", "10"),
            Step("stop", "invalid command"),  # assert program completed
            Step("q"),
        ],
        filename="t"
    )
