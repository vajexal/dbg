Debugger for linux x86_64

[![Build](https://github.com/vajexal/dbg/actions/workflows/ci.yml/badge.svg)](https://github.com/vajexal/dbg/actions/workflows/ci.yml)

### Commands

- `b` - set breakpoint. Exmaples:
    - `b t.c:6` - set breakpoint at file `t.c` at line 6
    - `b 6` - set breakpoint at line 6 of current file
- `rm` - remove breakpoint. Example: `rm t.c:6`
- `l` - list breakpoints
- `disable` - disable breakpoint. Debugger won't stop at disabled breakpoint
- `enable` - enable breakpoint
- `clear` - clear all breakpoints
- `r` - run the program
- `stop` - stop the program
- `c` - continue running the program after breakpoint hit
- `step` - run the program until next line
- `step-in` - run into function call
- `step-out` - run out of current function
- `p` - print var. Exmaples:
    - `p` - print all vars
    - `p foo` - print var *foo*
    - `p foo.bar` - print var *bar* of object *foo*
- `set` - set var. Examples:
    - `set i 123`
    - `set b true`
    - `set s = "hello world"` - sets string value
    - `set *foo.p = 123` - sets value behind pointer
- `q` - quit the program
