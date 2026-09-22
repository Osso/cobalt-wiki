# Attachment invalidation

Creating an attachment on an existing page changes that page's files, not the page's existence. File revision invalidation lives in `deepwell/src/services/file_revision/service.rs`. See [replica status](../wiki/systems/cobalt-replica-status.md) for operations and proof.

## What it must do

- [x] Commit the upload request before file finalization, matching the production transaction boundary.
- [x] Preserve the created file's owner, filename and byte size.
- [x] Do not enqueue rerenders for ordinary links to an unchanged owner page when its first file revision is created.
- [x] Preserve site-navigation invalidation when the attachment owner is itself the navigation page.
- [ ] Verify template/include invalidation for attachment owners with those dependencies.

## How it works

- [Replica status](../wiki/systems/cobalt-replica-status.md)

## Implementation inventory

- `deepwell/src/services/file_revision/service.rs`: first file revision uses page-edit invalidation, not page-displacement invalidation.
- `deepwell/tests/file_attachment_invalidation.rs`: native upload/file-create regression with a configured navigation page linking to the existing owner.

## Tests asserting this spec

- `deepwell/tests/file_attachment_invalidation.rs`: committed upload request, real S3 PUT, file creation/readback, unchanged ordinary links, and retained navigation fanout. Run explicitly with `cargo test --test file_attachment_invalidation -- --ignored` against a dedicated empty Redis database. The fixture precreates its queue before starting workers, excluding unrelated recurring maintenance producers from cumulative enqueue counts.

## Known gaps (current cycle)

- [ ] Independently verify the change and legitimate invalidation behavior.
- [ ] Deploy the verified change and recover the existing backlog under authorized maintenance conditions.
- [ ] Establish which existing queued jobs are redundant before removing any; this correction alone does not remove queued work or explain every job.

## Out of scope

No page/file/source deletion, queue deletion without semantic proof, or logging suppression.
