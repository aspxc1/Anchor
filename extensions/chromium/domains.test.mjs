import { test } from 'node:test';
import { strict as assert } from 'node:assert';
import { isAllowed } from './domains.js';
const policy = (...allowed_domains) => ({ allowed_domains, allow_localhost_domains: false });
test('exact hosts, normalization and explicit subdomains', () => {
  assert.equal(isAllowed('https://WWW.Example.com./x', policy('example.com')), true);
  assert.equal(isAllowed('https://sub.example.com/', policy('example.com')), false);
  assert.equal(isAllowed('https://badexample.com/', policy('*.example.com')), false);
  assert.equal(isAllowed('https://example.com/', policy('*.example.com')), true);
  assert.equal(isAllowed('https://sub.example.com/', policy('*.example.com')), true);
  assert.equal(isAllowed('https://sub.example.com/', policy('example.com'), true), true);
  assert.equal(isAllowed('https://lms.eng.asu.edu.eg/course/1', policy('https://lms.eng.asu.edu.eg/*')), true);
  assert.equal(isAllowed('https://example.com/docs', policy('https://example.com:8443/docs')), true);
});
test('path and query prefixes', () => {
  assert.equal(isAllowed('https://github.com/myorg/repo', policy('github.com/myorg/*')), true);
  assert.equal(isAllowed('https://github.com/other', policy('github.com/myorg/*')), false);
  assert.equal(isAllowed('https://example.com/search?q=foo&x=1', policy('example.com/search?q=foo')), true);
  assert.equal(isAllowed('https://example.com/search?q=bar', policy('example.com/search?q=foo')), false);
});
test('exempt schemes, localhost option and empty policies', () => {
  for (const value of ['chrome://settings', 'edge://settings', 'brave://settings', 'about:blank', 'chrome-extension://abc/a', 'file:///tmp/x']) assert.equal(isAllowed(value, policy('example.com')), true);
  assert.equal(isAllowed('https://elsewhere.com/', policy()), true);
  assert.equal(isAllowed('http://localhost:3000/', {...policy('example.com'), allow_localhost_domains: true}), true);
  assert.equal(isAllowed('http://127.0.0.1/', {...policy('example.com'), allow_localhost_domains: true}), true);
  assert.equal(isAllowed('http://localhost/', policy('example.com')), false);
});

test("a configured domain list applies only while focus is active", () => {
  for (const state of ["Idle", "Break", "HallPass"]) {
    assert.equal(isAllowed("https://reddit.com", { state, allowed_domains: ["docs.rs"] }), true);
  }
  assert.equal(isAllowed("https://reddit.com", { state: "ActiveFocus", allowed_domains: ["docs.rs"] }), false);
});
