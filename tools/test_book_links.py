"""Regression checks for the built-book link validator."""

import runpy
from pathlib import Path

check_links = runpy.run_path(str(Path(__file__).with_name("check-book-links.py")))[
    "check_links"
]


def page(root, name, content):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def test_relative_links_assets_fragments_and_external_urls(tmp_path):
    page(
        tmp_path,
        "chapter/index.html",
        '<a href="../index.html#first%20section">Home</a>'
        '<a href="#local">Local</a><h1 id="local">Chapter</h1>'
        '<img src="../image.svg"><a href="https://example.com/">External</a>'
        '<a href="mailto:reader@example.com">Email</a>',
    )
    page(tmp_path, "index.html", '<h1 id="first section">Home</h1>')
    page(tmp_path, "image.svg", "<svg/>")
    assert check_links(tmp_path) == []


def test_missing_file_and_fragment_fail(tmp_path):
    page(
        tmp_path,
        "index.html",
        '<a href="missing.html">Missing</a><a href="#missing">Fragment</a>',
    )
    errors = check_links(tmp_path)
    assert len(errors) == 2
    assert "missing local target" in errors[0]
    assert "missing HTML fragment" in errors[1]


def test_links_cannot_escape_the_book(tmp_path):
    page(tmp_path, "index.html", '<a href="../outside.html">Outside</a>')
    assert len(check_links(tmp_path)) == 1


def test_empty_book_fails(tmp_path):
    assert len(check_links(tmp_path)) == 1


def test_site_root_links_use_the_configured_pages_prefix(tmp_path):
    page(tmp_path, "index.html", '<a href="/RustiQ/">Home</a>')
    assert check_links(tmp_path, "/RustiQ/") == []
    page(tmp_path, "index.html", '<a href="/another-site/">Outside</a>')
    assert len(check_links(tmp_path, "/RustiQ/")) == 1
