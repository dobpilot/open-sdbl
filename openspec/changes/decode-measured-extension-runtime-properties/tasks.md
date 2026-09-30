## 1. Evidence
- [x] 1.1 Retain paired native info/raw records for true, false and named profile SafeMode.
- [x] 1.2 Retain native HashSum/root-key correspondence and InfoBase scope mapping.

## 2. Implementation
- [x] 2.1 Decode mode/profile only for the measured complete record shape, including unknown/truncated cases.
- [x] 2.2 Add platform hash sum and measured typed scope to acquired extensions, with Russian rustdoc.
- [x] 2.3 Keep version/synonym parsing independent from profile strings and add regressions.

## 3. Validation
- [x] 3.1 Run workspace fmt, Clippy, tests, rustdoc and strict OpenSpec validation.
- [x] 3.2 Check live demo/buh projection values without modifying these bases.
- [ ] 3.3 Archive after green verification.
