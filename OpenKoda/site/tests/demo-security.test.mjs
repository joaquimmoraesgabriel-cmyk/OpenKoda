import test from 'node:test';
import assert from 'node:assert/strict';
import {demoSecurityHeaders} from '../worker.mjs';

test('only the public demo is embeddable and cannot reach account services',()=>{
 const headers=demoSecurityHeaders('https://koda.example');
 const csp=headers.get('Content-Security-Policy');
 assert.match(csp,/connect-src 'none'/);
 assert.match(csp,/frame-src 'none'/);
 assert.match(csp,/worker-src 'none'/);
 assert.match(csp,/form-action 'none'/);
 assert.match(csp,/sandbox allow-scripts allow-forms$/);
 assert.doesNotMatch(csp,/allow-same-origin/);
 assert.equal(headers.get('Access-Control-Allow-Origin'),'*');
 assert.equal(headers.get('Access-Control-Allow-Credentials'),null);
 assert.match(csp,/frame-ancestors 'self'/);
});
