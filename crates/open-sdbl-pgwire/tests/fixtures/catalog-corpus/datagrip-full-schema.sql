select T.oid as oid,
       relnamespace as schemaId,
       pg_catalog.translate(relkind, 'rmvpfS', 'rmvrfS') as kind
from pg_catalog.pg_class T
where relnamespace in ( null )
  and relkind in ('r', 'm', 'v', 'p', 'f', 'S')
--
union all
-- begin objectType
select T.oid,
       T.typnamespace,
       'T' as kind
from pg_catalog.pg_type T
     left outer join pg_catalog.pg_class C on T.typrelid = C.oid
where T.typnamespace in ( null )
  and ( T.typtype in ('d','e') or
        C.relkind = 'c'::"char" or
        (T.typtype = 'b' and (T.typelem = 0 OR T.typcategory <> 'A')) or
        T.typtype = 'p' and not T.typisdefined )
--
union all
-- begin collation
select oid,
       collnamespace,
       'C' as kind
from pg_catalog.pg_collation
where collnamespace in ( null )
--
union all
-- begin ops
select oid,
       oprnamespace,
       'O' as kind
from pg_catalog.pg_operator
where oprnamespace in ( null )
--
union all
-- begin opClasses
select oid,
       opcnamespace,
       'c' as kind
from pg_catalog.pg_opclass
where opcnamespace in ( null )
--
union all
-- begin opFams
select oid,
       opfnamespace,
       'F' as kind
from pg_catalog.pg_opfamily
where opfnamespace in ( null )
--
union all
-- begin agg & routine
select oid,
       pronamespace,
       case when prokind != 'a' /* not proisagg */ then 'R'
            else 'a'
            end as kind
from pg_catalog.pg_proc
where pronamespace in ( null )
-- next
select cls.xmin as sequence_state_number,
       sq.seqrelid as sequence_id,
       cls.relname as sequence_name,
       pg_catalog.format_type(sq.seqtypid, null) as data_type,
       sq.seqstart as start_value,
       sq.seqincrement as inc_value,
       sq.seqmin as min_value,
       sq.seqmax as max_value,
       sq.seqcache as cache_size,
       sq.seqcycle as cycle_option,
       pg_catalog.pg_get_userbyid(cls.relowner) as "owner"
from pg_catalog.pg_sequence sq
    join pg_class cls on sq.seqrelid = cls.oid
    where cls.relnamespace = ?::oid
--  and pg_catalog.age(cls.xmin) <= #TXAGE
--  and cls.relname in ( :[*f_names] )
-- next
select T.oid as type_id,
       T.xmin as type_state_number,
       T.typname as type_name,
       T.typtype as type_sub_kind,
       T.typcategory as type_category,
       T.typrelid as class_id,
       T.typbasetype as base_type_id,
       case when T.typtype in ('c','e') then null
            else pg_catalog.format_type(T.typbasetype, T.typtypmod) end as type_def,
       T.typndims as dimensions_number,
       T.typdefault as default_expression,
       T.typnotnull as mandatory,
       pg_catalog.pg_get_userbyid(T.typowner) as "owner"
from pg_catalog.pg_type T
         left outer join pg_catalog.pg_class C
             on T.typrelid = C.oid
where T.typnamespace = ?::oid
  --  and T.typname in ( :[*f_names] )
  --  and pg_catalog.age(T.xmin) <= #TXAGE
  and (T.typtype in ('d','e') or
       C.relkind = 'c'::"char" or
       (T.typtype = 'b' and (T.typelem = 0 OR T.typcategory <> 'A')) or
       T.typtype = 'p' and not T.typisdefined)
order by 1
-- next
select T.relkind as table_kind,
       T.relname as table_name,
       T.oid as table_id,
       T.xmin as table_state_number,
       false /* T.relhasoids */ as table_with_oids,
       T.reltablespace as tablespace_id,
       T.reloptions as options,
       T.relpersistence as persistence,
       (select pg_catalog.array_agg(inhparent::bigint order by inhseqno)::varchar from pg_catalog.pg_inherits where T.oid = inhrelid) as ancestors,
       (select pg_catalog.array_agg(inhrelid::bigint order by inhrelid)::varchar from pg_catalog.pg_inherits where T.oid = inhparent) as successors,
       T.relispartition /* false */ as is_partition,
       pg_catalog.pg_get_partkeydef(T.oid) /* null */ as partition_key,
       pg_catalog.pg_get_expr(T.relpartbound, T.oid) /* null */ as partition_expression,
       T.relam am_id,
       pg_catalog.pg_get_userbyid(T.relowner) as "owner"
