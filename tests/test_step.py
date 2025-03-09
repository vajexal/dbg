from tests import Step, debug_code


def test_step(tmp_path):
    debug_code(
        tmp_path,
        code="""#include <stdio.h>

int x = 0;

void foo()
{
    x = 1;
}

int main()
{
    foo();
    x = 2;
    return 0;
}
""",
        steps=[
            Step("b 12", "breakpoint set"),
            Step("r"),
            Step("p x", "int x = 0"),
            Step("step"),
            Step("p x", "int x = 1"),
            Step("step"),
            Step("p x", "int x = 2"),
            Step("c"),
            Step("q"),
        ]
    )


def test_step_in(tmp_path):
    debug_code(
        tmp_path,
        code="""#include <stdio.h>

int x = 0;

void foo()
{
    x = 1;
}

int main()
{
    foo();
    x = 2;
    return 0;
}
""",
        steps=[
            Step("b 12", "breakpoint set"),
            Step("r"),
            Step("p x", "int x = 0"),
            Step("step-in"),
            Step("step-in"),
            Step("step-in"),
            Step("p x", "int x = 1"),
            Step("step-in"),
            Step("step-in"),
            Step("p x", "int x = 2"),
            Step("c"),
            Step("q"),
        ]
    )


def test_step_out(tmp_path):
    debug_code(
        tmp_path,
        code="""#include <stdio.h>

int x = 0;

void foo()
{
    x = 1;
}

int main()
{
    foo();
    x = 2;
    return 0;
}
""",
        steps=[
            Step("b 7", "breakpoint set"),
            Step("r"),
            Step("p x", "int x = 0"),
            Step("step-out"),
            Step("p x", "int x = 1"),
            Step("step-out"),
            Step("stop", "invalid command"),  # assert program completed
            Step("q"),
        ]
    )
