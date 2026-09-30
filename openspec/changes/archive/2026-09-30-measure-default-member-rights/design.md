# Design

Native 1C 8.3.27.2342 was measured on disposable `/tmp/open-sdbl-rights-095c`, not demo, buh or unf. Real authenticated users had object Read on a catalog and a Boolean constant, with `setForAttributesByDefault` true and false. Both queried the constant value and used the constant manager successfully. The constant value is represented in DBNames by the same GUID as its owner. It has no separate attribute metadata object or member Read permission. The minimal implementation grants only this verified identity after the existing object Read check.

The probe additionally showed that native AccessRight rejects `Read` for a custom attribute; its UI right is `View`. Reading custom attributes in a query succeeded in the probe with both default flag states and an explicit View refusal. That does not justify inventing default member Read semantics. The existing conservative custom/standard-field boundary remains until a separately measured mapping change.

Exact explicit member refusals, unsupported nested mappings, missing resources and unresolved identifiers remain closed. Cross-role object Read grants remain additive. Extension safe mode (Boolean or security-profile string), scope mapping and platform HashSum remain unknown until differential native/database evidence exists; the CAS root key is not HashSum.
