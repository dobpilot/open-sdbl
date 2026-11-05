#!/usr/bin/env bash
# Records the catalog-introspection corpus of the supported clients.
#
# Starts postgres:13 with log_statement=all and log_line_prefix='%a|'
# (the application name tags every statement), creates a small schema
# shaped like a 1C catalog, runs each client, and extracts the statements
# into <client>.sql next to this script. Needs podman (or docker), psql,
# java 17+ and network access to Maven Central, PyPI and Debian mirrors.
#
#   ./record.sh            # every automated client
#   ./record.sh dbeaver    # keep the server up for a manual DBeaver session
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
engine="$(command -v podman || command -v docker)"
work="$(mktemp -d)"
port=55432
url="jdbc:postgresql://127.0.0.1:${port}/postgres"

"$engine" rm -f sdbl-corpus >/dev/null 2>&1 || true
"$engine" run -d --name sdbl-corpus -e POSTGRES_PASSWORD=corpus -p "${port}:5432" \
  docker.io/library/postgres:13 -c log_statement=all -c log_destination=stderr \
  -c "log_line_prefix=%a|" >/dev/null
until "$engine" exec sdbl-corpus pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done
"$engine" exec -i sdbl-corpus psql -U postgres -q <<'SQL'
CREATE SCHEMA "Справочник";
CREATE SCHEMA "РегистрНакопления";
CREATE TABLE "Справочник"."Номенклатура" ("Ссылка" bytea, "Наименование" varchar(150),
  "Цена" numeric(15,2), "Пометка" boolean, "Дата" timestamp, "Ид" uuid, "Описание" text);
COMMENT ON TABLE "Справочник"."Номенклатура" IS 'Номенклатура';
COMMENT ON COLUMN "Справочник"."Номенклатура"."Наименование" IS 'Наименование товара';
CREATE TABLE "Справочник"."Номенклатура.Товары" ("Ссылка" bytea, "НомерСтроки" numeric(5,0));
CREATE VIEW "РегистрНакопления"."ЗамерОстатков.Остатки" AS
  SELECT 'a'::varchar(12) AS "Категория", 1.5::numeric(15,2) AS "КоличествоОстаток";
SQL

commands='\dn
\dt
\dt Справочник.*
\dt "Справочник".Номенклат*
\dv
\dv РегистрНакопления.*
\d "Справочник"."Номенклатура"
\d+ "Справочник"."Номенклатура"
\d "РегистрНакопления"."ЗамерОстатков.Остатки"
\dT
\d'

record_psql() {
  printf '%s\n' "$commands" | "$engine" exec -i -e PGAPPNAME=psql13 sdbl-corpus psql -U postgres -X -f - >/dev/null 2>&1 || true
  printf '%s\n' "$commands" | PGPASSWORD=corpus PGAPPNAME=psql18 psql -h 127.0.0.1 -p "$port" -U postgres -X -f - >/dev/null 2>&1 || true
}

record_pgjdbc() {
  (cd "$work" && curl -sSfLO https://repo1.maven.org/maven2/org/postgresql/postgresql/42.7.4/postgresql-42.7.4.jar)
  cat > "$work/Corpus.java" <<JAVA
import java.sql.*;
public class Corpus {
  public static void main(String[] a) throws Exception {
    try (Connection c = DriverManager.getConnection("${url}?ApplicationName=pgjdbc", "postgres", "corpus")) {
      DatabaseMetaData m = c.getMetaData();
      drain(m.getSchemas()); drain(m.getSchemas(null, "Справ%")); drain(m.getCatalogs());
      drain(m.getTableTypes());
      drain(m.getTables(null, null, "%", new String[]{"TABLE", "VIEW"}));
      drain(m.getTables(null, "Справочник", "Номенклатура", null));
      drain(m.getColumns(null, "Справочник", "Номенклатура", "%"));
      drain(m.getColumns(null, null, "%", null));
      drain(m.getPrimaryKeys(null, "Справочник", "Номенклатура"));
      drain(m.getImportedKeys(null, "Справочник", "Номенклатура"));
      drain(m.getIndexInfo(null, "Справочник", "Номенклатура", false, true));
      drain(m.getTypeInfo()); drain(m.getProcedures(null, "Справочник", "%"));
      try (PreparedStatement p = c.prepareStatement(
          "SELECT \"Цена\", \"Дата\", \"Ид\" FROM \"Справочник\".\"Номенклатура\" WHERE \"Наименование\" = ?")) {
        p.setString(1, "x"); drain(p.executeQuery());
        ResultSetMetaData r = p.getMetaData();
        for (int i = 1; i <= r.getColumnCount(); i++) {
          r.getColumnTypeName(i); r.isNullable(i); r.getTableName(i); r.getSchemaName(i); r.isAutoIncrement(i);
        }
      }
    }
  }
  static void drain(ResultSet r) throws SQLException { while (r.next()) {} r.close(); }
}
JAVA
  (cd "$work" && javac -cp postgresql-42.7.4.jar Corpus.java && java -cp postgresql-42.7.4.jar:. Corpus)
}

record_psycopg() {
  python3 -m venv "$work/venv"
  "$work/venv/bin/pip" install -q 'psycopg[binary]==3.2.*'
  "$work/venv/bin/python" - <<PY
import psycopg
from psycopg.types import TypeInfo
with psycopg.connect("host=127.0.0.1 port=${port} user=postgres password=corpus dbname=postgres application_name=psycopg") as c:
    for name in ("numeric", "text", "bytea", "uuid", "timestamp", "varchar"):
        TypeInfo.fetch(c, name)
    c.execute('SELECT * FROM "Справочник"."Номенклатура"').fetchall()
PY
}

record_odbc() {
  "$engine" run --rm --network host docker.io/library/debian:bookworm bash -c "
    apt-get -qq update >/dev/null && apt-get -qq install -y odbc-postgresql unixodbc python3-pyodbc >/dev/null &&
    python3 - <<'PY'
import pyodbc
c = pyodbc.connect('DRIVER={PostgreSQL Unicode};SERVER=127.0.0.1;PORT=${port};DATABASE=postgres;UID=postgres;PWD=corpus;ConnSettings=SET application_name%3D%27psqlodbc%27')
cur = c.cursor()
list(cur.tables()); list(cur.tables(schema='Справочник'))
list(cur.columns(table='Номенклатура', schema='Справочник'))
list(cur.primaryKeys('Номенклатура', schema='Справочник'))
PY"
}

clients=("${@:-psql pgjdbc psycopg odbc}")
for client in ${clients[@]}; do
  case "$client" in
    psql) record_psql ;;
    pgjdbc) record_pgjdbc ;;
    psycopg) record_psycopg ;;
    odbc) record_odbc ;;
    dbeaver)
      echo "Connect DBeaver to 127.0.0.1:${port} as postgres/corpus with application name 'dbeaver'"
      echo "(Driver properties → ApplicationName), expand schemas, open Номенклатура → Columns, then press Enter."
      read -r ;;
  esac
done

"$engine" logs sdbl-corpus 2>"$work/server.log" >/dev/null
python3 -I "$here/extract.py" "$work/server.log" "$here" psql13 psql18 pgjdbc psycopg psqlodbc dbeaver
"$engine" rm -f sdbl-corpus >/dev/null
rm -rf "$work"
