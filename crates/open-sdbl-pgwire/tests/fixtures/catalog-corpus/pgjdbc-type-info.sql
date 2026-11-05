SELECT typinput='pg_catalog.array_in'::regproc as is_array,
       typtype, typname, pg_type.oid
FROM pg_catalog.pg_type
LEFT JOIN (
    select ns.oid as nspoid, ns.nspname, r.r
    from pg_namespace as ns
    join (
        select s.r, (current_schemas(false))[s.r] as nspname
        from generate_series(1, array_upper(current_schemas(false), 1)) as s(r)
    ) as r using (nspname)
) as sp ON sp.nspoid = typnamespace
WHERE pg_type.oid = $1
ORDER BY sp.r, pg_type.oid DESC;
