## 1. Implementation

- [x] 1.1 The restriction store keeps the origin and can forget the
      derived restrictions.
- [x] 1.2 `role_restrictions` expands the `Чтение` restrictions of the
      current user into single-line SDBL conditions.
- [x] 1.3 `\as`, `\as clear`, `\session` and `\refresh` maintain them.
- [x] 1.4 Case-insensitive search folds the text once per search, not
      once per occurrence.

## 2. Verification

- [x] 2.1 Tests; a live check on the УНФ base; the five CI checks.
