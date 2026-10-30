"""Verify authored and generated HTML before publishing the static site."""

from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urljoin, urlsplit
import re


SITE = Path(__file__).resolve().parents[1]
OUTPUT = SITE / "dist"
ORIGIN = "https://oatty.io"
COPY_PROMPT_ROUTES = {"/", "/docs/api-access/"}
PROMPT_ASSETS = ("setup-prompt.txt", "copy-prompt.js")
REQUIRED_ROUTES = (
    "/", "/docs/", "/404.html",
    "/docs/api-access/", "/docs/agent-authored-schema/", "/docs/repair-command/",
    "/docs/quick-start/", "/docs/reference/cli-commands/",
    "/docs/reference/tui-interactions/", "/docs/learn/getting-oriented/",
    "/docs/learn/how-oatty-executes-safely/", "/docs/learn/library-and-catalogs/",
    "/docs/learn/mcp-http-server/", "/docs/learn/plugins/",
    "/docs/learn/search-and-run-commands/", "/docs/learn/workflows-basics/",
    "/docs/guides/access-review-collection-playbook/",
    "/docs/guides/credential-rotation-readiness-playbook/",
    "/docs/guides/sentry-bootstrap/",
    "/docs/guides/sentry-datadog-pagerduty-playbook/",
    "/docs/guides/vercel-to-render-migration-playbook/",
)


class PageInspector(HTMLParser):
    """Collect structure, metadata, links, identifiers, and script violations."""

    def __init__(self, public_url: str = "") -> None:
        super().__init__(convert_charrefs=True)
        self.public_url = public_url
        self.titles = 0
        self.headings = 0
        self.main_regions = 0
        self.descriptions: list[str] = []
        self.canonicals: list[str] = []
        self.identifiers: set[str] = set()
        self.references: list[tuple[str, str]] = []
        self.violations: list[str] = []
        self.script_count = 0
        self.prompt_texts: list[str] = []
        self._in_script = False
        self._in_pre = False
        self._prompt_index: int | None = None

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attributes = dict(attrs)
        if tag == "script":
            self.script_count += 1
            self._in_script = True
            if (self.public_url not in COPY_PROMPT_ROUTES or len(attrs) != 2
                    or attributes != {"src": "/copy-prompt.js", "defer": None}):
                self.violations.append("unapproved script element")
            if self.script_count > 1:
                self.violations.append("extra script element")
        if self._prompt_index is not None:
            self.violations.append("setup prompt must contain only text")
        if tag == "pre":
            self._in_pre = True
        if "data-prompt-text" in attributes:
            if tag != "code" or not self._in_pre or "hidden" in attributes:
                self.violations.append("setup prompt must be visible pre/code text")
            self.prompt_texts.append("")
            self._prompt_index = len(self.prompt_texts) - 1
        if tag == "title":
            self.titles += 1
        if tag == "h1":
            self.headings += 1
        if tag == "main":
            self.main_regions += 1
        if tag == "meta" and (attributes.get("name") or "").lower() == "description":
            self.descriptions.append(attributes.get("content") or "")
        if tag == "link" and "canonical" in (attributes.get("rel") or "").lower().split():
            self.canonicals.append(attributes.get("href") or "")
        if attributes.get("id"):
            self.identifiers.add(attributes["id"])
        for name, value in attrs:
            if name.startswith("on"):
                self.violations.append(f"event handler: {name}")
            if value is None:
                continue
            if "".join(value.split()).lower().startswith("javascript:"):
                self.violations.append(f"javascript URL: {name}")
            if name in {"href", "src", "poster", "action"}:
                self.references.append((name, value))
            if name == "srcset":
                self.references.extend((name, item.strip().split()[0]) for item in value.split(",") if item.strip())

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.handle_starttag(tag, attrs)
        if tag == "script":
            self.violations.append("self-closing script element")
        self.handle_endtag(tag)

    def handle_endtag(self, tag: str) -> None:
        if tag == "script":
            self._in_script = False
        if tag == "code":
            self._prompt_index = None
        if tag == "pre":
            self._in_pre = False

    def handle_data(self, data: str) -> None:
        if self._in_script and data:
            self.violations.append("inline script content")
        if self._prompt_index is not None:
            self.prompt_texts[self._prompt_index] += data


def inspect_prompt_assets(errors: list[str]) -> None:
    """Require canonical prompt and clipboard assets to match the published bytes."""
    for name in PROMPT_ASSETS:
        source, target = SITE / "public" / name, OUTPUT / name
        if not source.is_file():
            errors.append(f"{source}: missing prompt asset source")
        if not target.is_file():
            errors.append(f"{target}: missing published prompt asset")
        elif source.is_file() and source.read_bytes() != target.read_bytes():
            errors.append(f"{target}: prompt asset differs from source")
    source = SITE / "public/setup-prompt.txt"
    if source.is_file():
        prompt = source.read_bytes().decode("utf-8")
        if not prompt.strip() or not prompt.endswith("\n") or prompt.endswith("\n\n") or "\r" in prompt:
            errors.append(f"{source}: expected nonempty UTF-8 prompt with exactly one terminal LF and no CR")


