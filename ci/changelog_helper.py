import json
import re
import subprocess
import sys

USAGE = """
Usage:
  python changelog_helper.py replace-nums CHANGELOG_MARKDOWN
    Replace Rustup PR numbers or links with `[pr#1234]`, moving the actual links to the bottom

  python changelog_helper.py usernames GITHUB_GENERATED_CHANGELOG
    Generate a Markdown list of contributors to be pasted below the line `Thanks go to:`
    A logged-in GitHub CLI (https://cli.github.com) is required for this subcommand
    For a GitHub-generated changelog, see https://github.com/rust-lang/rustup/releases/new
"""

BOTS = {"renovate": "Renovate Bot"}


def extract_usernames(text):
    return sorted(
        set(re.findall(r"@([\w-]+)", text)),
        key=lambda name: (name in BOTS, str.casefold(name)),
    )


def github_name(username):
    # url = f"https://api.github.com/users/{username}"
    # response = urlopen(url)
    if username in BOTS:
        return BOTS[username]
    try:
        response = subprocess.check_output(
            [
                "gh",
                "api",
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                "X-GitHub-Api-Version: 2022-11-28",
                f"/users/{username}",
            ]
        )
        data = json.loads(response)
        return data["name"] or username
    except Exception as e:
        print("An error occurred:", str(e))


def read_file(file_name):
    try:
        with open(file_name, "r") as file:
            return file.read()
    except FileNotFoundError:
        print("File not found")
    except Exception as e:
        print("An error occurred:", str(e))


def help():
    print(USAGE)
    sys.exit(1)


def main():
    if len(sys.argv) < 3:
        help()

    _, subcmd, file_name = sys.argv[:3]

    if subcmd == "usernames":
        content = read_file(file_name)
        if not content:
            return
        for username in extract_usernames(content):
            print(f"- {github_name(username)} ({username})")
    elif subcmd == "replace-nums":
        content = read_file(file_name)
        footer = ""
        if not content:
            return
        issue_pat = r"(#|https://github\.com/rust-lang/rustup/pull/)(\d+)"
        for prefix, num in re.findall(issue_pat, content):
            link = f"[pr#{num}]"
            footer += f"{link}: https://github.com/rust-lang/rustup/pull/{num}\n"
            content = content.replace(prefix + num, link)
        print(f"{content}\n{footer}")
    else:
        help()


if __name__ == "__main__":
    main()
