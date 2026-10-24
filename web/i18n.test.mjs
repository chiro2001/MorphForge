import test from 'node:test';
import assert from 'node:assert/strict';
import {messages} from './locales/messages.js';
import {t,resolveLanguage,setLanguageForTesting,getLanguage,getLanguagePreference,translateStatic,bindText} from './i18n.js';

test('system language negotiation covers regions and explicit overrides', () => {
  assert.equal(resolveLanguage('auto', ['zh-TW','en-US']), 'zh-CN');
  assert.equal(resolveLanguage('auto', ['ja-JP','en']), 'ja');
  assert.equal(resolveLanguage('auto', ['en-GB']), 'en');
  assert.equal(resolveLanguage('auto', ['fr-FR']), 'en');
  assert.equal(resolveLanguage('ja', ['zh-CN']), 'ja');
});
test('changing languages uses the original key and preserves interpolation data verbatim', () => {
  const userText = '保存 / 日本語 / <script> / {1}';
  for (const lang of ['en','ja','zh-CN','en']) {
    setLanguageForTesting(lang);
    assert.equal(getLanguage(),lang);
    assert.equal(getLanguagePreference(),lang);
    const rendered = t('“{0}”已受保护；点击解除后再编辑',userText);
    assert.ok(rendered.includes(userText));
    assert.equal(t(userText),userText);
  }
  setLanguageForTesting('auto',['ja-JP']); assert.equal(getLanguage(),'ja');
  setLanguageForTesting('zh-CN');
});
test('every interface message has English/Japanese translations and identical placeholders', () => {
  assert.ok(Object.keys(messages).length >= 1200);
  const placeholders = s => [...s.matchAll(/\{\d+\}/g)].map(m=>m[0]).sort();
  for (const [source,values] of Object.entries(messages)) for (const lang of ['en','ja']) {
    assert.ok(values[lang]?.trim(),source+' '+lang);
    assert.deepEqual(placeholders(values[lang]),placeholders(source),source+' '+lang);
  }
});
test('static localization is restricted to marked labels and does not touch user content', () => {
  const attrs = new Map([['data-i18n','保存']]);
  const label = {textContent:'保存',hasAttribute:k=>attrs.has(k),getAttribute:k=>attrs.get(k),setAttribute:(k,v)=>attrs.set(k,v)};
  const user = {textContent:'保存'};
  const root = {querySelectorAll:()=>[label]};
  setLanguageForTesting('en'); translateStatic(root); assert.equal(label.textContent,'Save'); assert.equal(user.textContent,'保存');
  setLanguageForTesting('ja'); translateStatic(root); assert.equal(label.textContent,'保存');
  setLanguageForTesting('zh-CN');
});
test('binding replacement preserves subsequent user text and clearing removes obsolete labels', () => {
  const node = {textContent:'',isConnected:false};
  bindText(node,()=>t('保存')); assert.equal(node.textContent,'保存');
  bindText(node,()=> '保存 / user'); assert.equal(node.textContent,'保存 / user');
  bindText(node,()=> ''); assert.equal(node.textContent,'');
});
