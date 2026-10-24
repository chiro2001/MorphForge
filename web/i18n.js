import { messages } from './locales/messages.js';

export const supportedLanguages = ['zh-CN', 'ja', 'en'];
let preference = 'auto', language = 'zh-CN', systemLanguages = [];
const listeners = new Set(), bindings = new Set();
const nodeBindings = new WeakMap();
let channel = null, observer = null;
const validPreference = value => value === 'auto' || supportedLanguages.includes(value);
export function resolveLanguage(value = 'auto', languages = []) {
  if (supportedLanguages.includes(value)) return value;
  for (const candidate of languages) {
    const code = String(candidate).toLowerCase().replaceAll('_', '-');
    if (code === 'zh' || code.startsWith('zh-')) return 'zh-CN';
    if (code === 'ja' || code.startsWith('ja-')) return 'ja';
    if (code === 'en' || code.startsWith('en-')) return 'en';
  }
  return 'en';
}
export const getLanguage = () => language;
export const getLanguagePreference = () => preference;
export function t(source, ...values) {
  const message = language === 'zh-CN' ? source : messages[source]?.[language] ?? source;
  return String(message).replace(/\{(\d+)\}/g, (whole, index) => index < values.length ? String(values[index]) : whole);
}
// Use only for application diagnostics, never artwork, captions or chat text.
export function diagnosticMessage(value) {
  const source = String(value ?? '');
  if (messages[source]) return t(source);
  for (const [key, translations] of Object.entries(messages)) {
    if (translations.en === source || translations.ja === source) return t(key);
  }
  return source;
}
export function onLanguageChange(listener) { listeners.add(listener); return () => listeners.delete(listener); }
// These bindings are used only for application-owned labels. User text never
// enters a DOM-wide translation pass. Weak references do not retain old rows.
export function bindText(node, render, property = 'textContent') {
  if (!node) return;
  const value = render(); if (node[property] !== value) node[property] = value;
  let properties = nodeBindings.get(node);
  if (!properties) { properties = new Map(); nodeBindings.set(node, properties); }
  const old = properties.get(property);
  if (old) { bindings.delete(old); properties.delete(property); }
  if (value !== '') {
    const entry = { node: new WeakRef(node), render, property };
    properties.set(property, entry); bindings.add(entry);
  }
  return node[property];
}
export function translateStatic(root = document) {
  const selector = '[data-i18n], [data-i18n-title], [data-i18n-placeholder], [data-i18n-aria-label]';
  const nodes = [...(root.matches?.(selector) ? [root] : []), ...root.querySelectorAll(selector)];
  for (const node of nodes) {
    if (node.hasAttribute('data-i18n')) {
      const value = t(node.getAttribute('data-i18n'));
      if (node.textContent !== value) node.textContent = value;
    }
    for (const attr of ['title', 'placeholder', 'aria-label']) if (node.hasAttribute('data-i18n-' + attr)) {
      const value = t(node.getAttribute('data-i18n-' + attr));
      if (node.getAttribute(attr) !== value) node.setAttribute(attr, value);
    }
  }
}
function apply(value) {
  preference = validPreference(value) ? value : 'auto';
  language = resolveLanguage(preference, [...systemLanguages, ...(globalThis.navigator?.languages || [])]);
  try { globalThis.localStorage?.setItem('morphforge.uiLanguage', preference); } catch {}
  if (typeof document !== 'undefined') {
    document.documentElement.lang = language;
    translateStatic();
    for (const node of document.querySelectorAll('[data-language-select]')) node.value = preference;
    for (const entry of bindings) {
      const node = entry.node.deref();
      if (!node?.isConnected) { bindings.delete(entry); continue; }
      if (entry.property !== 'textContent' || !node.childElementCount) node[entry.property] = entry.render();
    }
  }
  for (const listener of listeners) listener(language);
  globalThis.dispatchEvent?.(new Event('morphforge-language-change'));
}
export function setLanguageForTesting(value, system = []) { systemLanguages = system; apply(value); }
export async function setLanguage(value) {
  if (!validPreference(value)) throw Error('Unsupported interface language');
  // Persist first so a failed save cannot masquerade as a saved preference.
  const response = await fetch('/api/ui-settings', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({language:value}) });
  if (!response.ok) throw Error(t('无法保存界面语言'));
  const saved = await response.json(); systemLanguages = saved.systemLanguages || [];
  apply(saved.language); channel?.postMessage({language:saved.language});
}
export async function initializeLanguage() {
  try { preference = globalThis.localStorage?.getItem('morphforge.uiLanguage') || 'auto'; } catch {}
  try {
    const response = await fetch('/api/ui-settings');
    if (response.ok) { const saved = await response.json(); preference = saved.language; systemLanguages = saved.systemLanguages || []; }
  } catch {}
  apply(preference);
  if (typeof document === 'undefined') return;
  document.addEventListener('change', async event => {
    if (!event.target.matches('[data-language-select]')) return;
    const node = event.target; node.disabled = true;
    try { await setLanguage(node.value); }
    catch (error) { node.value = preference; globalThis.alert(t(error.message)); }
    finally { node.disabled = false; }
  });
  observer = new MutationObserver(records => {
    for (const record of records) for (const node of record.addedNodes) if (node.nodeType === 1) translateStatic(node);
    if (records.some(record => [...record.removedNodes].some(node => node.nodeType === 1))) {
      for (const entry of bindings) if (!entry.node.deref()?.isConnected) bindings.delete(entry);
    }
  });
  observer.observe(document.body, {childList:true,subtree:true});
  if (typeof BroadcastChannel !== 'undefined') {
    channel = new BroadcastChannel('morphforge-ui-language');
    channel.onmessage = event => { if (validPreference(event.data?.language)) apply(event.data.language); };
  }
  globalThis.addEventListener?.('languagechange', () => { if (preference === 'auto') { systemLanguages = []; apply('auto'); } });
}
