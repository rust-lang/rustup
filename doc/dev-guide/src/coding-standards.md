# Coding standards

Generally we just follow good sensible Rust practices, clippy and so forth.
However there are some practices we've agreed on that are not machine-enforced;
meeting those requirements in a PR will make it easier to merge.

The following guidelines are based on that of
[rustls](https://github.com/rustls/rustls/blob/64cf0213132d9793da168b0287462fa5ec6630cf/CONTRIBUTING.md),
slightly adapted to fit rustup's situation.

## Atomic commits

Our default workflow is to rebase clean commit history from a PR into the
target branch, and we prefer to keep the history clean and easy to follow,
blame, bisect, backport, etc.

The idea is that when bisecting, one can easily understand whether the breakage
was introduced in a refactoring commit or a functional change; when
backporting, it becomes much easier to keep all the mechanical changes in the
backport to reduce the risk of merge conflicts, and so on.

Thus, we use atomic commits across the repo. This means that when drafting a
PR, the general goal is to [rewrite the
history](https://git-scm.com/book/en/v2/Git-Tools-Rewriting-History) so that
each commit should represent a single unit of change, ideally so "boring" that
the commit message alone can be used to understand the change without having to
read the code. In particular:

- Avoid mixing refactoring and functional changes in the same commit if possible
- Make mechanical changes (like renaming or moving code around) in a separate commit
- Isolate updates to Cargo.lock in their own commits

You can read more about atomic commits
[here](https://www.aleksandrhovhannisyan.com/blog/atomic-git-commits).

## Commit messages

Each message should in principle start with a short verbal phrase describing
the change. If mentions of issues/PRs are desired, use the `#1234` format
instead of pasting links.

It is useful to refer to [conventional
commits](https://www.conventionalcommits.org/en/v1.0.0/) for more detailed
guidance on writing good commit messages, however strict adherence to the
format is not required.

## Coding style

### Ordering

#### Top-down ordering within modules

Within a module, we prefer to order items top-down. This means that items within
a module will depend on items defined below them, but not (usually) above them.
The idea here is that the public API, with more internal dependencies, will be
read (and changed) more often, and putting it closer to the top of the module
makes it more accessible.

This can be surprising to many engineers who are used to the bottom-up ordering
used in languages like Python, where items can have a run-time dependency on
other items defined in the same module.

Usually `const` values will thus go on the bottom of the module (least complex,
usually no dependencies of their own), although in larger modules it can make
sense to place a `const` directly below the user (especially if there is a
single user, or just a few co-located users).

The `#[cfg(test)] mod tests {}` module goes on the very bottom, if present.
Other module definitions (like `mod foo { .. }`) can be ordered among other
items as it makes sense in the context of the items imported from them.
Module declarations (like `mod foo;`) should be ordered before other items
but after imports. Imports from local modules (both declared and defined)
should be kept close to the module declaration/definition.

Files that have substantial amounts of code inside inline modules should
probably avoid also having much code outside of these modules.

#### Ordering for a given type

For a given type, we prefer to order items as follows:

1. The type definition (`struct` or `enum`)
2. The inherent `impl` block (that is, not a trait implementation)
3. `impl` blocks for traits, from most specific to least specific.
   The least specific would be something like a `Debug` or `Clone` impl.

#### Ordering associated functions within an inherent `impl` block

Here's a guide to how we like to order associated functions:

0. Associated functions (that is, `fn foo() {}` instead of `fn foo(&self) {}`)
1. Constructors, starting with the constructor that takes the least arguments
2. Public API that takes a `&mut self`
3. Public API that takes a `&self`
4. Private API that takes a `&mut self`
5. Private API that takes a `&self`
6. `const` values

Note that we usually also practice top-down ordering here; where these are in
conflict, make a choice that you think makes sense. For getters and setters, the
order should typically mirror the order of the fields in the type definition.

#### Attribute ordering

Order attributes so that documentation appears first, and the attributes with the
most effect on the meaning and function of the type appear last. For example:

```rust
/// Doc comment always first
#[cfg(feature-gates)]
#[allow(lint-configuration)]
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Foo;
```

Prefer to write `derive`d traits in alphabetical order.

### Functions

#### Consider avoiding short single-use functions

While single-use functions can make sense if the algorithm is sufficiently complex
that it warrants an explicit name and interface, using many short single-use
functions can make the code harder to follow, due to having to jump around in order
to gain an understanding of what's going on. When writing a single-use function,
consider whether it needs the dedicated interface, or if it could be inlined into
its caller instead.

As an exception, the introduction of a single-use function is allowed as an
intermediate step in the middle of a PR, given that at the end of the PR the
said function has either been reused or become reasonably complex.

#### Consider avoiding free-standing functions

If a function's semantics or implementation are strongly dependent on one of its
arguments, and the argument is defined in a type within the current crate,
prefer using a method on the type. Similarly, if a function is taking multiple
arguments that originate from the same common type in all call-sites it is
a strong candidate for becoming a method on the type.

#### Order arguments from most specific to least specific

When writing a function, we prefer to order arguments from most specific to
least specific. This means that an `image_id` might go before the `domain`,
which will go before the `app` context. More specific arguments are more
differentiating between a given function and other functions, so putting them
first makes it easier to infer the context/meaning of the function (compared to
starting with a number of generic context-like types).

#### Use `impl Trait` types where possible

We prefer to use `impl ...` for arguments and return types when there's a single
use of the type. Generic type argument bounds add a level of indirection that's
harder to read in one pass.

#### Avoid type elision for fully qualified function calls

We prefer to write [fully qualified function calls] with types included, rather
than elided. For example:

```rust
// Incorrect:
<_>::default()

// Correct:
CertificateChain::default()
```

[fully qualified function calls]: https://doc.rust-lang.org/beta/reference/expressions/call-expr.html#disambiguating-function-calls

#### Validation

Where possible, avoid writing `validate` or `check` type functions that try to
check for error conditions based on the state of a populated object. Prefer
["parse, don't validate"](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)
style and try to use the type system to make it impossible for invalid states to
be represented.

#### Error handling

We use `Result` types pervasively throughout the code to signal error cases.
Outside of unit/integration tests we prefer to avoid `unwrap()` and `expect()`
calls unless there is a clear invariant which can be locally validated by the
structure of the code. If there is such an invariant, we usually add a comment
explaining how the invariant is upheld. In other cases (especially for error
cases which can arise from network traffic, which could represent an attacker),
we always prefer to handle errors and ultimately return an error to the network
peer or close the connection.

### Expressions

#### Avoid single-use bindings

We generally make full use of the expression-oriented nature of Rust. For
example, when using iterators we prefer to use `map` and other combinators
instead of `for`-loops when possible, and will often avoid variable bindings if
a variable is only used once. Naming variables takes cognitive efforts, and so
does tracking references to bindings in your mind. One metric we like to
minimize is the number of mutable bindings in a given scope.

Remember that the overall goal is to make the code easy to understand.
Combinators can help with this by eliding boilerplate (like replacing a
`None => None` arm with a `map()` call), but they can also make it harder to
understand the code. One example is that a combinator chain like
`.map().map_err()` might be harder to understand than a `match` statement
(since, in this case, both of the arms have a significant transformation).

#### Use early `return` and `continue` to reduce nesting

The typed nature of Rust can cause some code to end up at deeply indented
levels, which we call "rightward drift". This makes lines shorter, making the
code harder to read. To avoid this, try to `return` early for error cases, or
`continue` early in a loop to skip an iteration.

#### Hoist common expression returns

When writing a `match` or `if` expression that has arms that each share a return
type (e.g. `Ok(...)`), hoist the commonality outside the `match`. This helps
separate out the important differences and reduces code duplication.

```rust
// Incorrect:
match foo {
    1..10 => Ok(do_one_thing()),
    _ => Ok(do_another()),
}

// Correct:
Ok(match foo {
    1..10 => do_one_thing(),
    _ => do_another(),
})
```

#### Avoid `ref` in match patterns

When writing match expressions, try to avoid using `ref` in patterns. Prefer
taking a reference on the
[scrutinee](https://doc.rust-lang.org/reference/expressions/match-expr.html)
of the `match`.

Since the addition of [binding
modes](https://rust-lang.github.io/rfcs/2005-match-ergonomics.html) for improved
match ergonomics the `ref` keyword is unidiomatic and can be unfamiliar to
readers.

### Naming

#### Use concise names

We prefer concise names, especially for local variables, but prefer to
expand acronyms/abbreviations that are not very well known (e.g. prefer
`key_usage` instead of `ku`, `anonymous` instead of `anon`). Extremely common
short-forms like `url` are acceptable.

Avoid adding a suffix for a variable that describes its type (provided that its
type is hard to confuse with other types -- for example, we do still use `_id`
suffixes because we usually use numeric IDs for database entities). The
precision/conciseness trade-off for variable names also depends on the scope of
the binding.

#### Avoid `get_` prefixes

Per the
[API guidelines](https://rust-lang.github.io/api-guidelines/naming.html#getter-names-follow-rust-convention-c-getter),
`get_()` prefixes are discouraged.

```rust
// Incorrect:
circle.get_radius()

// Correct:
circle.radius()
```

#### Prefer positive booleans

When creating a boolean variable, prefer giving it a positive meaning to
prevent [double negatives](https://refactoring.com/catalog/removeDoubleNegative.html).

This rule also applies to CLI switches and environment variables.

```rust
// Incorrect:
let skip_update: bool;

// Correct:
let update: bool;

// Also correct, preferable if e.g. the `update` is already used by a function:
let should_update: bool;
```

#### Enum variants

When implementing or modifying an `enum` type, list its variants in alphabetical
order. It's acceptable to ignore this advice when matching the order imposed by
an external source, e.g. a standards document.

Prefer active verbs for variant names. E.g. `Allow` instead of `Allowed`,
`Forbid` instead of `Forbidden`. Avoid faux-bools like `Yes` and `No`, instead
preferring variant names that are descriptive of the different states.

#### Don't elide generic lifetimes

We prefer not to elide lifetimes when naming types that are generic over
lifetimes. Always include a lifetime placeholder (e.g. `<'_>`) to avoid
confusion.

### Imports

In each file the imports should be grouped into at most 4 groups in the
following order:

1. stdlib
2. non-repository local crates
3. repository local other crates
4. this crate

```rust
// Incorrect:
use alloc::format;
use alloc::vec::Vec;

// Correct:
use alloc::{format, vec::Vec};
```

Separate each group with a blank line, and rustfmt will sort into a canonical
order. Any file that is not grouped like this can be rearranged whenever the
file is touched. When you think this should be done for an existing file,
please do it at the beginning of your PR in a separate commit.

We prefer to reference types and traits by an imported symbol name instead of
using qualified references. Qualification paths generally add noise and are
unnecessary. The one exception to this is when the symbol name is overly
generic, or easily confused between different crates. In this case we prefer to
import the symbol name under an alias, or if the parent module name is short,
using a one-level qualified path. E.g. for a crate with a local `Error` type,
prefer to `import std::error::Error as StdError`.

### Exports

We prefer to export types under a single name, avoiding re-exporting types from
the top-level `lib.rs`. The exception to this are "paved path" exports that we
expect every user will need. The canonical example of such types are
`client::ClientConfig` and `server::ServerConfig`. In general this sort of type
is rare and most new types should be exported only from the module in which they
are defined.

### Misc

#### Numeric literals

Prefer a numeric base that fits with the domain of the value being used. E.g.
use hexadecimal for protocol message literals, and octal for UNIX privileges.
Use digit grouping to make larger numeric constants easy to read, e.g. use
`100_000_000` instead of `100000000`.

#### Avoid type aliases

We prefer to avoid type aliases as they obfuscate the underlying type and
don't provide additional type safety. Using the
[newtype idiom](https://doc.rust-lang.org/rust-by-example/generics/new_types.html)
is one alternative when an abstraction boundary is worth the added complexity.

#### No direct use of process state outside `rustup::process`

The `rustup::process` module abstracts the global state that is
`std::env::args`, `std::env::vars`, `std::io::std*` and `std::env::current_dir`
permitting threaded tests of the CLI logic; use the relevant methods of the
`rustup::process::Process` type rather than those APIs directly. Usually, a
`process: &Process` variable will be available to you in the current context.
For example, it could be in the form of a parameter of the current function, or
a field of a `Cfg` instance, etc.

## Writing tests

Rustup provides a number of test helpers in the `rustup::test` module
which is conditionally enabled with the `test` feature.

The existing tests under `tests/suite` provide good examples of how to use these
helpers, but you might also find it useful to look at the documentation for
particular APIs in the `rustup::test` module.

For example, for more information regarding end-to-end tests with the
`.expect()` APIs (e.g. how to generate/update the snapshots), please refer to
the documentation of the [`Assert`] type.

[`Assert`]: https://github.com/search?q=repo%3Arust-lang%2Frustup+symbol%3A%2F%28%3F-i%29Assert%2F&type=code

## Clippy lints

At the time of writing, rustup's CI pipeline runs clippy on both Windows and
Linux, but contributors to particularly OS-specific code should also make
sure that their clippy checking is done on that particular platform, as
OS-conditional code is a common source of unused imports and other small lints,
which can build up over time.

## Writing platform-specific code

If you are on Unix and would like to develop Windows-specific code
(`#[cfg(windows)]`), you can [check and lint your code
locally](linting.md#checking-windows-specific-code-on-unix) before pushing the
code and leaving the rest to our CI as long as the relevant test cases are in
place.

In the rare case where you would like to test Windows-specific behavior
yourself, you can use one of [Microsoft's developer VM
images](https://developer.microsoft.com/en-us/windows/downloads/virtual-machines/).

For developing Unix-specific code (`#[cfg(unix)]`) on Windows, it is
recommended to use
[WSL2](https://learn.microsoft.com/en-us/windows/wsl/install) for a full Linux
environment.
