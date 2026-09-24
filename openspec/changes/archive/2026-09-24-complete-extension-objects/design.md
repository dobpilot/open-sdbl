# Design — the object headers of an extension, whole or not at all

## Context

`AcquiredExtension::objects` (change
`2026-09-24-blocking-session-and-extension-structure`) collects the object
headers of every description resource of an extension. A resource that did
not inflate within the per-resource limit, or was not the brace
serialization, was skipped, and the rest was answered as if it were all.
The catalog of the same extension already answers `None` rather than a
partial catalog; the headers must follow the same rule.

## Decisions

### 1. What "complete" means

The list is answered as `Some` only when all three hold:

1. **Every description resource decoded.** A description resource is one
   the extension's root index names by a bare identifier — no `.` in the
   name. Each must inflate within the per-resource decoding limit and parse
   as the brace serialization. On the four measured extensions every such
   resource does. Resources whose name carries a suffix (`<guid>.0`,
   `<guid>.2`, …) are modules, rights, templates and other content; they are
   not descriptions, are never decoded for headers, and neither add to nor
   subtract from completeness.
2. **A catalog to check against.** See decision 2.
3. **Every declared object described.** Each object the catalog lists must
   have a header whose identifier is that object, read from the resource
   named by that identifier. A declared object with no resource, or whose
   resource carries no header for it, makes the list incomplete.

Anything else answers `None` for the whole list. There is no partial `Vec`
and no per-resource error list: a consumer that must apply the structure
has exactly one question to ask, and it is answered once.

A header that does parse but whose tail is not in the measured shape is
**not** a failure: it is answered with `belonging` and `extends` unknown,
as the header decoder already does, and a consumer sees that on the header
itself. Completeness is about whether a header is missing, not about
whether every header is fully understood.

### 2. No catalog means no list

When `catalog` is `None`, `objects` is `None`.

Without the catalog there is nothing to check the headers against: the
set of description resources is exactly what the root index lists, and a
top-level object whose description is absent from both would go unnoticed.
Answering the headers anyway would make an unverified list look verified —
the very thing this change removes. The rule is also the simplest one a
consumer can rely on: `objects.is_some()` implies `catalog.is_some()`, and
both mean "checked".

### 3. Nothing is inferred

An adopted object whose header records the nil identifier — an extension
mapping by name — keeps `extends: None`. No link is derived from names,
from `ConfigCAS` row names, or from the catalog. Completeness adds a check;
it adds no data.

## Risks

- A future platform writing a bare-identifier resource that is not a
  description would make every such extension answer `None`. That fails
  closed, which is the intended direction; it would be widened by
  measurement, not by skipping.