from pg_catalog.pg_class T
where relnamespace = ?::oid
       and relkind in ('r', 'm', 'v', 'f', 'p')
--  and pg_catalog.age(T.xmin) <= #TXAGE
--  and T.relname in ( :[*f_names] )
order by table_kind, table_id
-- next
select ft.ftrelid as table_id,
       srv.srvname as table_server,
       ft.ftoptions as table_options,
       pg_catalog.pg_get_userbyid(cls.relowner) as "owner"
from pg_catalog.pg_foreign_table ft
     left outer join pg_catalog.pg_foreign_server srv on ft.ftserver = srv.oid
     join pg_catalog.pg_class cls on ft.ftrelid = cls.oid
where cls.relnamespace = ?::oid
  --  and pg_catalog.age(ft.xmin) <= #TXAGE
  --  and cls.relname in ( :[*f_names] )
order by table_id
-- next
with schema_procs as (select prorettype, proargtypes, proallargtypes
                      from pg_catalog.pg_proc
                      where pronamespace = ?::oid
                        /* and pg_catalog.age(xmin) <= #TXAGE */ ),
     schema_opers as (select oprleft, oprright, oprresult
                      from pg_catalog.pg_operator
                      where oprnamespace = ?::oid
                        /* and pg_catalog.age(xmin) <= #TXAGE */ ),
     schema_aggregates as (select A.aggtranstype , A.aggmtranstype 
                           from pg_catalog.pg_aggregate A
                           join pg_catalog.pg_proc P
                             on A.aggfnoid = P.oid
                           where P.pronamespace = ?::oid
                           /* and (pg_catalog.age(A.xmin) <= #TXAGE or pg_catalog.age(P.xmin) <= #TXAGE) */),
     schema_arg_types as ( select prorettype as type_id
                           from schema_procs
                           union
                           select distinct unnest(proargtypes) as type_id
                           from schema_procs
                           union
                           select distinct unnest(proallargtypes) as type_id
                           from schema_procs
                           union
                           select oprleft as type_id
                           from schema_opers
                           where oprleft is not null
                           union
                           select oprright as type_id
                           from schema_opers
                           where oprright is not null
                           union
                           select oprresult as type_id
                           from schema_opers
                           where oprresult is not null
                           union
                           select aggtranstype::oid as type_id
                           from schema_aggregates
                           union
                           select aggmtranstype::oid as type_id
                           from schema_aggregates
                           
                           )
select type_id, pg_catalog.format_type(type_id, null) as type_spec
from schema_arg_types
where type_id <> 0 -- todo unclear how to frag
-- next
with languages as (select oid as lang_oid, lanname as lang
                   from pg_catalog.pg_language),
     routines as (select proname as r_name,
                         prolang as lang_oid,
                         oid as r_id,
                         xmin as r_state_number,
                         proargnames as arg_names,
                         proargmodes as arg_modes,
                         proargtypes::int[] as in_arg_types,
                         proallargtypes::int[] as all_arg_types,
                         pg_catalog.pg_get_expr(proargdefaults, 0) as arg_defaults,
                         provariadic as arg_variadic_id,
                         prorettype as ret_type_id,
                         proretset as ret_set,
                         prokind /* case when proiswindow then 'w'
                                                when proisagg then 'a'
                                                else 'f'
                                           end */ as kind,
                         provolatile as volatile_kind,
                         proisstrict as is_strict,
                         prosecdef as is_security_definer,
                         proconfig as configuration_parameters,
                         procost as cost,
                         pg_catalog.pg_get_userbyid(proowner) as "owner",
                         prorows as rows ,
                         proleakproof as is_leakproof  ,
                         proparallel as concurrency_kind 
                  from pg_catalog.pg_proc
                  where pronamespace = ?::oid
                    --  and proname in ( :[*f_names] )
                    and not (prokind = 'a') /* proisagg */
                    /* and pg_catalog.age(xmin) <= #TXAGE */)
