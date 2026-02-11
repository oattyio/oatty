export interface ConstructibleStyle {
  readonly sheet: CSSStyleSheet | null;
  readonly text: string;
}

function supportsConstructibleStylesheets(): boolean {
  return 'adoptedStyleSheets' in Document.prototype && 'replaceSync' in CSSStyleSheet.prototype;
}

export function createConstructibleStyle(cssText: string): ConstructibleStyle {
  if (!supportsConstructibleStylesheets()) {
    return { sheet: null, text: cssText };
  }

  const stylesheet = new CSSStyleSheet();
  stylesheet.replaceSync(cssText);

  return { sheet: stylesheet, text: cssText };
}

export function applyConstructibleStyles(root: ShadowRoot, styles: readonly ConstructibleStyle[]): void {
  const stylesheets = styles
    .map((style) => style.sheet)
    .filter((sheet): sheet is CSSStyleSheet => sheet !== null);

  if (stylesheets.length === styles.length) {
    root.adoptedStyleSheets = stylesheets;
    return;
  }

  const styleElementId = 'oatty-fallback-styles';
  let styleElement = root.querySelector<HTMLStyleElement>(`style[data-style-id='${styleElementId}']`);

  if (!styleElement) {
    styleElement = document.createElement('style');
    styleElement.dataset.styleId = styleElementId;
    root.prepend(styleElement);
  }

  styleElement.textContent = styles.map((style) => style.text).join('\n');
}
