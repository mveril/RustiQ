"""Regression checks for the built-book link validator."""

import runpy
import tempfile
import unittest
from pathlib import Path

check_links = runpy.run_path(str(Path(__file__).with_name("check-book-links.py")))[
    "check_links"
]


class BookLinksTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)


    def page(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


    def test_relative_links_assets_fragments_and_external_urls(self):
        self.page(
            "chapter/index.html",
            '<a href="../index.html#first%20section">Home</a>'
            '<a href="#local">Local</a><h1 id="local">Chapter</h1>'
            '<img src="../image.svg"><a href="https://example.com/">External</a>'
            '<a href="mailto:reader@example.com">Email</a>',
        )
        self.page("index.html", '<h1 id="first section">Home</h1>')
        self.page("image.svg", "<svg/>")
        self.assertEqual(check_links(self.root), [])


    def test_missing_file_and_fragment_fail(self):
        self.page(
            "index.html",
            '<a href="missing.html">Missing</a><a href="#missing">Fragment</a>',
        )
        errors = check_links(self.root)
        self.assertEqual(len(errors), 2)
        self.assertIn("missing local target", errors[0])
        self.assertIn("missing HTML fragment", errors[1])


    def test_links_cannot_escape_the_book(self):
        self.page("index.html", '<a href="../outside.html">Outside</a>')
        self.assertEqual(len(check_links(self.root)), 1)


    def test_empty_book_fails(self):
        self.assertEqual(len(check_links(self.root)), 1)


    def test_site_root_links_use_the_configured_pages_prefix(self):
        self.page("index.html", '<a href="/RustiQ/">Home</a>')
        self.assertEqual(check_links(self.root, "/RustiQ/"), [])
        self.page("index.html", '<a href="/another-site/">Outside</a>')
        self.assertEqual(len(check_links(self.root, "/RustiQ/")), 1)


if __name__ == "__main__":
    unittest.main()