select *
from routines natural join languages
-- next
select P.oid as aggregate_id,
       P.xmin as state_number,
       P.proname as aggregate_name,
       P.proargnames as arg_names,
       P.proargmodes as arg_modes,
       P.proargtypes::int[] as in_arg_types,
       P.proallargtypes::int[] as all_arg_types,
       A.aggtransfn::oid as transition_function_id,
       A.aggtransfn::regproc::text as transition_function_name,
       A.aggtranstype as transition_type,
       A.aggfinalfn::oid as final_function_id,
       case when A.aggfinalfn::oid = 0 then null else A.aggfinalfn::regproc::varchar end as final_function_name,
       case when A.aggfinalfn::oid = 0 then 0 else P.prorettype end as final_return_type,
       A.agginitval as initial_value,
       A.aggsortop as sort_operator_id,
       case when A.aggsortop = 0 then null else A.aggsortop::regoper::varchar end as sort_operator_name,
       pg_catalog.pg_get_userbyid(P.proowner) as "owner"
       ,
       A.aggfinalextra as final_extra,
       A.aggtransspace as state_size,
       A.aggmtransfn::oid as moving_transition_id,
       case when A.aggmtransfn::oid = 0 then null else A.aggmtransfn::regproc::varchar end as moving_transition_name,
       A.aggminvtransfn::oid as inverse_transition_id,
       case when A.aggminvtransfn::oid = 0 then null else A.aggminvtransfn::regproc::varchar end as inverse_transition_name,
       A.aggmtranstype::oid as moving_state_type,
       A.aggmtransspace as moving_state_size,
       A.aggmfinalfn::oid as moving_final_id,
       case when A.aggmfinalfn::oid = 0 then null else A.aggmfinalfn::regproc::varchar end as moving_final_name,
       A.aggmfinalextra as moving_final_extra,
       A.aggminitval as moving_initial_value,
       A.aggkind as aggregate_kind,
       A.aggnumdirectargs as direct_args
       
       ,
       A.aggcombinefn::oid as combine_function_id,
       case when A.aggcombinefn::oid = 0 then null else A.aggcombinefn::regproc::varchar end as combine_function_name,
       A.aggserialfn::oid as serialization_function_id,
       case when A.aggserialfn::oid = 0 then null else A.aggserialfn::regproc::varchar end as serialization_function_name,
       A.aggdeserialfn::oid as deserialization_function_id,
       case when A.aggdeserialfn::oid = 0 then null else A.aggdeserialfn::regproc::varchar end as deserialization_function_name,
       P.proparallel as concurrency_kind
       
from pg_catalog.pg_aggregate A
     join pg_catalog.pg_proc P
       on A.aggfnoid = P.oid
