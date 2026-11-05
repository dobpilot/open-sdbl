select ssl from pg_stat_ssl where pid = pg_backend_pid()
-- next
select current_database() as a, current_schemas(false) as b
-- next
select round(extract(epoch from pg_postmaster_start_time() at time zone 'UTC')) as startup_time
