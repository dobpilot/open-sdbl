#!/usr/bin/env bash
set -euo pipefail

trino_command=${TRINO_COMMAND:-trino}
catalog=${ONEC_CATALOG:-onec}
schema=${ONEC_SCHEMA:-Справочник}
table=${ONEC_TABLE:-Контрагенты}
filter_value=${ONEC_FILTER_VALUE:-7701234567}
escaped_filter_value=${filter_value//\'/\'\'}

"$trino_command" --execute "SHOW SCHEMAS FROM \"$catalog\""
"$trino_command" --execute "SHOW TABLES FROM \"$catalog\".\"$schema\""
"$trino_command" --execute "DESCRIBE \"$catalog\".\"$schema\".\"$table\""
"$trino_command" --execute "SELECT \"Код\", \"Наименование\" FROM \"$catalog\".\"$schema\".\"$table\" LIMIT 10"
"$trino_command" --execute "SELECT \"Код\", \"Наименование\", \"ИНН\" FROM \"$catalog\".\"$schema\".\"$table\" WHERE \"ИНН\" = '$escaped_filter_value' LIMIT 10"
