## ADDED Requirements

### Requirement: Request explicitly bound enabled Shared values structurally
For IndependentAndShared with explicitly configured use and value parameters, bound compilation SHALL report the existing structural missing-session-parameter diagnostic naming the value parameter when the supplied use is Boolean true and that value is absent from compilation options. Supplying the value SHALL compile its existing typed predicate. This diagnostic protocol SHALL NOT assert that the real host Session value is uninitialized.

#### Scenario: Enabled bound value absent from compiler options
- **WHEN** a read table has Shared with both bindings, true condition or no condition, use=true, and its value absent from compilation options
- **THEN** compilation names that value as a missing session parameter and does not produce an empty-area SQL result

#### Scenario: Host supplies an existing value
- **WHEN** the host retries with its existing String or Number value
- **THEN** compilation uses that value in the separator predicate

### Requirement: Preserve unbound Shared defaults and inactive controls
Shared without a use binding SHALL retain its typed empty-value default. Shared with a use binding but no value binding SHALL retain the common-attribute-name fallback and its existing empty default. A false global condition SHALL select its measured empty area without requesting use/value. Shared use=false SHALL disable its predicate without requesting its value. Preparation and unrelated tables SHALL NOT request separator values. Independent handling SHALL remain unchanged.

#### Scenario: Unbound defaults
- **WHEN** Shared has no use binding, or use=true has no explicit value binding and its fallback name is absent
- **THEN** compilation retains the typed empty default

#### Scenario: Inactive controls
- **WHEN** the global condition is false or Shared use=false
- **THEN** no value diagnostic is requested and the existing empty-area or disabled-predicate behavior is preserved
