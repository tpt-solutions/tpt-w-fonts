# Archon support: design (design only, not in v1)

Status: **design draft.** Nothing here ships in v1. The v1 code paths for Windows and Linux are
unchanged by this document.

Audience: tpt-fonts maintainers, and the tpt-archon team who will review the open questions.

## 1. Problem

Every application that uses a font loads its own copy of the bytes, so 10,000 installed fonts
cost RAM once per process that touches them. On Archon, fonts can instead live in the unified
page cache as memory-mapped, read-only shared objects. Apps would bind to a font by capability and
share one copy.

tpt-fonts is the manager that decides which fonts exist and which are active. This document covers
how it would hand those fonts to Archon.

## 2. Non-goals

- Replacing the Windows and Linux backends. Archon support is additive.
- Defining Archon's kernel or IPC interfaces. Those belong to the tpt-archon project.
- Any change to the v1 session model in `core/src/session.rs`, beyond what section 6 lists.

## 3. What tpt-archon documents today

Taken from a summary of the public project page at github.com/tpt-solutions/tpt-archon. The raw
README and the crate source paths returned 404 when fetched, so the crate-level details below are
unverified:

- A layered stack: `tpt-archon-core` (`no_std` storage engine), `tpt-archon-bridge` (zero-copy IPC
  and page-cache traits), `tpt-archon-kernel` (capability-based microkernel), and
  `tpt-archon-relational`.
- The page cache is described as unified with kernel memory management and the database buffer
  pool, in one zero-copy address space.
- A read-only `mmap` path exists behind a feature flag. A writable `mmap` path exists in the core
  crate only.
- The project says it is not production-ready.

It does **not** document a shared-object or font API, a capability-handle model, or a syscall
table. Section 8 lists those as questions.

## 4. Proposed model

### 4.1 Font objects are content-addressed

Phase 2 already computes a BLAKE3 hash of every font file (`FontFile::content_hash`). Using that
hash as the object identity gives:

- Two identical files in different folders map to one shared object. This matches the exact
  duplicate detection from Phase 2.
- A font whose bytes change gets a new object. Old bindings keep the old bytes until released.

The object name is the hash. The file name and install location are metadata only.

### 4.2 Lifecycle

```
catalog scan ──► ensure object (hash) present in page cache, read-only
app binds    ──► grant capability to object  ──► app maps the bytes read-only
app releases ──► revoke capability
last release ──► object becomes evictable (page cache decides when)
```

tpt-fonts would **not** pin pages itself. Eviction is Archon's job. tpt-fonts only tracks which
objects are wanted.

### 4.3 Sketch of the interface

This is a sketch for discussion. Names are placeholders until the Archon bridge API is confirmed.

```rust
/// Binds fonts from tpt-fonts into Archon's shared page cache.
///
/// Implementations must not expose writable mappings: font objects are read-only once loaded.
pub trait ArchonFontBackend {
    /// Whether this host runs Archon with the shared-object capability available.
    /// The v1 build always returns `false`.
    fn is_available(&self) -> bool;

    /// Loads the bytes for `hash` into the page cache if they are not already resident.
    fn ensure_object(&self, hash: &ContentHash, bytes: &[u8]) -> Result<ObjectId, FontError>;

    /// Grants a capability to `app` so it can map the object read-only.
    fn bind(&self, object: ObjectId, app: AppId) -> Result<Binding, FontError>;

    /// Revokes a binding. The object stays cached until Archon evicts it.
    fn release(&self, binding: Binding) -> Result<(), FontError>;
}
```

Until implemented, every method other than `is_available` returns `FontError::Unsupported`.

## 5. Post-v1 feature flag plan

Recorded here so the work is scoped before anyone starts it. None of it is built in v1.

1. Add `archon = []` to `tpt-fonts-core`'s `[features]`. It is off by default.
2. Under `#[cfg(feature = "archon")]`, add the `ArchonFontBackend` trait and a stub implementation
   whose `is_available()` returns `false`.
3. Stub methods return `FontError::Unsupported`. Add that variant only with the feature enabled, so
   the default error type does not change.
4. Tests: with the feature on, `is_available()` is `false` and every operation reports
   `Unsupported`. Nothing else changes.
5. The Archon implementation replaces the stub only after the open questions in section 8 are
   answered and the tpt-archon API is published.

## 6. Interaction with the session model

The v1 session model activates fonts with `AddFontResourceEx` (Windows) or a fontconfig drop-in
(Linux). On Archon, an activation would become a `bind` for the requesting app, not a system-wide
registration. Open questions:

- Does "session-scoped" mean "bound for the lifetime of the login session", or per app?
- Who releases bindings if the tray process dies? The orphan recovery in Phase 6 would need an
  Archon equivalent.

## 7. Risks

- **Unclear ABI.** Without a published shared-object API, the trait in section 4.3 may change
  significantly.
- **Page-cache coupling.** tpt-fonts depends on Archon evicting unused objects. If Archon keeps
  everything resident, the memory benefit disappears.
- **Two code paths for one feature.** Session behaviour would differ between Archon and the
  other platforms. The UI must make that visible rather than hide it.

## 8. Open questions for the tpt-archon team

These are the questions from `todo.md` Phase 9, plus ones found while writing this document.

1. What is the mmap and shared-object syscall interface, and does it take a file descriptor, a
   byte slice, or a content hash?
2. What capability grants and revocations exist for font handles, and can a grant be scoped to
   read-only?
3. What are the per-app binding performance targets? Is a bind expected to be microseconds or
   milliseconds?
4. How does a shared object interact with session-scoped activation (section 6)?
5. Are objects page-aligned in the cache? Font tables are not aligned on disk, so this decides
   whether tpt-fonts must copy bytes on load.
6. Is eviction policy configurable, and can an application pin an object?
7. How are bindings released if the process that granted them crashes?
8. Is there a documented mapping from `tpt-archon-kernel`'s `Resource` capabilities to a font
   resource? The README mentions `Resource::Device` for drivers, but not fonts.
9. Does `tpt-archon-bridge` expose the page-cache traits as public API that tpt-fonts can depend
   on, and under what stability promise?

## 9. Exit criteria for leaving the design phase

- The tpt-archon team answers questions 1, 2, 5 and 7. Those decide whether the trait in section
  4.3 can exist as written.
- A small spike on Archon shows one font object bound read-only by one app, and released cleanly.
- The Phase 9 todo items are closed with links to the answers.
