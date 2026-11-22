# Design

The separator count at field 17 and the ordered string pairs after it are shared by the legacy and extended layouts. The optional block starts at `30 + count`; the measured extended form has two or five strings followed by the six-field suffix. The production decoder reuses the existing structural tail validation, accepting identifiers 1 and 2 for the new separation profiles. The caller still receives only the optional list of separator names and values. Authentication consumes the same private fields and validation as before.

## Native control

On 1C 8.3.27.2342, four users were created in a disposable PostgreSQL infobase with empty or single-string separator bindings. Only the optional block and suffix of their own records were reconstructed in the measured extended shapes. The native user manager under a separate authenticated control administrator read both zero counts and the original `НаименованиеЯзык1` value for the two-string SHA-512 and five-string SHA-256 inputs. The controlled records were checked again after reading to verify byte stability. The text result is in `tests/fixtures/user-data-separation/extended.oracle`. The real UNF record was inspected read-only and has 42 fields, zero bindings, five optional strings and algorithm code 1.
