# About the team

In response to the [call for chartering work] in the Rust 2021 roadmap, this
document sets out the charter for [t-rustup] ("the team"), which maintains
[rustup][rust-lang/rustup] ("the project").

[t-rustup]: https://rust-lang.org/governance/teams/dev-tools/#team-rustup
[call for chartering work]: https://github.com/rust-lang/rfcs/blob/main/text/3037-roadmap-2021.md#chartering-work-in-the-rust-project

## Mission

The team steers rustup's development and maintenance to ensure its long-term
sustainability and meet the needs of the Rust community.

The main goal of rustup is to provide a way to securely and reliably install
Rust toolchains for end users and automated environments such as CI systems,
while providing newcomers with a user-friendly starting point for using Rust.

### Responsibilities

The team is responsible for:

- Developing the rustup tool
- Maintaining the project's release process, in collaboration with [the release
  team][t-release]
- Maintaining the process for publishing Rust toolchains to the server from
  which rustup downloads them, in collaboration with [the bootstrap
  team][t-bootstrap]

### Scope

The team aims to make rustup cross-platform without imposing an unreasonable
maintenance burden. As such, its technical work is defined within Rust's
official [platform support] and [target tier] policies.

For host platforms in particular, the team guarantees support for all tier 1
platforms and provides best-effort support for tier 2 platforms with host
tools. Other host platforms may also be supported with no guarantees being
made.

### Community building

The team aims to attract new contributors to open-source development by
improving the developer experience of the project, which may also make rustup a
gateway to other Rust-related open-source projects.

[rust-lang/rustup]: https://github.com/rust-lang/rustup
[t-bootstrap]: https://rust-lang.org/governance/teams/infra/#team-bootstrap
[t-release]: https://rust-lang.org/governance/teams/infra/#team-release
[platform support]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[target tier]: https://doc.rust-lang.org/nightly/rustc/target-tier-policy.html

## Membership

The team has one or two leads, selected by consensus of the team (i.e. without
objections).

