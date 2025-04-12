from tests import Step


def test_set_var(debugger):
    debugger(
        code="""#include <stdio.h>
#include <stdbool.h>

int main()
{
    int i = 123;
    float f = 3.14;
    bool b = true;
    void *p = &i;
    printf("i = %d, f = %.2f, b = %s, p = %p\\n", i, f, b ? "true" : "false", p);
    return 0;
}
""",
        steps=[
            Step("b 10", "breakpoint set"),
            Step("r"),
            Step("set i 234"),
            Step("p i", "int i = 234"),
            Step("set f 9.81"),
            Step("p f", "float f = 9.81"),
            Step("set b false"),
            Step("p b", "bool b = false"),
            Step("set b true"),
            Step("p b", "bool b = true"),
            Step("set b none", "invalid value"),
            Step("set p 0xffffffff"),
            Step("p p", "void* p = 0xffffffff"),
            Step("set x 123", "x not found"),
            Step("set i foo", "invalid value"),
            Step("q"),
        ]
    )

def test_set_field(debugger):
    debugger(
        code="""#include <stdio.h>

typedef struct {
    int a;
    void *b;
} Foo;

int main()
{
    Foo foo = {123, (void *)0xff};
    printf("%d, %p\\n", foo.a, foo.b);
    return 0;
}
""",
        steps=[
            Step("b 11", "breakpoint set"),
            Step("r"),
            Step("set foo.a 100"),
            Step("p foo.a", "int a = 100"),
            Step("set foo.b 0xcc"),
            Step("p foo.b", "void* b = 0xcc"),
            Step("q"),
        ]
    )
