# Network proxies

Enterprise networks often don't have direct outside HTTP access, but enforce
the use of proxies. If you're on such a network, you can request that `rustup`
uses a proxy by setting its URL in the environment. In most cases, setting
`https_proxy` should be sufficient. Commands may differ between different
systems and shells:

- On a Unix-like system with a shell like **bash** or **zsh**:
  ```bash
  export https_proxy=socks5://proxy.example.com:1080
  ```
- On Windows [**Command Prompt (cmd)**][cmd]:
  ```cmd
  set https_proxy=socks5://proxy.example.com:1080
  ```
- On Windows [**PowerShell**][ps] (or **PowerShell Core**):
  ```cmd
  $env:https_proxy="socks5://proxy.example.com:1080"
  ```
- Replace `socks5://proxy.example.com:1080` with
  `http://proxy.example.com:8080` when an HTTP proxy is used instead.

If you need a more complex setup, `rustup` supports the convention used by the
**curl** program, documented in the ENVIRONMENT section of [its manual
page][curlman].

[curlman]: https://curl.se/docs/manpage.html#:~:text=Environment,-The%20environment%20variables
[cmd]: https://en.wikipedia.org/wiki/Cmd.exe
[ps]: https://en.wikipedia.org/wiki/PowerShell
