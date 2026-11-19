# Focused GitBucket HTML fixtures

`gitbucket-4.44-label-row.html` and `gitbucket-4.44-label-edit.html` record the label row and edit form served by the Docker GitBucket 4.44.0 fixture. The fragments use the dummy ID `42`, the repository `alice/project`, and the base URL `https://example.test/gitbucket`. The edit fragment excludes scripts and authentication data.

The label row retains the server's root-relative link when requested through `/gitbucket`. The parser resolves the link against the configured deployment. The fixtures retain entity escaping, input IDs, values, and HTML structure so parser tests can detect layout changes.

`gitbucket-4.44-fork-header.html` records the repository link and immediate `forked from` link with the dummy targets `alice/project` and `bob/project`. GitBucket renders this evidence in the repository header and omits `fork`, `parent`, and `source` in the corresponding API response.

`gitbucket-4.44-review-conversation.html` captures visible review markup for the supported server layout. The review tests exercise sanitized identities, old and new line positions, and the separation of discussion and commit comments.
