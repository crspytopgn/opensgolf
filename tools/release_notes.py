#!/usr/bin/env python3
"""Release notes from CHANGELOG.md, for the GitHub release page and the Discord #releases post.

    release_notes.py github  v0.9.5 [CHANGELOG.md]   prints the GitHub release body (Markdown)
    release_notes.py discord v0.9.5 [CHANGELOG.md]   prints the Discord webhook payload (JSON)

A minor release (0.9 or 0.9.0) shows its "## 0.9 (...)" highlights. A fix release (0.9.5) shows its own changes first:
its "## 0.9.5 (...)" section, or else its "- **0.9.5:** ..." line under the minor's *Fix releases*. The GitHub page then
adds the minor's highlights below them; the Discord post links to them instead, so it stays about what is new.
Used by .github/workflows/release.yml (on each release) and release-notes.yml (to redo an older release's notes).
"""
import json
import re
import sys

REPO = "https://github.com/crspytopgn/opensgolf"


def section(text, ver):
    """The "## <ver> (...)" section up to the next "## " heading, or ""."""
    m = re.search(r"^## " + re.escape(ver) + r" \(.*?(?=^## |\Z)", text, re.S | re.M)
    return m.group(0).strip() if m else ""


def fix_line(minor_section, ver):
    """The "- **<ver>:** ..." bullet (with its wrapped lines) from a minor's Fix releases list, without the label."""
    lines = minor_section.splitlines()
    for i, line in enumerate(lines):
        m = re.match(r"^- \*\*" + re.escape(ver) + r":\*\* ?(.*)", line)
        if m:
            out = [m.group(1)]
            for nxt in lines[i + 1:]:
                if nxt.startswith(" ") and nxt.strip():
                    out.append(nxt.strip())
                else:
                    break
            return " ".join(out).strip()
    return ""


def is_fix(v):
    return v.split(".")[2:] not in ([], ["0"])


def notes(v, text):
    """(this version's own notes as Markdown, the minor's highlights section, the minor) for version v ("0.9.5")."""
    minor = ".".join(v.split(".")[:2])
    minor_sec = section(text, minor)
    if not is_fix(v):
        return "", minor_sec, minor
    own = section(text, v)
    if not own:
        line = fix_line(minor_sec, v)
        if line:
            own = f"## {v}\n\n- {line[:1].upper()}{line[1:]}"
    return own, minor_sec, minor


def absolute(md):
    md = md.replace("](docs/", f"]({REPO}/blob/main/docs/")
    return re.sub(r"\]\((?!https?:|#)([^)]*)\)", rf"]({REPO}/blob/main/\1)", md)


def github(v, text):
    own, minor_sec, minor = notes(v, text)
    d = f"{REPO}/releases/download/v{v}/"
    # which file to pick, above the notes (the file names never change: Update now and the stores use them)
    body = (
        "### Which download do I need?\n\n"
        "| You have | Download |\n|---|---|\n"
        f"| Windows PC | [opensgolf-windows.zip]({d}opensgolf-windows.zip) |\n"
        f"| Windows on ARM (Snapdragon laptops) | [opensgolf-windows-arm64.zip]({d}opensgolf-windows-arm64.zip) |\n"
        f"| Mac (Apple silicon or Intel) | [opensgolf-macos.zip]({d}opensgolf-macos.zip) |\n"
        f"| Linux PC or Steam Deck | [opensgolf-linux-x86_64.AppImage]({d}opensgolf-linux-x86_64.AppImage) |\n"
        f"| Linux on ARM (Raspberry Pi 4/5) | [opensgolf-linux-arm64.AppImage]({d}opensgolf-linux-arm64.AppImage) |\n"
        f"| Android phone or tablet | [opensgolf-android.apk]({d}opensgolf-android.apk) |\n"
        "| Just want to try it | [Play in your browser](https://crspytopgn.github.io/opensgolf/), nothing to install |\n\n"
        "The Linux `.tar.gz` files are the same game as a plain folder, for when an AppImage won't start. "
        "`SHA256SUMS` and `SHA256SUMS.sig` let the game's *Update now* check what it downloads; you don't need them. "
        f"Install help: [INSTALL.md]({REPO}/blob/main/docs/INSTALL.md)\n\n"
    )
    if own:
        body += f"## What's new in {v}\n\n" + own.split("\n", 1)[1].strip() + "\n\n"
        if minor_sec:
            # the minor's highlights under a smaller heading, so this release's changes stay on top
            rest = minor_sec.split("\n", 1)[1].strip()
            rest = re.sub(r"^\*\*Highlights\*\*\s*", "", rest)
            body += f"## The {minor} highlights\n\n" + rest
    else:
        body += minor_sec
    body = absolute(body.strip())
    body += f"\n\nEverything, version by version: [CHANGELOG.md]({REPO}/blob/main/CHANGELOG.md)\n"
    body += "\nChat, help and bug reports: join the [OpenSGolf Discord](https://discord.gg/DVsBzGN4dC).\n"
    return body


def bullets(md, limit=300):
    """A section's bullets (nested ones indented) for Discord, each on one line and kept short, links made absolute."""
    items = []
    for line in md.splitlines():
        m = re.match(r"^( *)- (.*)", line)
        if m:
            items.append([len(m.group(1)) > 0, m.group(2)])
        elif items and line.startswith(" ") and line.strip():
            items[-1][1] += " " + line.strip()
    out = []
    for nested, b in items:
        b = absolute(b)
        if len(b) > limit:
            b = b[:limit - 3].rsplit(" ", 1)[0] + "…"
        out.append(("    ◦ " if nested else "• ") + b)
    return "\n".join(out)


def discord(v, text):
    own, minor_sec, minor = notes(v, text)
    url = f"{REPO}/releases/tag/v{v}"
    if own:
        # a fix release's own changes are the post's point: give them more room than a minor's highlights
        desc = bullets(own, limit=1200)
    else:
        desc = bullets(minor_sec)
        if is_fix(v):
            # a fix release with no notes of its own yet: say so, above the minor's highlights
            desc = f"An update to OpenSGolf {minor}. The {minor} highlights:\n\n" + desc
    if len(desc) > 3500:
        desc = desc[:3500].rsplit("\n", 1)[0] + "\n…"
    if own:
        anchor = re.sub(r"[^a-z0-9 -]", "", minor_sec.split("\n", 1)[0].lstrip("# ").lower()).replace(" ", "-")
        desc = f"What's new in {v}:\n\n" + desc
        desc += f"\n\nAn update to OpenSGolf {minor}: [the {minor} highlights]({REPO}/blob/main/CHANGELOG.md#{anchor})."
    desc += f"\n\n**[Download and full notes]({url})** · the game's *Check for updates* finds it too."
    return {
        "username": "OpenSGolf Releases",
        "allowed_mentions": {"parse": []},
        "embeds": [{"title": f"OpenSGolf {v} is out", "url": url, "description": desc, "color": 0x2F8F4E}],
    }


def main():
    mode, ver = sys.argv[1], sys.argv[2].lstrip("v")
    path = sys.argv[3] if len(sys.argv) > 3 else "CHANGELOG.md"
    text = open(path, encoding="utf-8").read()
    if mode == "github":
        sys.stdout.write(github(ver, text))
    elif mode == "discord":
        print(json.dumps(discord(ver, text), ensure_ascii=False))
    else:
        sys.exit("mode must be github or discord")


if __name__ == "__main__":
    main()
