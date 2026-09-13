## 1. Implementation

- [x] 1.1 Carry the dimensions and resources of an aggregating register
  relation into its source scope.
- [x] 1.2 Sum away the unused dimensions once the branch is compiled,
  beside the constants finalizer.
- [x] 1.3 Accept a virtual table without its argument list.

## 2. Verification and documentation

- [x] 2.1 Goldens for the pruned relation and the bare form; probe the
  platform on an accumulation register added to the probe base.
- [x] 2.2 Update `docs/query-language-support.md`; run the five CI checks
  and strict OpenSpec validation.
