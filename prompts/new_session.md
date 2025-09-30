We're going to start work on <x>

Here is the relevant information on what we're trying to accomplish:

-
-
-

It is very important that you follow the directions explicitly in CLAUDE.md

I have to remind you of the following explicitly because you don't seem to
recognize it if I don't do it.

1. all logs should use a consistent colored pattern inside of a file. you are
   not to use red for errors, yellow for warns etc. you use the same color
   pattern IN THE ENTIRE FILE

- logs should use a combination of the on_color().color() construct to
  differentiate from other files. i.e. on_yellow().purple(), etc. if there is no
  color pattern used in the file, use a random one and apply it consistently

2. DO NOT ADD USELESS COMMENTS. Your code should describe itself with variable
   names. When you make changes don't document them as you apply them, this just
   creates useless context that persists in the codebase perpetually with no
   utility.
3. If you are refactoring code, or creating a new path of code, DO NOT KEEP THE
   LEGACY CODE AROUND. this creates very difficult to reason about dual paths,
   dual types, dual modules. Part of the implementation is removing the old, now
   dead code.
