import json
import re
import subprocess
import sys


def extract_usernames(text):
    return sorted(set(re.findall(r"@([\w-]+)", text)), key=str.casefold)


def github_name(username):
    # url = f"https://api.github.com/users/{username}"
    # response = urlopen(url)
    if username == "renovate":
        return "Renovate Bot"
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
        return data["name"]
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
    print("Usage:")
    print("  python changelog_helper.py usernames GITHUB_GENERATED_CHANGELOG")
    print("  python changelog_helper.py replace-nums CHANGELOG_MARKDOWN")
    print()
    print(
        "A logged-in GitHub CLI (https://cli.github.com) is required for the `usernames` subcommand"
    )
    print(
        "For a GitHub-generated changelog, see https://github.com/rust-lang/rustup/releases/new"
    )
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
        for match in re.findall(r"(?<=#)(\d+)", content):
            # Replace issue number with fully-qualified link
            link = f"[pr#{match}]"
            footer += f"{link}: https://github.com/rust-lang/rustup/pull/{match}\n"
            content = content.replace(f"#{match}", link)
        print(f"{content}\n{footer}")
    else:
        help()


if __name__ == "__main__":
    main()
