# Why
An absent version and an unsupported record currently both return None, preventing a complete runtime projection. Native 1C returns an empty version for a newly created extension.

# What Changes
Add a reliable runtime version projection preserving the existing partially decoded version field.

# Impact
Public extension acquisition API; no dependency or transport changes.
