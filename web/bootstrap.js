import { initializeLanguage } from './i18n.js';
await initializeLanguage();
const entry = document.documentElement.dataset.appEntry || 'studio';
if (['studio', 'mini', 'app'].includes(entry)) await import('./' + entry + '.js');
