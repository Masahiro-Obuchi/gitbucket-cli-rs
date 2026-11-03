# Focused GitBucket HTML fixtures

`gitbucket-4.44-label-row.html` and `gitbucket-4.44-label-edit.html` were captured
from the disposable pinned GitBucket 4.44.0 fixture during PR-06 validation.
Only the label row and edit fragment were retained. Numeric IDs were replaced
with 42, repository paths with `alice/project`, and any origin with
`https://example.test/gitbucket`. Scripts were removed from the edit fragment.
There are no authentication fields, cookies or tokens in these fixtures.

The row intentionally retains the server's root-relative link even when fetched
through `/gitbucket`. The parser resolves this as metadata within the configured
base; network requests still use the selected deployment URL. Entity escaping,
input IDs, values and HTML structure are retained to detect layout drift.
