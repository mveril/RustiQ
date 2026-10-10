"""Regression checks for the built-book link validator."""

import runpy
from pathlib import Path

import pytest

check_links = runpy.run_path(str(Path(__file__).with_name("check-book-links.py")))[
    "check_links"
]


@pytest.fixture
def book_root(tmp_path):
    return tmp_path


def page(root, name, content):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def test_relative_links_assets_fragments_and_external_urls(book_root):
    page(
        book_root,
        "chapter/index.html",
        '<a href="../index.html#first%20section">Home</a>'
        '<a href="#local">Local</a><h1 id="local">Chapter</h1>'
        '<img src="../image.svg"><a href="https://example.com/">External</a>'
        '<a href="mailto:reader@example.com">Email</a>',
    )
    page(book_root, "index.html", '<h1 id="first section">Home</h1>')
    page(book_root, "image.svg", "<svg/>")
    assert check_links(book_root) == []


def test_missing_file_and_fragment_fail(book_root):
    page(
        book_root,
        "index.html",
        '<a href="missing.html">Missing</a><a href="#missing">Fragment</a>',
    )
    errors = check_links(book_root)
    assert len(errors) == 2
    assert "missing local target" in errors[0]
    assert "missing HTML fragment" in errors[1]


def test_links_cannot_escape_the_book(book_root):
    page(book_root, "index.html", '<a href="../outside.html">Outside</a>')
    assert len(check_links(book_root)) == 1


def test_empty_book_fails(book_root):
    assert len(check_links(book_root)) == 1


def test_site_root_links_use_the_configured_pages_prefix(book_root):
    page(book_root, "index.html", '<a href="/RustiQ/">Home</a>')
    assert check_links(book_root, "/RustiQ/") == []
    page(book_root, "index.html", '<a href="/another-site/">Outside</a>')
    assert len(check_links(book_root, "/RustiQ/")) == 1
