/** Enhance the visible setup prompt only when browser clipboard access is available. */
(() => {
    if (!window.isSecureContext || typeof navigator.clipboard?.writeText !== 'function') {
        return;
    }

    for (const container of document.querySelectorAll('[data-setup-prompt]')) {
        const prompt = container.querySelector('[data-prompt-text]');
        const button = container.querySelector('[data-copy-prompt]');
        const status = container.querySelector('[data-copy-status]');
        if (!prompt || !button || !status) {
            continue;
        }

        button.addEventListener('click', async () => {
            button.disabled = true;
            try {
                await navigator.clipboard.writeText(prompt.textContent);
                status.textContent = 'Prompt copied. Paste it into your assistant.';
            } catch {
                status.textContent = 'Copy failed. Select the prompt text and copy it manually.';
            } finally {
                button.disabled = false;
            }
        });
        button.hidden = false;
    }
})();
