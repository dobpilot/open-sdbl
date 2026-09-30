# Native field access evidence (1C 8.3.27.2342)

Measured on 2026-09-30 using a disposable file infobase `/tmp/open-sdbl-rights-095c`, with native Designer XML load/update and actual authenticated Enterprise sessions. No changes were made to demo, buh or unf. `query.bsl` ran through the existing open-bsl form measurement harness. Its outputs include the authenticated user as a canary.

`DefaultGrant` and `DefaultRefuse` have object Read on the catalog and Boolean constant and differ in `setForAttributesByDefault` (true/false). Both successfully queried the constant and called its manager Get. `ExplicitRefuse` additionally refuses custom attribute View, but its programmatic query still succeeds. These catalog queries returned zero rows: they establish preparation/execution authorization, not nonempty value retrieval.

For the denial control, DefaultGrant's constant Read was changed to false in this disposable base. Its constant query and manager Get both failed with native access violations while the catalog queries still succeeded. DefaultGrant.Rights.xml is the final native DENIAL dump: 1C omits the false right matching setForNewObjects=false. DefaultGrant.granted.input.Rights.xml reconstructs the input of the initial grant run by adding the explicit constant Read true; it is labelled input rather than native output. DefaultRefuse and ExplicitRefuse retain constant Read true.

The metadata XML is the native round-trip dump, not output from this implementation. The constant has no child attribute object: its value uses the constant's own GUID in DBNames.

`access-right-invalid-member.platform.txt` records a diagnostic probe, not a successful permission assertion. `member-read-inapplicable.bsl` and its separate output request Read of a custom attribute; native AccessRight rejects that right as inapplicable. The retained second version checks View; the standard attribute call has an unsupported argument type and aborts each case before constant checks. It is not used to authorize any standard field.

Scope of the implementation: constant value Read through verified owner identity only. Custom and standard field authorization retains the existing conservative boundary. SafeMode/profile, extension scope mapping and native HashSum remain unmeasured; CAS root hashes must not be substituted for HashSum.
