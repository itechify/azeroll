import {readFile} from 'node:fs/promises';
import test from 'node:test';
import assert from 'node:assert/strict';
const source=(await readFile(new URL('../data-url.js',import.meta.url),'utf8')).replaceAll('import.meta.url',JSON.stringify('https://itechify.github.io/azeroll/data-url.js'));
const {dataUrl}=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
test('hosted data normalizes names while local MPQs retain component encoding',()=>{
  globalThis.window={__azeroll_data_base:'https://example.com/1.12.1-5875/'};
  assert.equal(dataUrl('Fonts\\FRIZQT__.TTF'),'https://example.com/1.12.1-5875/fonts/frizqt__.ttf');
  assert.equal(dataUrl('World/Some Name.M2'),'https://example.com/1.12.1-5875/world/some%20name.m2');
  window.__azeroll_data_base='';
  assert.equal(dataUrl('Fonts\\FRIZQT__.TTF'),'https://itechify.github.io/azeroll/data/Fonts%5CFRIZQT__.TTF');
  assert.equal(dataUrl('__index'),'https://itechify.github.io/azeroll/data/__index');
  delete globalThis.window;
});
