#!/usr/bin/env python3
"""Exercise a built manual through file URLs with browser networking disabled.

Requires Playwright and its Chromium browser (verification tools only).
"""

import argparse
import shutil
import tempfile
from pathlib import Path

from playwright.sync_api import sync_playwright


def check(book: Path) -> None:
    book = book.resolve()
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        context = browser.new_context(offline=True)
        remote_requests = []
        errors = []
        failed_requests = []
        context.on("requestfailed", lambda request: failed_requests.append(request.url))
        context.on(
            "request",
            lambda request: (
                remote_requests.append(request.url)
                if request.url.startswith(("https://", "http://"))
                else None
            ),
        )
        page = context.new_page()
        page.on("pageerror", lambda error: errors.append(str(error)))
        page.goto((book / "index.html").as_uri())
        assert page.locator("h1").first.is_visible(), "manual index did not render"
        assert page.evaluate("document.styleSheets.length") > 0, "styles are missing"
        page.goto((book / "scientific-scope.html").as_uri())
        page.wait_for_selector(".MathJax_SVG svg")
        page.goto((book / "user-guide/calculation-inputs.html").as_uri())
        page.wait_for_selector(".mermaid svg")
        assert not remote_requests, f"remote runtime requests: {remote_requests}"
        assert not failed_requests, f"failed local resource requests: {failed_requests}"
        assert not errors, f"browser JavaScript errors: {errors}"
        browser.close()
    print(
        "Offline file navigation, styling, SVG equations, and Mermaid rendering passed."
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("book", type=Path)
    with tempfile.TemporaryDirectory(prefix="rustiq relocated manual ") as temporary:
        relocated = Path(temporary) / "manual"
        shutil.copytree(parser.parse_args().book, relocated)
        check(relocated)