The team leads are responsible for coordinating the team, representing it to
other stakeholders, and making final decisions in case of a
[deadlock](#decision-making).

Current team members and leads are listed in the [t-rustup section][t-rustup]
of the Rust governance website.

### Privileges

Team members have the following privileges to facilitate their maintenance activities:

- GitHub permissions: for managing issues, editing milestones, merging pull
  requests, etc.
- [Voting and decision making](#decision-making), with external voices and
  concerns welcome but voting restricted to team members
- Access to private communications related to sensitive topics

### Joining the team

New members are invited by existing members, based on their past contributions
to the project, understanding of its technical context, and interest in its
regular maintenance.

There is currently no formal onboarding process. New members are encouraged to
continue contributing while meeting the team's [membership
expectations](#membership-expectations).

### Membership expectations

Members are expected to:

- Participate in the [decision making process](#decision-making).

- Regularly participate in at least some membership-related activities.
  Participation in all of them is not required, but members should demonstrate
  interest and involvement in areas such as:

  - Triaging and responding to issues
  - Reviewing contributions
  - Keeping up with regular maintenance of the project
  - Shepherding major changes
  - Mentoring new contributors
  - Managing the team's policies
  - Coordinating with stakeholders

- As a member of the Rust project:

  - Represent Rust in a way that upholds the Rust code of conduct to a high
    standard.
  - Represent rustup in a way that upholds the expectations of this charter,
    and be friendly, welcoming, and constructive with contributors and users.

### Breaks and vacations

Breaks and vacations are welcome and encouraged, and informing the team in
advance is greatly appreciated.

A member who is absent from their [expected
activities](#membership-expectations) for an extended period without notice may
be asked to [step down](#leaving-the-team).

### Leaving the team

Members may leave the team at any time, preferably by informing the team ahead
of time. After a member leaves, they will be moved to the alumni list in the
[`t-rustup` section][t-rustup] of the Rust governance website.

The team acknowledges former members for their contributions, and they are
welcome to return to the team upon consensus of the current team.

## Team resources

### GitHub

The team has permissions in several GitHub repositories:

- [rust-lang/rustup]: The primary repository for rustup development.

- [rust-lang/rust]: The primary repository for Rust releases. The team
  can help maintain the toolchain release process, notably its
  bootstrap-related parts in collaboration with [t-bootstrap].

- [rust-lang/rustup-components-history]: The code for the nightly component
  availability [dashboard][rustup-components-history]. The team can help the
  dashboard keep up with changes in Rust's release process such as new
  component deprecation notices or target tier policy updates.

[rust-lang/rust]: https://github.com/rust-lang/rust
[rust-lang/rustup-components-history]: https://github.com/rust-lang/rustup-components-history
[rustup-components-history]: https://rust-lang.github.io/rustup-components-history

### Zulip

The team primarily communicates in the public [#t-rustup] channel.

The team may use the private [#t-rustup/private] channel for sensitive
discussions, such as personal matters or security emergencies. Most discussions
should take place in the public channel.

[#t-rustup]: https://rust-lang.zulipchat.com/#narrow/channel/490103-t-rustup
[#t-rustup/private]: https://rust-lang.zulipchat.com/#narrow/channel/504306-t-rustup.2Fprivate

### Blogs

The team is welcome to post on the two official blogs: ["Main"] and ["Inside
Rust"].

The [blogs policy] explains how to choose between them and provides guidance
for each one.

["Main"]: https://blog.rust-lang.org/
["Inside Rust"]: https://blog.rust-lang.org/inside-rust
[blogs policy]: https://forge.rust-lang.org/platforms/blogs.html

### Travel grants

The team is welcome to attend conferences and other events to represent the
project, coordinate in-depth offline discussions, and share experiences with
the rest of the Rust community.

Travel grants are provided by the Leadership Council for this purpose. See the
[travel policy] for more information.

[travel policy]: https://github.com/rust-lang/leadership-council/blob/main/policies/spending/travel.md

## Decision making

> This process reflects the team's small size (4 members as of November 2026).
> If the team grows or circumstances change, it may adopt escalation mechanisms
> such as the FCP/RFC processes used elsewhere in the Rust project.

Minor changes may be decided by any team member independently. Examples
include:

- Non-breaking bugfixes
- Simple refactoring changes with minor overall impact on the codebase
- User-facing diagnostic changes, e.g. the addition of new warnings
- Documentation updates

Moderate changes are approved by at least two team members, with the original
author counting as one approval if they are on the team. Examples include:

- Breaking bugfixes, e.g. changes to how existing commands or flags interact
- Major extensions to rustup's accepted inputs, particularly changes to
  `rustup[-init]`'s CLI interface, e.g. new (sub)commands, arguments, flags, or
  configuration options
- Breaking changes to machine-facing rustup outputs, especially those with a
  high likelihood of breaking downstream community-maintained parsers (e.g. from
  IDEs, build systems, and CI scripts)
- Changes to binary proxies or their installation, e.g. adding a new binary
  proxy for some toolchain component

Major changes are approved by the majority of the team as a whole, with the
team leads able to resolve ties. If the leads disagree when resolving a tie,
the status quo is maintained while the issue may be escalated to an appropriate
higher-level governance body. Examples include:

- Fundamental changes to the team's processes, e.g. revising the
  decision-making policy defined in this charter
- Fundamental changes to how rustup works, e.g redesigning the setup flow,
  the filesystem layout, or the toolchain installation mechanisms
- Fundamental changes to how rustup is built, e.g. introducing breaking changes
  or complex refactors to the build process or the CI/CD pipeline

Where possible, appropriate external stakeholders are consulted and decisions
are guided by their feedback.

For example, changes to the release server's layout or manifest format that
require a corresponding rustup change constitute major changes for the team to
evaluate. The team should consult [t-bootstrap] to ensure compatibility with
the latter's expectations and processes. When the teams disagree or the
ownership of the changes remains unclear, they should share the responsibility
for these changes, escalating to an appropriate higher-level governance body if
necessary.

## Contacting the team

The team may be contacted through several channels:

- For security concerns, please refer to Rust's [security policy].
- For issues and feature requests, submit through the project's [issue tracker].
- For general questions or requests, post in the public [#t-rustup] channel on Zulip.

[issue tracker]: https://github.com/rust-lang/rustup/issues
[security policy]: https://www.rust-lang.org/security
