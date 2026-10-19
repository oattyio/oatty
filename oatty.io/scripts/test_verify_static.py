"""Exercise the narrow clipboard exception and canonical prompt contract."""

from contextlib import redirect_stdout
from html import escape
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("verify_static", Path(__file__).with_name("verify-static.py"))
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)
SCRIPT = '<script src="/copy-prompt.js" defer></script>'
PROMPT = "Set up Oatty.\nCheck <expected> & actual results.\n"


def inspect_markup(markup: str, route: str = "/") -> list[str]:
    """Return script and inline behavior violations without requiring a build."""
    inspector = VERIFIER.PageInspector(route)
    inspector.feed(markup)
    inspector.close()
    return inspector.violations


def page(route: str, script: str = SCRIPT, prompt: str = PROMPT) -> str:
    """Make a complete fixture page with decoded pre/code prompt text."""
    return (
        '<!doctype html><html><head><title>Test</title>'
        '<meta name="description" content="Test page">'
        f'<link rel="canonical" href="https://oatty.io{route}">{script}'
        f'</head><body><main><h1>Test</h1><pre><code data-prompt-text>{escape(prompt)}</code></pre>'
        '</main></body></html>'
    )


class ScriptPolicyTests(unittest.TestCase):
    def test_exact_local_deferred_script_on_only_prompt_routes(self) -> None:
        for route in ("/", "/docs/api-access/"):
            with self.subTest(route=route):
                self.assertEqual(inspect_markup(SCRIPT, route), [])
        self.assertEqual(inspect_markup('<script defer src="/copy-prompt.js"></script>'), [])
        self.assertIn("unapproved script element", inspect_markup(SCRIPT, "/docs/quick-start/"))

    def test_rejects_script_variants_and_duplicate_attributes(self) -> None:
        scripts = (
            '<script>alert(1)</script>',
            '<script src="https://example.com/copy-prompt.js" defer></script>',
            '<script src="//oatty.io/copy-prompt.js" defer></script>',
            '<script src="/copy-prompt.js?version=1" defer></script>',
            '<script src="/copy-prompt.js#copy" defer></script>',
            '<script src="/other/../copy-prompt.js" defer></script>',
            '<script src="/%63opy-prompt.js" defer></script>',
            '<script src="copy-prompt.js" defer></script>',
            '<script src="/copy-prompt.js"></script>',
            '<script src="/copy-prompt.js" defer="defer"></script>',
            '<script src="/copy-prompt.js" defer async></script>',
            '<script src="/copy-prompt.js" defer type="module"></script>',
            '<script src="https://example.com/evil.js" src="/copy-prompt.js" defer></script>',
            '<script src="/copy-prompt.js" defer defer></script>',
        )
        for script in scripts:
            with self.subTest(script=script):
                self.assertIn("unapproved script element", inspect_markup(script))

    def test_rejects_extra_self_closing_and_inline_script_content(self) -> None:
        self.assertIn("extra script element", inspect_markup(SCRIPT + SCRIPT))
        self.assertIn("self-closing script element", inspect_markup('<script src="/copy-prompt.js" defer/>'))
        for content in ("alert(1)", " ", "<!-- comment -->"):
            with self.subTest(content=content):
                markup = SCRIPT.replace("></script>", f">{content}</script>")
                self.assertIn("inline script content", inspect_markup(markup))

    def test_keeps_rejecting_handlers_and_javascript_urls(self) -> None:
        self.assertIn("event handler: onclick", inspect_markup('<button onclick="copy()">Copy</button>'))
        self.assertIn("javascript URL: href", inspect_markup('<a href=" java\nscript:alert(1)">Copy</a>'))


class PublishedPromptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.site = Path(self.temporary.name)
        self.output = self.site / "dist"
        self.write("public/setup-prompt.txt", PROMPT)
        self.write("public/copy-prompt.js", "// clipboard-only fixture\n")
        self.write("dist/setup-prompt.txt", PROMPT)
        self.write("dist/copy-prompt.js", "// clipboard-only fixture\n")
        self.write_pair("index.html", "index.html", page("/"))
        self.write_pair("static-docs/api-access/index.html", "docs/api-access/index.html", page("/docs/api-access/"))
        self.write_pair("static-pages/404.html", "404.html", '<title>Missing</title><main><h1>Missing</h1></main>')
        patcher = patch.multiple(VERIFIER, SITE=self.site, OUTPUT=self.output,
                                 REQUIRED_ROUTES=("/", "/docs/api-access/", "/404.html"))
        patcher.start()
        self.addCleanup(patcher.stop)

    def write(self, relative: str, content: str) -> None:
        target = self.site / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")

    def write_pair(self, source: str, target: str, content: str) -> None:
        self.write(source, content)
        self.write("dist/" + target, content)

    def verify(self) -> None:
        with redirect_stdout(io.StringIO()):
            VERIFIER.verify_site()

    def test_accepts_identical_decoded_prompt_and_asset_bytes(self) -> None:
        self.verify()

    def test_rejects_displayed_prompt_drift_even_when_page_output_matches(self) -> None:
        self.write_pair("index.html", "index.html", page("/", prompt=PROMPT.rstrip()))
        with self.assertRaisesRegex(SystemExit, "setup prompt differs from canonical text"):
            self.verify()

    def test_rejects_missing_prompt_or_script(self) -> None:
        for markup, expected in ((page("/", prompt=PROMPT).replace("data-prompt-text", "data-other"), "complete setup prompt"),
                                 (page("/", script=""), "one deferred copy-prompt script")):
            with self.subTest(expected=expected):
                self.write_pair("index.html", "index.html", markup)
                with self.assertRaisesRegex(SystemExit, expected):
                    self.verify()

    def test_rejects_published_asset_drift(self) -> None:
        for asset in VERIFIER.PROMPT_ASSETS:
            with self.subTest(asset=asset):
                original = (self.output / asset).read_text()
                self.write("dist/" + asset, original + "drift")
                with self.assertRaisesRegex(SystemExit, "prompt asset differs from source"):
                    self.verify()
                self.write("dist/" + asset, original)

    def test_rejects_missing_asset_sources_and_targets(self) -> None:
        for directory in ("public", "dist"):
            for asset in VERIFIER.PROMPT_ASSETS:
                with self.subTest(directory=directory, asset=asset):
                    target = self.site / directory / asset
                    original = target.read_bytes()
                    target.unlink()
                    with self.assertRaisesRegex(SystemExit, "missing .*prompt asset"):
                        self.verify()
                    target.write_bytes(original)

    def test_rejects_missing_authored_prompt_page(self) -> None:
        (self.site / "static-docs/api-access/index.html").unlink()
        with self.assertRaisesRegex(SystemExit, "missing authored prompt page"):
            self.verify()

    def test_rejects_extra_javascript_outputs(self) -> None:
        for name in ("extra.js", "nested/copy-prompt.js", "extra.mjs", "extra.cjs", "extra.JS"):
            with self.subTest(name=name):
                self.write("dist/" + name, "// extra\n")
                with self.assertRaisesRegex(SystemExit, "JavaScript resource in published site"):
                    self.verify()
                (self.output / name).unlink()

    def test_requires_exactly_one_terminal_lf(self) -> None:
        for prompt in (PROMPT.rstrip(), PROMPT + "\n", PROMPT.replace("\n", "\r\n"), "\n"):
            with self.subTest(prompt=repr(prompt)):
                self.write("public/setup-prompt.txt", prompt)
                with self.assertRaisesRegex(SystemExit, "exactly one terminal LF"):
                    self.verify()


if __name__ == "__main__":
    unittest.main()
