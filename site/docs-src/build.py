#!/usr/bin/env python3
"""Builds the user documentation: wraps every page of pages/ in template.html and writes the
result to site/docs/. Standard library only, so it runs anywhere Python 3 does.

    python3 site/docs-src/build.py [OUTPUT_DIR]

Page format: a header of `key: value` lines (`title`, `description`), a `---` line, then the
page body in HTML. Inside the body:

- A fenced block, ```lang title="file name" ... ```, becomes a code panel with its content
  escaped. `lang` is one of the languages js/docs.js highlights (toml, shell, json) or `text`.
  The fences must start at the beginning of a line.
- An h2, h3 or h4 without an id gets one from its text, so it can be linked and searched.

The build fails, listing every problem, when a page in NAV has no source file, a source file is
not in NAV, an internal link or anchor does not exist, or a page contains an em dash.
"""
import html
import re
import sys
from pathlib import Path
from string import Template

HERE = Path(__file__).resolve().parent
SITE = HERE.parent
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else SITE / "docs"

# The sidebar, in order. It also sets the Previous and Next links and the breadcrumb.
NAV = [
    ("Getting started", [
        ("index.html", "Introduction"),
        ("installation.html", "Installation"),
        ("quick-start.html", "Quick start"),
        ("interface.html", "The interface"),
    ]),
    ("Guides", [
        ("playback.html", "Playback"),
        ("playlists.html", "Playlists"),
        ("equalizer.html", "The equalizer"),
        ("skins.html", "Skins"),
    ]),
    ("Reference", [
        ("keyboard-shortcuts.html", "Keyboard shortcuts"),
        ("settings.html", "Settings"),
        ("files-and-folders.html", "Files and folders"),
    ]),
    ("Help", [
        ("troubleshooting.html", "Troubleshooting"),
    ]),
]


def slugify(text):
    text = re.sub(r"<[^>]+>", "", text)
    text = html.unescape(text).lower()
    return re.sub(r"[^a-z0-9]+", "-", text).strip("-")


def code_panel(match):
    lang, title, code = match.group(1), match.group(2), match.group(3)
    attrs = f' data-lang="{lang}"' if lang and lang != "text" else ""
    head = ""
    if title:
        head = f'<div class="code-title"><span class="dot"></span>{html.escape(title)}</div>'
    return f'<div class="code">{head}<pre{attrs}>{html.escape(code, quote=False)}</pre></div>'


def render_body(body):
    body = re.sub(
        r'^```(\w*)(?: title="([^"]*)")?\n(.*?)\n```$', code_panel, body, flags=re.S | re.M
    )
    used = set(re.findall(r'id="([^"]+)"', body))

    def add_id(match):
        tag, attrs, inner = match.groups()
        if "id=" in attrs:
            return match.group(0)
        slug = base = slugify(inner)
        n = 2
        while slug in used:
            slug, n = f"{base}-{n}", n + 1
        used.add(slug)
        return f'<{tag}{attrs} id="{slug}">{inner}</{tag}>'

    # Not indented: indentation would end up inside <pre> blocks.
    return re.sub(r"<(h[234])([^>]*)>(.*?)</\1>", add_id, body, flags=re.S).strip()


def render_nav(current):
    lines = []
    for group, items in NAV:
        lines.append('            <div class="group">')
        lines.append(f"                <h2>{group}</h2>")
        lines.append("                <ul>")
        for file, label in items:
            mark = ' aria-current="page"' if file == current else ""
            lines.append(f'                    <li><a href="{file}"{mark}>{label}</a></li>')
        lines.append("                </ul>")
        lines.append("            </div>")
    return "\n".join(lines)


def render_pager(order, i):
    links = []
    if i > 0:
        file, label, _ = order[i - 1]
        links.append(f'                    <a class="prev" href="{file}"><small>Previous</small>{label}</a>')
    if i < len(order) - 1:
        file, label, _ = order[i + 1]
        links.append(f'                    <a class="next" href="{file}"><small>Next</small>{label}</a>')
    return "\n".join(links)


def check_links(pages, errors):
    """Every link to another docs page, and every #anchor, must exist."""
    ids = {name: set(re.findall(r'id="([^"]+)"', text)) for name, text in pages.items()}
    for name, text in pages.items():
        for href in re.findall(r'href="([^"]+)"', text):
            if href.startswith(("http:", "https:", "mailto:")):
                continue
            if href.startswith("../"):
                target = (SITE / href[3:].split("#")[0]).resolve()
                if not target.exists():
                    errors.append(f"{name}: broken link {href}")
                continue
            page, _, anchor = href.partition("#")
            if page == "":
                page = name
            elif page == "./":
                page = "index.html"
            if page not in pages:
                errors.append(f"{name}: broken link {href}")
            elif anchor and anchor not in ids[page]:
                errors.append(f"{name}: missing anchor {href}")
        for src in re.findall(r'src="\.\./([^"]+)"', text):
            if not (SITE / src).exists():
                errors.append(f"{name}: missing file ../{src}")


def main():
    template = Template((HERE / "template.html").read_text())
    order = [(file, label, group) for group, items in NAV for file, label in items]
    errors = []

    sources = {path.name for path in (HERE / "pages").glob("*.html")}
    listed = {file for file, _, _ in order}
    errors += [f"{file}: in NAV but there is no pages/{file}" for file in sorted(listed - sources)]
    errors += [f"pages/{file}: not listed in NAV" for file in sorted(sources - listed)]

    pages = {}
    for i, (file, label, group) in enumerate(order):
        if file not in sources:
            continue
        head, body = (HERE / "pages" / file).read_text().split("\n---\n", 1)
        meta = dict(line.split(": ", 1) for line in head.strip().split("\n"))
        page = template.substitute(
            title=meta.get("title", label),
            description=html.escape(meta["description"]),
            canonical="" if file == "index.html" else file,
            nav=render_nav(file),
            group=group,
            body=render_body(body),
            file=file,
            pager=render_pager(order, i),
        )
        if "\u2014" in page:
            errors.append(f"pages/{file}: contains an em dash, use a comma or a period")
        pages[file] = page

    check_links(pages, errors)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)

    OUT.mkdir(parents=True, exist_ok=True)
    for stale in OUT.glob("*.html"):
        if stale.name not in pages:
            stale.unlink()
    for file, page in pages.items():
        (OUT / file).write_text(page)
    print(f"Built {len(pages)} pages into {OUT}")


if __name__ == "__main__":
    main()
