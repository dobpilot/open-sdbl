select case
  when pg_catalog.pg_is_in_recovery()
    then null
  else
    (pg_catalog.txid_current() % 4294967296)::varchar::bigint
  end as current_txid
-- next
select usesuper
from pg_user
where usename = current_user
-- next
select t.oid as id,
       t.xmin as state_number,
       t.evtname as name,
       t.evtevent as event,
       t.evtfoid as routine_id,
       pg_catalog.pg_get_userbyid(t.evtowner) as owner,
       t.evttags as tags,
       case when t.evtenabled = 'D' then 1 else 0 end as is_disabled
from pg_catalog.pg_event_trigger t
--  where pg_catalog.age(t.xmin) <= #TXAGE
-- next
select fdw.oid as id,
       fdw.xmin as state_number,
       fdw.fdwname as name,
       pr.proname as handler,
       nspc.nspname as handler_schema,
       pr2.proname as validator,
       nspc2.nspname as validator_schema,
       fdw.fdwoptions as options,
       pg_catalog.pg_get_userbyid(fdw.fdwowner) as "owner"
from pg_catalog.pg_foreign_data_wrapper fdw
     left outer join pg_catalog.pg_proc pr on fdw.fdwhandler = pr.oid
     left outer join pg_catalog.pg_namespace nspc on pr.pronamespace = nspc.oid
     left outer join pg_catalog.pg_proc pr2 on fdw.fdwvalidator = pr2.oid
     left outer join pg_catalog.pg_namespace nspc2 on pr2.pronamespace = nspc2.oid
--  where pg_catalog.age(fdw.xmin) <= #TXAGE
-- next
select srv.oid as id,
       srv.srvfdw as fdw_id,
       srv.xmin as state_number,
       srv.srvname as name,
       srv.srvtype as type,
       srv.srvversion as version,
       srv.srvoptions as options,
       pg_catalog.pg_get_userbyid(srv.srvowner) as "owner"
from pg_catalog.pg_foreign_server srv
--  where pg_catalog.age(srv.xmin) <= #TXAGE
-- next
select umid as id,
       srvid as server_id,
       usename as user,
       umoptions as options
from pg_catalog.pg_user_mappings
order by server_id
-- next
select A.oid as access_method_id,
       A.xmin as state_number,
       A.amname as access_method_name
       ,
       A.amhandler::oid as handler_id,
       pg_catalog.quote_ident(N.nspname) || '.' || pg_catalog.quote_ident(P.proname) as handler_name,
       A.amtype as access_method_type
       
from pg_am A
     join pg_proc P on A.amhandler::oid = P.oid
     join pg_namespace N on P.pronamespace = N.oid
     
--  where pg_catalog.age(A.xmin) <= #TXAGE
-- next
select E.oid        as id,
       E.xmin       as state_number,
       extname      as name,
       extversion   as version,
       extnamespace as schema_id,
       nspname      as schema_name
       ,
       array(select unnest
             from unnest(available_versions)
             where unnest > extversion) as available_updates
       
from pg_catalog.pg_extension E
       join pg_namespace N on E.extnamespace = N.oid
       left join (select name, array_agg(version) as available_versions
                  from pg_available_extension_versions()
                  group by name) V on E.extname = V.name
       
--  where pg_catalog.age(E.xmin) <= #TXAGE
-- next
select l.oid as id, l.xmin state_number, lanname as name, lanpltrusted as trusted,
       h.proname as handler, hs.nspname as handlerSchema,
       i.proname as inline, isc.nspname as inlineSchema,
       v.proname as validator, vs.nspname as validatorSchema
from pg_catalog.pg_language l
    left join pg_catalog.pg_proc h on h.oid = lanplcallfoid
    left join pg_catalog.pg_namespace hs on hs.oid = h.pronamespace
    left join pg_catalog.pg_proc i on i.oid = laninline
    left join pg_catalog.pg_namespace isc on isc.oid = i.pronamespace
    left join pg_catalog.pg_proc v on v.oid = lanvalidator
    left join pg_catalog.pg_namespace vs on vs.oid = v.pronamespace
--  where pg_catalog.age(l.xmin) <= #TXAGE
order by lanname
-- next
select D.objoid id, case
    when 'pg_catalog.pg_event_trigger'::regclass = classoid then 'T'
    when 'pg_catalog.pg_am'::regclass = classoid then 'A'
    when 'pg_catalog.pg_cast'::regclass = classoid then 'C'
    when 'pg_catalog.pg_foreign_data_wrapper'::regclass = classoid then 'W'
    when 'pg_catalog.pg_foreign_server'::regclass = classoid then 'S'
    when 'pg_catalog.pg_language'::regclass = classoid then 'L'
    when 'pg_catalog.pg_extension'::regclass = classoid then 'E'
    
  end as kind,
  D.objsubid sub_id, D.description
from pg_catalog.pg_description D
where classoid in (
  'pg_catalog.pg_event_trigger'::regclass,
  'pg_catalog.pg_am'::regclass,
  'pg_catalog.pg_cast'::regclass,
  'pg_catalog.pg_foreign_data_wrapper'::regclass,
  'pg_catalog.pg_foreign_server'::regclass,
  'pg_catalog.pg_language'::regclass
  ,
  'pg_catalog.pg_extension'::regclass
  
)
--  and pg_catalog.age(D.xmin) <= #TXAGE
-- next
select T.oid as object_id,
                 T.fdwacl as acl
          from pg_catalog.pg_foreign_data_wrapper T 
          union all
          select T.oid as object_id,
                 T.lanacl as acl
          from pg_catalog.pg_language T 
          union all
          select T.oid as object_id,
                 T.nspacl as acl
          from pg_catalog.pg_namespace T 
          union all
          select T.oid as object_id,
                 T.srvacl as acl
          from pg_catalog.pg_foreign_server T
-- next
select C.oid,
       C.xmin as state_number,
       C.castsource as castsource_id,
       pg_catalog.quote_ident(SN.nspname) || '.' || pg_catalog.quote_ident(S.typname) as castsource_name,
       C.casttarget as casttarget_id,
       pg_catalog.quote_ident(TN.nspname) || '.' || pg_catalog.quote_ident(T.typname) as casttarget_name,
       C.castfunc as castfunc_id,
       pg_catalog.quote_ident(FN.nspname) || '.' || pg_catalog.quote_ident(F.proname) as castfunc_name,
       C.castcontext,
       C.castmethod
from pg_cast C
     left outer join pg_proc F on C.castfunc::oid = F.oid
     left outer join pg_namespace FN on F.pronamespace = FN.oid
     join pg_type S on C.castsource::oid = S.oid
     join pg_namespace SN on S.typnamespace = SN.oid
     join pg_type T on C.casttarget::oid = T.oid
     join pg_namespace TN on T.typnamespace = TN.oid
--  where pg_catalog.age(C.xmin) <= #TXAGE
-- next
select E.oid   as extension_id,
       D.objid as member_id
from pg_extension E
     join pg_depend D on E.oid = D.refobjid and
                         D.refclassid = 'pg_extension'::regclass::oid
where D.deptype = 'e'
order by extension_id
