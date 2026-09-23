# Attachment invalidation

Creating an attachment on an existing page changes that page's files, not the page's existence. File revision invalidation lives in `deepwell/src/services/file_revision/service.rs`. See [queue incident and recovery](../wiki/systems/cobalt-queue-recovery.md) for operational evidence.

## What it must do

- [x] Commit the upload request before file finalization, matching the production transaction boundary.
- [x] Preserve the created file's owner, filename and byte size.
- [x] Do not enqueue rerenders for ordinary links to an unchanged owner page when its first file revision is created.
- [x] Preserve site-navigation invalidation when the attachment owner is itself the navigation page.
- [ ] Verify template/include invalidation for attachment owners with those dependencies.
- [x] Verify the isolated native attachment proof in production order with a real upload.

## How it works

- [Queue incident and recovery](../wiki/systems/cobalt-queue-recovery.md)

## Implementation inventory

- `deepwell/src/services/file_revision/service.rs`: first file revision uses page-edit invalidation, not page-displacement invalidation.
- `deepwell/tests/file_attachment_invalidation.rs`: native upload/file-create regression with a configured navigation page linking to the existing owner.

## Tests asserting this spec

- `deepwell/tests/file_attachment_invalidation.rs`: committed upload request, real S3 PUT, file creation/readback, unchanged ordinary links, and retained navigation fanout. Run explicitly with `cargo test --test file_attachment_invalidation -- --ignored` against a dedicated empty Redis database. The fixture precreates its queue before starting workers, excluding unrelated recurring maintenance producers from cumulative enqueue counts. Valid RED: `/tmp/claude/cobalt-file-invalidation-red-valid.log`; ordinary-owner GREEN: `/tmp/claude/cobalt-file-invalidation-green.log`; final isolated production-order proof: `/tmp/claude/cobalt-file-invalidation-isolated-green.log`.

## Known gaps (current cycle)

- [x] Deploy the isolated hotfix `f18dcfed691983a12021d6c1da372203ce145138` (parent `474b308be897f40d96767c585e96c45fa3c55095`). Host commit `72bd4a4` pins it; remote generation `b8lyk2lnn46pkgw1w2l9nqyy9bcy5qy8-nixos-system-sakuin-digital-ocean-26.05.20260606.9b69646` is active. Deployment scope was exactly three files, with one runtime line changing `process_page_displace` to `process_page_edit`; `ops/deploy.sh` ended `Ready. Deploy complete.` User-owned rendering work was untouched.
- [ ] Verify template/include invalidation for attachment owners with those dependencies.
- [ ] Attribute every historical queue producer. This fix prevents the verified attachment path but does not explain every historical job.

## Out of scope

No page/file/source deletion, history migration, rendering-parity claim, or logging suppression.
