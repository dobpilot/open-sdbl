select L.transactionid::varchar::bigint as transaction_id
from pg_catalog.pg_locks L
where L.transactionid is not null
order by pg_catalog.age(L.transactionid) desc
limit 1
-- next
show DateStyle
-- next
select name, is_dst from pg_catalog.pg_timezone_names
union distinct
select abbrev as name, is_dst from pg_catalog.pg_timezone_abbrevs
-- next
select R.oid::bigint as role_id, rolname as role_name,
  rolsuper is_super, rolinherit is_inherit,
  rolcreaterole can_createrole, rolcreatedb can_createdb,
  rolcanlogin can_login, rolreplication /* false */ is_replication,
  rolconnlimit conn_limit, rolvaliduntil valid_until,
  rolbypassrls /* false */ bypass_rls, rolconfig config,
  D.description
from pg_catalog.pg_roles R
  left join pg_catalog.pg_shdescription D on D.objoid = R.oid
-- next
select member id, roleid role_id, admin_option
          from pg_catalog.pg_auth_members order by id, roleid::text
-- next
select T.oid::bigint as id, T.spcname as name,
       T.xmin as state_number, pg_catalog.pg_get_userbyid(T.spcowner) as owner,
       pg_catalog.pg_tablespace_location(T.oid) /* null */ as location,
       T.spcoptions /* null */ as options,
       D.description as comment
from pg_catalog.pg_tablespace T
  left join pg_catalog.pg_shdescription D on D.objoid = T.oid
--  where pg_catalog.age(T.xmin) <= #TXAGE
-- next
select T.oid as object_id,
                 T.spcacl as acl
          from pg_catalog.pg_tablespace T 
          union all
          select T.oid as object_id,
                 T.datacl as acl
          from pg_catalog.pg_database T
-- next
select N.oid::bigint as id,
       datname as name,
       D.description,
       datistemplate as is_template,
       datallowconn as allow_connections,
       pg_catalog.pg_get_userbyid(N.datdba) as "owner"
from pg_catalog.pg_database N
  left join pg_catalog.pg_shdescription D on N.oid = D.objoid
order by case when datname = pg_catalog.current_database() then -1::bigint else N.oid::bigint end
-- next
select N.oid::bigint as id,
       N.xmin as state_number,
       nspname as name,
       D.description,
       pg_catalog.pg_get_userbyid(N.nspowner) as "owner"
from pg_catalog.pg_namespace N
  left join pg_catalog.pg_description D on N.oid = D.objoid
order by case when nspname = pg_catalog.current_schema() then -1::bigint else N.oid::bigint end