where P.pronamespace = ?::oid
  --  and P.proname in ( :[*f_names] )
  --  and (pg_catalog.age(A.xmin) <= #TXAGE or pg_catalog.age(P.xmin) <= #TXAGE)
order by P.oid
-- next
select O.oid as op_id,
       O.xmin as state_number,
       oprname as op_name,
       oprkind as op_kind,
       oprleft as arg_left_type_id,
       oprright as arg_right_type_id,
       oprresult as arg_result_type_id,
       oprcode::oid as main_id,
       oprcode::varchar as main_name,
       oprrest::oid as restrict_id,
       oprrest::varchar as restrict_name,
       oprjoin::oid as join_id,
       oprjoin::varchar as join_name,
       oprcom::oid as com_id,
       oprcom::regoper::varchar as com_name,
       oprnegate::oid as neg_id,
       oprnegate::regoper::varchar as neg_name,
       oprcanmerge as merges,
       oprcanhash as hashes,
       pg_catalog.pg_get_userbyid(O.oprowner) as "owner"
from pg_catalog.pg_operator O
where oprnamespace = ?::oid
  --  and oprname in ( :[*f_names] )
  --  and pg_catalog.age(xmin) <= #TXAGE
-- next
select oid as id,
       xmin as state_number,
       collname as name,
       collcollate as lc_collate,
       collctype as lc_ctype,
       pg_catalog.pg_get_userbyid(collowner) as "owner"
from pg_catalog.pg_collation
where collnamespace = ?::oid
  --  and collname in ( :[*f_names] )
  --  and pg_catalog.age(xmin) <= #TXAGE
-- next
select O.oid as id,
       O.xmin as state_number,
       opcname as name,
       opcintype::regtype::varchar as in_type,
       case when opckeytype = 0 then null else opckeytype::regtype::varchar end as key_type,
       opcdefault as is_default,
       opcfamily as family_id,
       opfname as family,
       opcmethod as access_method_id,
       pg_catalog.pg_get_userbyid(O.opcowner) as "owner"
from pg_catalog.pg_opclass O
     join pg_catalog.pg_opfamily F on F.oid = opcfamily
where opcnamespace = ?::oid
  --  and opcname in ( :[*f_names] )
  --  and pg_catalog.age(O.xmin) <= #TXAGE
-- next
select O.oid as id,
       O.xmin as state_number,
       opfname as name,
       opfmethod as access_method_id,
       pg_catalog.pg_get_userbyid(O.opfowner) as "owner"
from pg_catalog.pg_opfamily O
where opfnamespace = ?::oid
  --  and opfname in ( :[*f_names] )
  --  and pg_catalog.age(xmin) <= #TXAGE
-- next
select pg_amop.oid
from pg_catalog.pg_amop
         join pg_catalog.pg_opfamily on pg_opfamily.oid = amopfamily
where opfnamespace = ?::oid
-- next
select pg_amproc.oid
from pg_catalog.pg_amproc
         join pg_catalog.pg_opfamily on pg_opfamily.oid = amprocfamily
where opfnamespace = ?::oid
-- next
select IX.indexrelid
from pg_catalog.pg_index IX,
     pg_catalog.pg_class IC
where IC.oid = IX.indrelid
  and IC.relnamespace = ?::oid
-- next
select tab.oid               table_id,
       tab.relkind           table_kind,
       ind_stor.relname      index_name,
       ind_head.indexrelid   index_id,
       ind_stor.xmin         state_number,
       ind_head.indisunique  is_unique,
       ind_head.indisprimary is_primary,
       /* ind_head.indnullsnotdistinct */false  nulls_not_distinct,
       pg_catalog.pg_get_expr(ind_head.indpred, ind_head.indrelid) as condition,
       (select pg_catalog.array_agg(inhparent::bigint order by inhseqno)::varchar from pg_catalog.pg_inherits where ind_stor.oid = inhrelid) as ancestors,
       ind_stor.reltablespace tablespace_id,
       opcmethod as access_method_id
from pg_catalog.pg_class tab
         join pg_catalog.pg_index ind_head
              on ind_head.indrelid = tab.oid
         join pg_catalog.pg_class ind_stor
              on tab.relnamespace = ind_stor.relnamespace and ind_stor.oid = ind_head.indexrelid
         left join pg_catalog.pg_opclass on pg_opclass.oid = ANY(indclass)
where tab.relnamespace = ?::oid
        and tab.relkind in ('r', 'm', 'v', 'p')
        and ind_stor.relkind in ('i', 'I')
--  and tab.relname in ( :[*f_names] )
--  and pg_catalog.age(ind_stor.xmin) <= #TXAGE
-- next
select oid
from pg_catalog.pg_constraint
where conrelid != 0 and connamespace = ?::oid
-- next
select T.oid table_id,
       relkind table_kind,
       C.oid::bigint con_id,
       C.xmin::varchar::bigint con_state_id,
       conname con_name,
       contype con_kind,
       conkey con_columns,
       conindid index_id,
       confrelid ref_table_id,
       condeferrable is_deferrable,
       condeferred is_init_deferred,
       confupdtype on_update,
       confdeltype on_delete,
       connoinherit no_inherit,
       pg_catalog.pg_get_expr(conbin, T.oid) /* consrc */ con_expression,
       confkey ref_columns,
       conexclop::int[] excl_operators,
       array(select unnest::regoper::varchar from unnest(conexclop)) excl_operators_str
from pg_catalog.pg_constraint C
         join pg_catalog.pg_class T
              on C.conrelid = T.oid
where relkind in ('r', 'v', 'f', 'p')
  and relnamespace = ?::oid
  and contype in ('p', 'u', 'f', 'c', 'x')
  and connamespace = ?::oid
  --  and pg_catalog.age(T.xmin) <= #TXAGE or pg_catalog.age(c.xmin) <= #TXAGE
-- next
select RU.oid
from pg_catalog.pg_rewrite RU,
     pg_catalog.pg_class RC
where RC.oid = RU.ev_class
  and RC.relnamespace = ?::oid
  and not RU.rulename = '_RETURN'
-- next
select P.oid
from pg_catalog.pg_policy P
     join pg_catalog.pg_class C on polrelid = C.oid
where relnamespace = ?::oid
-- next
select
       P.oid id,
       P.xmin as state_number,
       polname policyname,
       polrelid table_id,
       polpermissive /* true */ as permissive,
       polroles roles,
       polcmd cmd,
       pg_get_expr(polqual, polrelid) qual,
       pg_get_expr(polwithcheck, polrelid) with_check
from pg_catalog.pg_policy P
       join pg_catalog.pg_class C on polrelid = C.oid
where relnamespace = ?::oid
  --  and C.relname in ( :[*f_names] )
  --  and pg_catalog.age(P.xmin) <= #TXAGE
order by polrelid
-- next
select TG.oid
from pg_catalog.pg_trigger TG,
     pg_catalog.pg_class TC
where TC.oid = TG.tgrelid
  and TC.relnamespace = ?::oid
-- next
select D.objoid id, pg_catalog.array_agg(D.objsubid) sub_ids
from pg_catalog.pg_description D
  join pg_catalog.pg_class C on D.objoid = C.oid
where C.relnamespace = ?::oid and C.relkind != 'c' and D.classoid = 'pg_catalog.pg_class'::regclass
group by D.objoid
union all
select T.oid id, pg_catalog.array_agg(D.objsubid)
from pg_catalog.pg_description D
  join pg_catalog.pg_type T on T.oid = D.objoid or T.typrelid = D.objoid
  left join pg_catalog.pg_class C on T.typrelid = C.oid
where T.typnamespace = ?::oid and (C.relkind = 'c' or C.relkind is null)
group by T.oid
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_constraint C on D.objoid = C.oid
where C.connamespace = ?::oid and D.classoid = 'pg_catalog.pg_constraint'::regclass
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_trigger T on T.oid = D.objoid
  join pg_catalog.pg_class C on C.oid = T.tgrelid
where C.relnamespace = ?::oid and D.classoid = 'pg_catalog.pg_trigger'::regclass
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_rewrite R on R.oid = D.objoid
  join pg_catalog.pg_class C on C.oid = R.ev_class
where C.relnamespace = ?::oid and D.classoid = 'pg_catalog.pg_rewrite'::regclass
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_proc P on P.oid = D.objoid
where P.pronamespace = ?::oid and D.classoid = 'pg_catalog.pg_proc'::regclass
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_operator O on O.oid = D.objoid
where O.oprnamespace = ?::oid and D.classoid = 'pg_catalog.pg_operator'::regclass
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_opclass O on O.oid = D.objoid
where O.opcnamespace = ?::oid and D.classoid = 'pg_catalog.pg_opclass'::regclass
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_opfamily O on O.oid = D.objoid
where O.opfnamespace = ?::oid and D.classoid = 'pg_catalog.pg_opfamily'::regclass
union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_collation C on C.oid = D.objoid
where C.collnamespace = ?::oid and D.classoid = 'pg_catalog.pg_collation'::regclass

union all
select D.objoid id, array[D.objsubid]
from pg_catalog.pg_description D
  join pg_catalog.pg_policy P on P.oid = D.objoid
  join pg_catalog.pg_class C on P.polrelid = C.oid
where C.relnamespace = ?::oid  and D.classoid = 'pg_catalog.pg_policy'::regclass

-- next
select T.oid as object_id,
                 T.relacl as acl
          from pg_catalog.pg_class T
          where relnamespace = ?::oid 
            and pg_catalog.age(T.xmin) <= pg_catalog.age(?::varchar::xid)
          union all
          select T.oid as object_id,
                 T.proacl as acl
          from pg_catalog.pg_proc T
          where pronamespace = ?::oid 
            and pg_catalog.age(T.xmin) <= pg_catalog.age(?::varchar::xid)
          union all
          select T.oid as object_id,
                 T.typacl as acl
          from pg_catalog.pg_type T
          where typnamespace = ?::oid 
            and pg_catalog.age(T.xmin) <= pg_catalog.age(?::varchar::xid)
          order by object_id
-- next
select T.oid as object_id,
               A.attnum as attr_position,
               A.attacl as acl
        from pg_catalog.pg_attribute A join pg_catalog.pg_class T on T.oid = A.attrelid
        where relnamespace = ?::oid
          and attnum > 0 
          and pg_catalog.age(A.xmin) <= pg_catalog.age(?::varchar::xid)
        order by object_id, attr_position
-- next
select D.objid as dependent_id,
       D.refobjid as owner_id,
       D.refobjsubid as owner_subobject_id
from pg_depend D
  join pg_class C_SEQ on D.objid    = C_SEQ.oid and D.classid    = 'pg_class'::regclass::oid
  join pg_class C_TAB on D.refobjid = C_TAB.oid and D.refclassid = 'pg_class'::regclass::oid
where C_SEQ.relkind = 'S'
  and C_TAB.relkind = 'r'
  and D.refobjsubid <> 0
  and (D.deptype = 'a' or D.deptype = 'i')
  and C_TAB.relnamespace = ?::oid
order by owner_id
-- next
--  select type_id, att_position, att_name, att_type, att_dropped, dimensions_number from (
select T.oid as type_id,
       A.attnum as att_position,
       A.attname as att_name,
       pg_catalog.format_type(A.atttypid, A.atttypmod) as att_type,
       A.attndims as dimensions_number,
       A.attisdropped as att_dropped
--  , min(pg_catalog.age(A.xmin)) over (partition by a.attrelid) as age
from pg_catalog.pg_type T
  join pg_catalog.pg_class C
    on T.typrelid = C.oid
  join pg_catalog.pg_attribute A
    on A.attrelid = C.oid
where T.typnamespace = ?::oid
  and T.typtype = 'c'::"char"
  and C.relkind = 'c'::"char"
--  and T.typname in ( :[*f_names] )
--  ) sub where age <= #TXAGE
order by /* type_id*/T.oid::bigint, /* att_position*/A.attnum
-- next
with types as (select distinct T.oid as type_id
               from pg_catalog.pg_type T
               join pg_catalog.pg_enum E
                 on T.oid = E.enumtypid
               where T.typnamespace = ?::oid
                 and T.typtype = 'e'::"char"
               --  and T.typname in ( :[*f_names] )
               --  and pg_catalog.age(E.xmin) <= #TXAGE
               )
select T.type_id as enum_id,
       pg_catalog.array_agg(E.enumlabel::varchar order by E.enumsortorder ) as enum_labels
from types T
join pg_catalog.pg_enum E
  on T.type_id = E.enumtypid
group by T.type_id
-- next
select T.relkind as view_kind,
       T.oid as view_id,
       pg_catalog.pg_get_viewdef(T.oid, true) as source_text
from pg_catalog.pg_class T
     join pg_catalog.pg_namespace N on T.relnamespace = N.oid
where N.oid = ?::oid
  and T.relkind in ('m','v')
  --  and T.relname in ( :[*f_names] )
  and (pg_catalog.age(T.xmin) <= coalesce(nullif(greatest(pg_catalog.age(?::varchar::xid), -1), -1), 2147483647) or exists(
  select A.attrelid from pg_catalog.pg_attribute A where A.attrelid = T.oid and pg_catalog.age(A.xmin) <= coalesce(nullif(greatest(pg_catalog.age(?::varchar::xid), -1), -1), 2147483647)))
-- next
with system_languages as ( select oid as lang
                           from pg_catalog.pg_language
                           where lanname in ('c','internal') )
select oid as id,
       pg_catalog.pg_get_function_arguments(oid) as arguments_def,
       pg_catalog.pg_get_function_result(oid) as result_def,
       /* pg_catalog.pg_get_function_sqlbody(oid) */null  as sqlbody_def,
       prosrc as source_text
from pg_catalog.pg_proc
where pronamespace = ?::oid
  --  and pg_proc.proname in ( :[*f_names] )
  --  and pg_catalog.age(xmin) <= #SRCTXAGE
  and not (prokind = 'a') /* proisagg */
  and prolang not in (select lang from system_languages)
  and prosrc is not null
