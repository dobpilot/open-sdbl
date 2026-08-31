## Context

DEFLATE codes are at most 15 bits. The current `Huffman::decode` reads one bit
at a time and scans a vector of every nonzero symbol after each bit. On large
metadata resources this dominates startup CPU time.

## Decisions

### Expand canonical codes into a bounded direct table

Allocate `2^maximum_length` entries for a Huffman tree, capped by DEFLATE's
15-bit maximum. For each canonical reversed code, populate all table indices
whose low `length` bits share that code. Decode by peeking the maximum-width
bit prefix, selecting one entry, validating that its real length is available,
and advancing by that real length.

The worst table has 32,768 compact entries and remains bounded independently of
input size. Incomplete-tree holes retain an invalid sentinel and preserve the
existing malformed-code diagnostic.

### Read bit windows by bytes

For fields up to 15 bits, assemble at most three input bytes into an integer,
shift by the current bit offset, and mask the requested width. Normal reads
still reject truncation before advancing. Huffman lookup may zero-pad its peek
past the physical end only to select a shorter code; it rejects a selected code
whose actual length exceeds the remaining input.

### Preserve validation before table construction

Keep empty-tree, length-limit, and oversubscription checks before expanding
codes. Assert during construction that prefix-free canonical codes do not
overwrite an existing table entry.

## Verification

Retain existing conformance vectors, add coverage for short final codes and
invalid lookup holes, run the complete quality gates, then time and profile the
reported live console startup again.
