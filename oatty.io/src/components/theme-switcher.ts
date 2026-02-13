import { LitElement, html } from 'lit';

import { applyThemeMode, getStoredThemeMode, type ThemeMode } from '../theme/theme-controller';
import { applyConstructibleStyles } from '../styles/sheets/constructible-style';
import { baseStyle } from '../styles/sheets/base-sheet';
import { moduleStyle } from '../styles/sheets/module-sheet';
import { themeStyle } from '../styles/sheets/theme-sheet';
import { utilsStyle } from '../styles/sheets/utils-sheet';

export class ThemeSwitcher extends LitElement {
  private currentThemeMode: ThemeMode = getStoredThemeMode();

  protected createRenderRoot(): ShadowRoot {
    return this.attachShadow({ mode: 'open' });
  }

  connectedCallback(): void {
    super.connectedCallback();

    if (this.shadowRoot) {
      applyConstructibleStyles(this.shadowRoot, [baseStyle, themeStyle, utilsStyle, moduleStyle]);
    }

    applyThemeMode(this.currentThemeMode);
  }

  private handleThemeChange(event: Event): void {
    const selectElement = event.currentTarget as HTMLSelectElement;
    this.currentThemeMode = selectElement.value as ThemeMode;
    applyThemeMode(this.currentThemeMode);
  }

  protected render() {
    return html`
      <div class="m-theme-control" role="group" aria-label="Theme selector">
        <label class="m-theme-control__label" for="theme-mode">Theme</label>
        <select id="theme-mode" class="m-theme-control__select" @change=${this.handleThemeChange} .value=${this.currentThemeMode}>
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
          <option value="high-contrast">High Contrast</option>
        </select>
      </div>
    `;
  }
}

customElements.define('theme-switcher', ThemeSwitcher);
