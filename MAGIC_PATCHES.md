This is a vendored fork of `fuse-backend-rs` 0.14.0.

Local changes:

- add `ZeroCopyWriter::write_from_fd_at`
- thread fd-backed ranges through the synchronous server writer
- let the Linux fusedev writer retain resident file ranges until commit
- commit large fd-backed read replies through a pipe and `splice` into
  `/dev/fuse`, with copy fallback for small reads, unsupported splice, or pipe
  sizing failure
- lower fusedev `max_read` below the common Linux pipe size ceiling so complete
  FUSE replies can fit in the splice pipe

The VM artifact cache only uses the fd-backed path after FUSE INIT negotiates
`FUSE_CAP_SPLICE_WRITE`.
