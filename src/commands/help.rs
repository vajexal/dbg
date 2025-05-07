pub fn help() {
    println!(
        "Commands:

breakpoint | break | b - set a breakpoint
remove | rm - remove a breakpoint
list | l - list breakpoints
disable - disable breakpoint
enable - enable breakpoint
clear - remove all breakpoints
run | r - run the program
stop - stop the execution
continue | cont | c - continue execution of the program
step - run the program until next line
step-in - run into function
step-out - run out of current function
print | p - print variable
set - modify variable
location | loc - print current location
quit | q - quit the program
"
    );
}
