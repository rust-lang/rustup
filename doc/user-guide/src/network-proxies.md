# Network proxies

Enterprise networks often don't have direct outside HTTP access, but enforce
the use of proxies. If you're on such a network, you can request that `rustup`
uses a proxy by setting its URL in the environment. In most cases, setting
`https_proxy` should be sufficient. Commands may differ between different
systems and shells:

 - On a Unix-like system with a shell like __bash__ or __zsh__:  
   ```bash
   export https_proxy=socks5://proxy.example.com:1080
   ```
 - On Windows [__Command Prompt (cmd)__][cmd]:  
   ```cmd
   set https_proxy=socks5://proxy.example.com:1080
   ```
 - On Windows [__PowerShell__][ps] (or __PowerShell Core__):  
   ```cmd
   $env:https_proxy="socks5://proxy.example.com:1080"
   ```
 - Replace `socks5://proxy.example.com:1080` with 
  `http://proxy.example.com:8080` when an HTTP proxy is used instead.

If you need a more complex setup, `rustup` supports the convention used by the
__curl__ program, documented in the ENVIRONMENT section of [its manual
page][curlman].

The use of `curl` is presently **deprecated**, however it can still be used by
providing the `RUSTUP_USE_CURL` environment variable, for example:

```bash
RUSTUP_USE_CURL=1 rustup update
```

Note that some versions of `libcurl` apparently require you to drop the
`http://` or `https://` prefix in environment variables. For example, `export
http_proxy=proxy.example.com:1080` (and likewise for HTTPS). If you are
getting an SSL `unknown protocol` error from `rustup` via `libcurl` but the
command-line `curl` command works fine, this may be the problem.

[curlman]: https://curl.se/docs/manpage.html#:~:text=Environment,-The%20environment%20variables
[cmd]: https://en.wikipedia.org/wiki/Cmd.exe
[ps]: https://en.wikipedia.org/wiki/PowerShell
