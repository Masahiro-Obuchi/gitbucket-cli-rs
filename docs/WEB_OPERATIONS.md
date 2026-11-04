# Operation routes and confirmation

The CLI keeps operation-specific adapters and a shared progress/result model.
The API and Web clients handle transport and forms; command handlers plan intent,
preflight known constraints, invoke the adapter and verify the stored effect.
`src/operation.rs` records confirmation without assuming that earlier writes can
be rolled back. It preserves underlying error codes/statuses in JSON failures.

These are confirmed GitBucket **4.44.0** differences, not promises about every
server version. REST routes of unknown capability are tried once; explicit known
rejections can select the corresponding Web adapter. Authentication failures,
5xx, network failures and parse failures never trigger a blind Web write retry.

| Operation | Confirmed REST support / restriction | Web route and confirmation |
| --- | --- | --- |
| PR title/body/state | Supported by PR PATCH. Body-only needs the current title; title-only retains body. | No Web retry, even with `--web`. Validate returned fields. |
| PR assignees | Not writable through the verified PR PATCH. | Explicit `--web`; authenticate before REST fields in mixed edits. Verify each delta through Issue GET. |
| Issue labels | Dedicated additive POST and individual DELETE supported. | No Web. Check additions exist before metadata, preserve unrelated labels, verify each mutation. |
| Issue metadata/state | Issue PATCH absent in 4.44; behavior on other instances remains unknown until requested. | Initially readable target required before a 404 fallback. Preflight forms and referenced targets, then verify fields after each form. Lifecycle never substitutes comment creation. |
| Milestones | REST supported; 4.44 restricts titles to a narrow ASCII set. Replacement requests require all retained fields. | Only the confirmed title restriction permits supplementation. Preserve/reject date/whitespace constraints before writes, verify content before changing state. |
| Repository label color | Same-name REST PATCH rejected with 422 in 4.44. Rename supported with complete name/color. | Only explicit `--web` after that rejection. Prepare and validate the numeric-ID form first; verify unchanged ID and stored color. |
| Label descriptions | Verified REST/Web routes do not persist them. | Unsupported set/clear fails before any mutation. |
| Repository fork | Route absent in 4.44; other compatible servers can support it. `--group` uses REST `organization`. | A readable source and nonconflicting destination are required. Verify destination and immediate parent; poll reads only for asynchronous creation. |
| Repository delete | Route absent in 4.44; other compatible servers can support it. | Read target before deletion, require a confirming 404 after deletion. A missing initial target never causes Web deletion. |

4.44's repository API omits `fork`, `parent` and `source`. This is **unknown fork
metadata**, not evidence that a repository is not a fork. When required, the
strict header adapter checks both the destination link and its `forked from`
link, which renders the immediate parent. Root-relative proxy links are resolved
within the configured deployment and must match the exact expected repository,
including when an owner matches the deployment prefix. Public verification
requests use an absolute deployment URL and need no password;
Web writes and private cookie-session reads use the verified Web session.
Unknown layouts, inaccessible proof and inconsistent destinations remain errors.

Web HTTP 200 only establishes that a response was received. Verification errors
are `unverified`, with no successful output. Multi-step errors list confirmed,
rejected, unverified and unattempted steps. A confirmed earlier step remains
recorded when a later request fails; neither automatic rollback nor write retry
is performed. Concurrent writers and later permission changes can invalidate a
snapshot after confirmation; these operations are not transactions.

Web principal verification requires password login and a cookie-authenticated
`/api/v3/user`. The default Web user must equal the REST token user. An explicit
`GB_WEB_USER` authorizes a different Web actor; notices show both actors and JSON
operation records include the verified Web actor. Interactive SSO/OIDC is not
implemented. Redirects stay within the selected origin and deployment.
This boundary check includes repeated percent decoding, so encoded traversal
cannot forward sign-in forms or cookies outside the selected deployment.
Mutation outcomes use that request's redirect history, preserving uncertainty
when a redirect returns to the original URL and avoiding false redirects for
Unicode URL encoding. Cookie state remains shared with the verified session.

Primary contracts:

- [ApiUserControllerBase](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiUserControllerBase.scala) and [IndexController](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/IndexController.scala): cookie principal and password/SSO sign-in.
- [IssuesController](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/IssuesController.scala): trimmed forms and state route.
- [AccountController](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/AccountController.scala), [RepositoryCreationService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/RepositoryCreationService.scala), [ApiRepository](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiRepository.scala), [repository menu](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/menu.scala.html): asynchronous fork and parent proof.
- [GitHub's compatible fork request](https://docs.github.com/en/rest/repos/forks#create-a-fork): REST `organization` field; this does not imply that 4.44 implements that route.

Transport-wide deadlines and response-body failure handling are a separate follow-up.
