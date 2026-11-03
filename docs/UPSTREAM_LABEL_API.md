# GitBucket label API follow-up

This records server changes needed beyond the PR-06 client adapter. It is based
on GitBucket 4.44.0; it is not an upstream patch or a claim about every release.

## Same-name color updates

The [REST controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueLabelControllerBase.scala)
accepts `CreateALabel(name, color)` for PATCH. A missing field causes 404. A
complete same-name request returns 422 because its collision check finds the
label being edited. A fresh-name PATCH succeeds and updates the existing label ID.

The upstream correction should exclude the current numeric ID from the collision
check. Another label with the destination name must still cause a conflict. A
future update-specific request type could make name/color independently optional
and preserve unspecified values rather than requiring the client to echo them.
Tests should prove that color-only, same-name and rename updates retain the same
ID and all Issue-label associations, while a conflicting rename changes nothing.
Do not implement a color update as delete/create or as a temporary rename.

The [Web controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/LabelsController.scala)
already updates by numeric ID and excludes that ID from name collision checks.
PR-06 uses that in-place route only with `label edit --web` after a same-name REST
422, validates the row and edit form, and checks the ID and stored color afterward.
Once REST is corrected, the same client request can succeed without Web credentials.

## Description persistence and capability evidence

The [input type](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/CreateALabel.scala)
and [API representation](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiLabel.scala)
contain name/color, and the Web form contains labelName/labelColor. A supplied
REST description is ignored and cannot be read back. Adding an HTTP parameter
alone does not add persistence.

Supporting descriptions upstream requires a storage/model migration plus explicit
create/read/update/clear semantics in REST (and Web if offered). Preserve descriptions
when other fields change. Define whether clear means null, an empty string or
both, and verify stored values after create/set/clear and after rename/color-only
updates. Expose an unambiguous versioned capability or documented contract so a
client does not infer write support from one read-only field or a successful HTTP
status. PR-06 rejects description write options before mutation until a supported
adapter can confirm their semantics; it still displays descriptions from reads.

## Stable identity and concurrency

Exposing a numeric label ID in the REST representation would remove the need to
identify a Web row through sanitized HTML. An ID-based resource route would also
make names that are ambiguous URL dot segments safely addressable. The client
rejects ambiguous names rather than letting path normalization target another API.

Conditional updates (for example, a checked version/ETag) would be needed to prevent
all races between reading current fields and writing a replacement. Client-side
snapshot and form checks do not provide a server-side transaction. The Issue label
add route also creates unknown repository labels; the client checks additions
before any edit, but a concurrent deletion can still race with that server behavior.
