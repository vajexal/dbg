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
    int *p = &i;
    const char *s = "hello world";
    printf("i = %d, f = %.2f, b = %s, p = %p, s = %s\\n", i, f, b ? "true" : "false", p, s);
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
            Step("set b 123", "invalid value"),
            Step("set *p = 345"),
            Step("p *p", "int *p = 345"),
            Step("p i", "int i = 345"),
            Step("set p = null"),
            Step("p p", "int* p = null"),
            Step("set x 123", "x not found"),
            Step('set s = "somebody once told me the world is gonna roll me"'),
            Step("p s", 'const char* s = "somebody once told me the world is gonna roll me"'),
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


def test_operators(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    int x = 10;
    int *y = &x;
    printf("%d\\n", *y);
    return 0;
}
""",
        steps=[
            Step("b 7", "breakpoint set"),
            Step("r"),
            Step("p &x", "int* &x = 0x"),
            Step("p *&x", "int *&x = 10"),
            Step("set *&x = 20"),
            Step("p x", "int x = 20"),
            Step("set &x = 30", "invalid location"),
            Step("p x", "int x = 20"),
            # Step("p **", "parser error"),
            # Step("p &&x", "parser error"),
            Step("q"),
        ]
    )


def test_enum(debugger):
    debugger(
        code="""#include <stdio.h>

enum Color
{
    RED,
    GREEN,
    BLUE,
};

int main()
{
    enum Color color = RED;
    switch (color)
    {
    case RED:
        printf("red\\n");
        break;
    case GREEN:
        printf("green\\n");
        break;
    case BLUE:
        printf("blue\\n");
        break;
    }
    return 0;
}
""",
        steps=[
            Step("b 13", "breakpoint set"),
            Step("r"),
            Step("p color", "enum Color color = RED"),
            Step("set color = BLUE"),
            Step("p color", "enum Color color = BLUE"),
            Step("set color = YELLOW", "invalid value"),
            Step("c"),
            Step("q"),
        ]
    )


def test_union(debugger):
    debugger(
        code="""#include <stdio.h>

union Data
{
    int i;
    float f;
};

int main()
{
    union Data data;
    data.i = 10;
    printf("%d\\n", data.i);
    return 0;
}
""",
        steps=[
            Step("b 13", "breakpoint set"),
            Step("r"),
            Step("p data", "invalid path"),
            Step("p data.i", "int i = 10"),
            Step("p data.s", "invalid path"),
            Step("set data = 20", "invalid path"),
            Step("set data.f = 3.14"),
            Step("p data.f", "float f = 3.14"),
            Step("c"),
            Step("q"),
        ]
    )
