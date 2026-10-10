"""Check local links and fragments in an already-built mdBook (no network)."""

import argparse
import tomllib
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


class Page(HTMLParser):
    def __init__(self, path: Path):
        super().__init__()
        self.ids: set[str] = set()
        self.links: list[str] = []
        self.feed(path.read_text(encoding="utf-8"))

    def handle_starttag(self, tag, attrs):
        for key, value in attrs:
            if value is None:
                continue
            if key == "id" or (tag == "a" and key == "name"):
                self.ids.add(value)
            if key in {"href", "src"}:
                self.links.append(value)


def check_links(root: Path, site_url: str = "/") -> list[str]:
    root = root.resolve()
    pages = {path: Page(path) for path in root.rglob("*.html")}
    if not pages:
        return [f"No HTML pages found in {root}"]
    errors = []
    for source, page in pages.items():
        for link in page.links:
            url = urlsplit(link)
            if url.scheme or url.netloc:
                continue
            path = unquote(url.path)
            if path.startswith("/"):
                if not path.startswith(site_url):
                    errors.append(
                        f"{source.relative_to(root)}: outside configured site {link}"
                    )
                    continue
                target = (root / path.removeprefix(site_url)).resolve()
            else:
                target = (source.parent / path).resolve() if path else source
            if target.is_dir():
                target /= "index.html"
            if not target.is_relative_to(root) or not target.is_file():
                errors.append(
                    f"{source.relative_to(root)}: missing local target {link}"
                )
            elif (
                target in pages
                and url.fragment
                and unquote(url.fragment) not in pages[target].ids
            ):
                errors.append(
                    f"{source.relative_to(root)}: missing HTML fragment {link}"
                )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("book", type=Path)
    parser.add_argument("--config", type=Path, help="mdBook configuration for site-url")
    args = parser.parse_args()
    site_url = "/"
    if args.config:
        config = tomllib.loads(args.config.read_text(encoding="utf-8"))
        site_url = config.get("output", {}).get("html", {}).get("site-url", "/")
    errors = check_links(args.book, site_url)
    if errors:
        print("\n".join(errors))
        return 1
    print("All local book links and HTML fragments resolve.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
