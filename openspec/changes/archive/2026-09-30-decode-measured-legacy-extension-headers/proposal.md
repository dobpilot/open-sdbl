# Why
The active buh extension has 37 catalog objects with native legacy headers the decoder does not recognize, so whole-or-unknown object acquisition correctly refuses the entire list.

# What Changes
Support the legacy identifier/name header measured against native XML export, preserving unknown links and whole-list completeness.

# Impact
Extension structure decoding only; no dependencies or database mutations.
