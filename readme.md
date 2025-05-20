Debugger for linux x86_64

[![Build](https://github.com/vajexal/dbg/actions/workflows/ci.yml/badge.svg)](https://github.com/vajexal/dbg/actions/workflows/ci.yml)

### Usage

consider the following code:

```c
#include <stdio.h>

int foo(int x)
{
    return x * 2;
}

int main()
{
    int y = foo(5);
    printf("%d\n", y);
    return 0;
}
```

compiled with

```bash
# program must be compiled with debugging info (`-g` flag for gcc for example)
gcc -g -O0 -Wall hello.c -o hello
```

then debugger could be invoked like this

```bash
dbg hello
```

### Commands

#### breakpoint | break | b

set a breakpoint. Argument is file:line, line or function name, for example

```
> b hello.c:10 // sets breakpoint on line 10 of file hello.c
> b 10 // sets breakpoint on line 10 of current file
> b foo // sets breakpoint on function's foo start
```

#### remove | rm

remove a breakpoint. `file:line` must be speicified as argument

```
> rm hello.c:10
```

#### list | l

list breakpoints

```
> l
hello.c:5
hello.c:10
```

#### disable

disable breakpoint so execution won't stop on the location

```
disable hello.c:5
```

#### enable

enable breakpoint so execution will stop on the location

```
enable hello.c:5
```

#### clear

remove all breakpoints

```
clear
```

#### run | r

run the program

#### stop

stop the execution

#### continue | cont | c

continue execution of the program

#### step

run the program until next line, for example if we stoped on `hello.c:10`

```
> step
```

now we are on line 11

#### step-in

run into function call, for example if we stopped on `hello.c:10`

```
> step-in
```

now we are on `hello.c:5`

#### step-out

run out of current function, for example if we stopped on `hello.c:5`

```
> step-out
```

now we are on `hello.c:11`

#### print | p

print variable

```
> p s // print variable
const char* s = "hello world"

> p &x // print address of x
int* &x = 0x7ffd8a95df50

> p *y // print value behind pointer
int *y = 10

> p *&x // multiple operators
int *&x = 10

> p foo.x // print field x of struct foo
int x = 15

> p a[0] // print static array element
int a[1] = 10

> p // prints all variables
const char* s = "hello world"
int x = 10
...
```

#### set

modify variable

```
> set *y = 20 // set value behind pointer

> set s = "somebody once told me the world is gonna roll me" // set string

> set foo.x = 30 // set struct field

> set a[1] = 20 // set static array element

> set color = BLUE // set enum variant

> set data.i = 20 // set union field

> set op = mul // set function pointer
```

#### location | loc

print current location

```
> loc
hello.c:5
```

#### quit | q

quit the program
