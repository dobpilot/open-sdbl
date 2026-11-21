-- Minimal synthetic information base. The resource payloads come from
-- open-sdbl-db/src/tests/configuration.rs and tests/access.rs.
CREATE TABLE params (filename text, binarydata bytea);
INSERT INTO params VALUES ('DBNames', decode('ab36d4a94eb64832324c4b4dd4354c354ed135494cb5d4b53037b4d4354f4b33494932b0484cb334d75172cd2bcd55d2b1b4acad0500','hex'));

CREATE TABLE config (filename text, binarydata bytea);
INSERT INTO config VALUES ('25c96bd3-fac4-42ef-b695-74c9af43589b', decode('4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f','hex'));
INSERT INTO config
SELECT '00000000-0000-0000-0000-' || lpad(i::text, 12, '0') || '.7', decode('fbff0f00', 'hex')
FROM generate_series(1, 4) AS i;

CREATE TABLE schemastorage (schemaid int, currentschema bytea);
INSERT INTO schemastorage VALUES (0, convert_to(
'{0,{1,{"Reference53","N",53,"",{6,{"ID",0,{1,{"R",0,0,"Reference53",2}},"",0},{"Code",0,{1,{"S",2147483657,0,"",0}},"",0},{"Description",0,{1,{"S",2147483657,0,"",0}},"",0},{"Marked",0,{1,{"B",16,0,"",0}},"",0},{"Fld54",0,{1,{"B",16,0,"",0}},"",0},{"Date_Time",0,{1,{"T",0,0,"",0}},"",0}},{0},{0},1,"R",{0},{0},"",0}}}',
'UTF8'));

CREATE TABLE v8users (name text, descr text, osname text, show boolean,
                      eauth boolean, admrole boolean, data bytea);
