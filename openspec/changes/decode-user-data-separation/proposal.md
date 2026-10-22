# Decode measured user data-separation bindings

## Why
Configured session startup needs the current user’s persisted data-separation bindings. Unknown bindings must not become an empty structure.

## What Changes
Expose an optional complete list of string bindings for the measured native user-record layout. Preserve unknown layouts as unknown and never expose password fields.

## Impact
Additive metadata API in UserData; no dependencies, database writes or authentication changes.
