// Policy entries are hostnames with optional wildcard, path and query prefixes.
export function normalizeDomain(host) {
  return host.toLowerCase().replace(/\.+$/, '').replace(/^www\./, '');
}

export function isAllowed(value, status, includeSubdomains = false) {
  if (status?.state && status.state !== "ActiveFocus") return true;
  let url;
  try { url = new URL(value); } catch { return true; }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return true;
  const entries = status?.allowed_domains;
  if (!Array.isArray(entries) || entries.length === 0) return true;
  const host = normalizeDomain(url.hostname);
  if (status.allow_localhost_domains && (host === 'localhost' || host === '127.0.0.1')) return true;
  return entries.some((entry) => {
    if (typeof entry !== 'string') return false;
    let rule = entry.trim();
    if (!rule) return false;
    rule = rule.replace(/^https?:\/\//i, '');
    const wildcard = rule.startsWith('*.');
    if (wildcard) rule = rule.slice(2);
    const split = rule.search(/[/?#]/);
    const ruleHost = normalizeDomain((split < 0 ? rule : rule.slice(0, split)).replace(/:\d+$/, ''));
    if (!ruleHost) return false;
    if (host !== ruleHost && !((wildcard || includeSubdomains) && host.endsWith('.' + ruleHost))) return false;
    if (split < 0) return true;
    let prefix = rule.slice(split).split('#', 1)[0];
    if (prefix.startsWith('?')) prefix = '/' + prefix;
    if (prefix.endsWith('*')) prefix = prefix.slice(0, -1);
    return (url.pathname + url.search).startsWith(prefix);
  });
}
