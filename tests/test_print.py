from tests import Step


def test_print_primitive(debugger):
    debugger(
        code="""#include <stdio.h>
#include <stdbool.h>

int main()
{
    int i = 123;
    float f = 3.14;
    bool b = true;
    const char *s = "hello world";
    printf("i = %d, f = %.2f, b = %s, s = %s\\n", i, f, b ? "true" : "false", s);
    return 0;
}
""",
        steps=[
            Step("b 10", "breakpoint set"),
            Step("r"),
            Step("p i", "int i = 123"),
            Step("p f", "float f = 3.14"),
            Step("p b", "bool b = true"),
            Step("p s", 'const char* s = "hello world"'),
            Step("p x", "x not found"),
            Step("p", [
                "int i = 123",
                "float f = 3.14",
                "bool b = true",
                'const char* s = "hello world"'
            ]),
            Step("c"),
            Step("q"),
        ]
    )


def test_print_void_ptr(debugger):
    debugger(
        code="""#include <stdio.h>

int main()
{
    int i = 123;
    void *p = &i;
    printf("p = %p\\n", p);
    return 0;
}
""",
        steps=[
            Step("b 7", "breakpoint set"),
            Step("r"),
            Step("p p", "void* p = 0x"),
            Step("c"),
            Step("q"),
        ]
    )


def test_print_nested_struct(debugger):
    debugger(
        code="""#include <stdio.h>

struct Foo {
    int a;
    struct Bar {
        int b;
    } bar;
};

int main()
{
    struct Foo foo = {10, {20}};

    printf("%d, %d\\n", foo.a, foo.bar.b);

    return 0;
}
""",
        steps=[
            Step("b 14", "breakpoint set"),
            Step("r"),
            Step("p foo.a", "int a = 10"),
            Step("p foo.bar.b", "int b = 20"),
            Step("p foo", "Foo foo = { a = 10, bar = { b = 20 } }"),
            Step("c"),
            Step("q"),
        ]
    )


def test_print_node(debugger):
    debugger(
        code="""#include <stdio.h>
#include <stdlib.h>

struct Node {
    int value;
    struct Node *left;
    struct Node *right;
};

typedef struct Node Node;

Node *Node_new(int value)
{
    Node *node = malloc(sizeof(Node));
    node->value = value;
    node->left = node->right = NULL;
    return node;
}

void Node_free(Node *node)
{
    if (node->left != NULL) {
        Node_free(node->left);
    }

    if (node->right != NULL) {
        Node_free(node->right);
    }

    free(node);
}

int main()
{
    Node *root = Node_new(10);
    root->left = Node_new(5);
    root->right = Node_new(15);

    printf("%d\\n", root->right->value);

    Node_free(root);

    return 0;
}
""",
        steps=[
            Step("b 39", "breakpoint set"),
            Step("r"),
            Step("p root.right.value", "int value = 15"),
            Step("p *root.left", "Node *left = { value = 5, left = null, right = null }"),
            Step("p root.right.right.value", "invalid path", "int value"),
            Step("c"),
            Step("q"),
        ]
    )


def test_print_func(debugger):
    debugger(
        code="""#include <stdio.h>

typedef int (*Operation)(int, int);

int add(int a, int b)
{
    return a + b;
}

int main()
{
    Operation op1 = add;
    int (*op2)(int, int) = add;
    printf("%d\\n", op2(5, op1(3, 4)));
    return 0;
}
""",
        steps=[
            Step("b 14", "breakpoint set"),
            Step("r"),
            Step("p op1", "Operation op1 = add"),
            Step("p op2", "int (int, int) op2 = add"),
            Step("p *op1", "invalid path"),
            Step("c"),
            Step("q"),
        ]
    )
