# Change: Read indexed configuration resources

## Why
Session hosts need only reached module and metadata dependencies. The existing selected acquisition still decodes all metadata and scans the remaining Config store.

## What Changes
- Add bootstrap acquisition of layout, root, configuration descriptor and raw extension catalog without metadata resolution or CAS acquisition.
- Add bounded SQL-addressed Config and ConfigCAS resource reads through asynchronous and blocking sessions, reusing the detected layout.
- Preserve existing acquisition, timeout, transaction cleanup and closed-session behavior.
