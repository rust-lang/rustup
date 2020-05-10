---
name: I am experiencing a panic on WSL
about: Report a panic when using Rustup on WSL
labels: bug
---

<!-- Please read the following very carefully and decide if you
actually need to file this bug...

WSL, specifically WSL 1, has a limitation where glibc 2.31 and
newer will panic, fundamentally the panic will look something
like the following:

thread 'main' panicked at 'assertion failed: `(left == right)`
  left: `22`,
 right: `4`', src/libstd/sys/unix/thread.rs:166:21

This is a bug, but it's a bug in WSL, not in Rust/Rustup and will
affect other programs built with Rust too.  Working around it
with Rustup will not help you until WSL (or glibc) is fixed.

This is known to affect:

* Ubuntu 20.04
* Arch Linux

But it may affect other versions of Linux on WSL1.

You can find more information on the WSL bug report here:

https://github.com/microsoft/WSL/issues/4898
`
If you're CERTAIN that you're not reporting yet another duplicate
of the above issue, then...

Thanks for filing a 🐛 bug report 😄! -->

**Problem**
<!-- A clear and concise description of what the bug is. -->
<!-- including what currently happens and what you expected to happen. -->

**Steps**
<!-- The steps to reproduce the bug. -->
1.
2.
3.

**Possible Solution(s)**
<!-- Not obligatory, but suggest a fix/reason for the bug, -->
<!-- or ideas how to implement the addition or change -->

**Notes**

Output of `rustup --version`:
Output of `rustup show`:
