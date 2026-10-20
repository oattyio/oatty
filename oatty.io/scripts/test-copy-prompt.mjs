/** Verify clipboard behavior without browser dependencies or permission prompts. */
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import test from 'node:test';
import {runInNewContext} from 'node:vm';

const script = await readFile(new URL('../public/copy-prompt.js', import.meta.url), 'utf8');
const displayedText = 'Displayed prompt with <tags> & exact whitespace.\n\n';

function mount({secure = true, clipboard, missingElement = false} = {}) {
    const events = new Map();
    const button = {hidden: true, disabled: false, addEventListener(name, handler) {
        assert.equal(this.hidden, true, 'bind the listener before revealing the button');
        events.set(name, handler);
    }};
    const prompt = {textContent: displayedText};
    const status = {textContent: 'Select the prompt text to copy it manually.'};
    const container = {querySelector(selector) {
        return {'[data-prompt-text]': prompt, '[data-copy-prompt]': button,
            '[data-copy-status]': missingElement ? null : status}[selector];
    }};
    const document = {querySelectorAll(selector) {
        assert.equal(selector, '[data-setup-prompt]');
        return [container];
    }};
    runInNewContext(script, {document, navigator: {clipboard}, window: {isSecureContext: secure}});
    return {button, prompt, status, events};
}

test('reveals a bound button without writing until click, then copies exact displayed text', async () => {
    const writes = [];
    let finishWrite;
    const view = mount({clipboard: {writeText(text) {
        writes.push(text);
        return new Promise((resolve) => { finishWrite = resolve; });
    }}});
    assert.equal(view.button.hidden, false);
    assert.deepEqual([...view.events.keys()], ['click']);
    assert.deepEqual(writes, []);
    // Reading at click time also ensures clipboard and visible text cannot drift.
    view.prompt.textContent = displayedText + 'Updated in the page.\n';
    const clicked = view.events.get('click')();
    assert.equal(view.button.disabled, true);
    assert.deepEqual(writes, [view.prompt.textContent]);
    finishWrite();
    await clicked;
    assert.equal(view.status.textContent, 'Prompt copied. Paste it into your assistant.');
    assert.equal(view.button.disabled, false);
});

test('announces rejection and leaves a retryable button with manual-copy guidance', async () => {
    let writes = 0;
    const view = mount({clipboard: {async writeText() { writes += 1; throw new Error('Permission denied'); }}});
    await view.events.get('click')();
    assert.equal(writes, 1);
    assert.equal(view.status.textContent, 'Copy failed. Select the prompt text and copy it manually.');
    assert.equal(view.button.disabled, false);
    assert.equal(view.prompt.textContent, displayedText);
});

test('handles synchronous clipboard failure with the same manual fallback', async () => {
    const view = mount({clipboard: {writeText() { throw new Error('Unavailable'); }}});
    await view.events.get('click')();
    assert.match(view.status.textContent, /^Copy failed\./);
    assert.equal(view.button.disabled, false);
});

test('keeps manual fallback and hidden controls when secure clipboard support is unavailable', () => {
    for (const options of [{}, {clipboard: {}}, {clipboard: {writeText: false}},
        {secure: false, clipboard: {writeText() { assert.fail('must not write'); }}}]) {
        const view = mount(options);
        assert.equal(view.button.hidden, true);
        assert.equal(view.events.size, 0);
        assert.equal(view.status.textContent, 'Select the prompt text to copy it manually.');
        assert.equal(view.prompt.textContent, displayedText);
    }
});

test('leaves incomplete markup untouched', () => {
    const view = mount({missingElement: true, clipboard: {writeText() { assert.fail('must not write'); }}});
    assert.equal(view.button.hidden, true);
    assert.equal(view.events.size, 0);
});