def output_file_for_url(url: str) -> Path | None:
    """Resolve a site URL within the generated output directory."""
    path = (OUTPUT / unquote(urlsplit(url).path).lstrip("/")).resolve()
    if not path.is_relative_to(OUTPUT.resolve()):
        return None
    return path / "index.html" if path.is_dir() else path


def page_url(page_path: Path) -> str:
    """Convert a generated HTML path to its public URL."""
    relative = page_path.relative_to(OUTPUT).as_posix()
    if relative == "index.html":
        return "/"
    return "/" + relative.removesuffix("index.html") if relative.endswith("/index.html") else "/" + relative


def inspect_page(page_path: Path, public_url: str, errors: list[str]) -> None:
    """Check one page's structure, metadata, JavaScript, and local references."""
    inspector = PageInspector(public_url)
    inspector.feed(page_path.read_text(encoding="utf-8"))
    inspector.close()
    if public_url in COPY_PROMPT_ROUTES:
        if inspector.script_count != 1:
            errors.append(f"{page_path}: expected one deferred copy-prompt script")
        canonical = SITE / "public/setup-prompt.txt"
        if len(inspector.prompt_texts) != 1:
            errors.append(f"{page_path}: expected one complete setup prompt")
        elif canonical.is_file() and inspector.prompt_texts[0] != canonical.read_bytes().decode("utf-8"):
            errors.append(f"{page_path}: setup prompt differs from canonical text")
    if (inspector.titles, inspector.headings, inspector.main_regions) != (1, 1, 1):
        errors.append(f"{page_path}: expected one title, h1, and main")
    if public_url != "/404.html":
        if len(inspector.descriptions) != 1 or not inspector.descriptions[0].strip():
            errors.append(f"{page_path}: expected one nonempty meta description")
        if len(inspector.canonicals) != 1:
            errors.append(f"{page_path}: expected one canonical URL")
        elif inspector.canonicals[0] != ORIGIN + ("/docs/quick-start/" if public_url == "/docs/" else public_url):
            errors.append(f"{page_path}: incorrect canonical URL {inspector.canonicals[0]}")
    errors.extend(f"{page_path}: {violation}" for violation in inspector.violations)
    for attribute, reference in inspector.references:
        resolved = urlsplit(urljoin(ORIGIN + public_url, reference))
        if resolved.scheme not in {"http", "https"} or resolved.netloc != "oatty.io":
            continue
        target = output_file_for_url(resolved.path)
        if target is None or not target.is_file():
            errors.append(f"{page_path}: broken {attribute} {reference}")
        elif attribute == "href" and resolved.fragment and target.suffix == ".html":
            target_inspector = PageInspector()
            target_inspector.feed(target.read_text(encoding="utf-8"))
            if unquote(resolved.fragment) not in target_inspector.identifiers:
                errors.append(f"{page_path}: missing fragment {reference}")


def verify_site() -> None:
    """Fail when pages, assets, metadata, or the clipboard-only script policy regress."""
    errors: list[str] = []
    inspect_prompt_assets(errors)
    for source in (SITE / "index.html", SITE / "static-docs/api-access/index.html"):
        if not source.is_file():
            errors.append(f"{source}: missing authored prompt page")
    for route in REQUIRED_ROUTES:
        target = output_file_for_url(route)
        if target is None or not target.is_file():
            errors.append(f"missing route: {route}")
    generated_pages = sorted(OUTPUT.rglob("*.html"))
    if not generated_pages:
        errors.append("no generated HTML pages; run npm run build first")
    if len(generated_pages) != len(REQUIRED_ROUTES):
        errors.append(f"expected {len(REQUIRED_ROUTES)} HTML pages, found {len(generated_pages)}")
    for page_path in generated_pages:
        inspect_page(page_path, page_url(page_path), errors)
    for source in sorted((SITE / "static-docs").rglob("*.html")):
        route = "/docs/" + source.relative_to(SITE / "static-docs").as_posix().removesuffix("index.html")
        target = output_file_for_url(route)
        if target is None or not target.is_file() or target.read_bytes() != source.read_bytes():
            errors.append(f"{source}: authored page missing or differs from build output")
        inspect_page(source, route, errors)
    for source, target in ((SITE / "index.html", OUTPUT / "index.html"),
                           (SITE / "static-pages/404.html", OUTPUT / "404.html")):
        if not source.is_file() or not target.is_file() or target.read_bytes() != source.read_bytes():
            errors.append(f"{source}: authored page missing or differs from build output")
        if source.is_file():
            inspect_page(source, "/" if source.name == "index.html" else "/404.html", errors)
    for resource in OUTPUT.rglob("*"):
        if resource.suffix.lower() in {".js", ".mjs", ".cjs"} and resource != OUTPUT / "copy-prompt.js":
            errors.append(f"{resource}: JavaScript resource in published site")
    for stylesheet in OUTPUT.rglob("*.css"):
        imports = re.findall(r"@import\s+(?:url\()?['\"]?(/[^)'\"\s;]+)", stylesheet.read_text(encoding="utf-8"))
        for reference in imports:
            target = output_file_for_url(reference)
            if target is None or not target.is_file():
                errors.append(f"{stylesheet}: broken stylesheet import {reference}")
    if errors:
        raise SystemExit("\n".join(errors))
    print(f"Verified {len(generated_pages)} static HTML pages, authored sources, metadata, and local links.")


if __name__ == "__main__":
    verify_site()
