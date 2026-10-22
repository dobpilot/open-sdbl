# Why
Selected metadata loses supported common/system settings tables because these physical tables have no Config/DBNames object owner. Existing settings load compilers consequently reject acquired storage.
# What changes
Retain only already supported settings storage facts and their required field mappings during selected resolution. Share fixed storage identities with the existing compiler. Preserve strict separator acquisition, fingerprints and exclusion of unrelated tables.
# Impact
Internal selected metadata projection and settings storage identity reuse; no new public API, SQL, I/O, dependencies or storage interpretation.
