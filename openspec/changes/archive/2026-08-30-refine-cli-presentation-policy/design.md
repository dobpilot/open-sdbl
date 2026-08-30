# Design: Kind-specific CLI presentation plans

The policy remains an application concern in `open-sdbl-cli`. It looks up the
requested object by GUID, selects a template from `MetadataKind`, and returns
the existing ID-only structured `PresentationPlan`.

For catalogs the plan is:

`Description + " (" + Code + ")"`

For documents the plan is:

`localized_type_name + " " + Number + " от " + Date`

The Russian Config synonym is preferred for `localized_type_name`; the Config
metadata name and literal `Документ` are fallbacks. `Date` is the physical 1C
document period field. No formatted SQL or metadata field name crosses the
core callback boundary.
