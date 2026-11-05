import java.sql.*;
import java.util.Properties;
import java.nio.file.Files;
import java.nio.file.Path;

// No credentials in fixtures or command arguments: both come from the caller's environment.
class ConnectionProbe {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static void row(ResultSet rows, String value, String label) throws SQLException {
        check(rows.getMetaData().getColumnCount() == 1, "one probe column");
        check(rows.getMetaData().getColumnType(1) == Types.VARCHAR, "text probe type");
        check(rows.getMetaData().getColumnLabel(1).equals(label), "probe label");
        check(rows.next() && value.equals(rows.getString(1)), "exact probe value");
        check(!rows.next(), "one probe row");
    }

    static void drainMetadata(ResultSet rows) throws SQLException {
        ResultSetMetaData metadata = rows.getMetaData();
        for (int i = 1; i <= metadata.getColumnCount(); i++) {
            metadata.getColumnType(i);
            metadata.getColumnTypeName(i);
        }
        while (rows.next()) {
            for (int i = 1; i <= metadata.getColumnCount(); i++) rows.getObject(i);
        }
    }

    public static void main(String[] args) throws Exception {
        Properties properties = new Properties();
        properties.setProperty("user", System.getenv("OPEN_SDBL_JDBC_USER"));
        properties.setProperty("password", System.getenv("OPEN_SDBL_JDBC_PASSWORD"));
        properties.setProperty("sslmode", "disable");
        properties.setProperty("connectTimeout", "5");
        String socketTimeout = System.getenv("OPEN_SDBL_JDBC_SOCKET_TIMEOUT");
        properties.setProperty("socketTimeout", socketTimeout == null ? "10" : socketTimeout);
        properties.setProperty("preferQueryMode", args[1]);
        properties.setProperty("prepareThreshold", "1");
        properties.setProperty("binaryTransferEnable", "text");
        try (Connection connection = DriverManager.getConnection(args[0], properties)) {
            System.out.println("pgjdbc=" + connection.getMetaData().getDriverVersion());
            check(connection.isValid(2), "initial JDBC isValid");
            try (Statement statement = connection.createStatement()) {
                try (ResultSet rows = statement.executeQuery("SELECT 'keep alive'")) {
                    row(rows, "keep alive", "?column?");
                }
                try (ResultSet rows = statement.executeQuery("SELECT 'Hello'")) {
                    row(rows, "Hello", "?column?");
                }
            }
            String sql = "/* probe */ SELECT 'I''m alive; -- /* Привет' AS \"KeepAlive\";";
            try (PreparedStatement statement = connection.prepareStatement(sql)) {
                for (int i = 0; i < 3; i++) {
                    try (ResultSet rows = statement.executeQuery()) {
                        row(rows, "I'm alive; -- /* Привет", "KeepAlive");
                    }
                }
            }
            connection.setReadOnly(true);
            connection.setAutoCommit(false);
            try (PreparedStatement statement = connection.prepareStatement("SELECT 'transaction probe'")) {
                try (ResultSet rows = statement.executeQuery()) {
                    row(rows, "transaction probe", "?column?");
                }
            }
            connection.rollback();
            connection.setAutoCommit(true);
            if (args.length > 2) {
                Path schemaPath = Path.of(args[2]);
                String[] preSchemas = Files.readString(schemaPath.resolveSibling("datagrip-pre-schemas.sql")).split("\n-- next\n");
                String expectedDatabase = System.getenv().getOrDefault("OPEN_SDBL_JDBC_EXPECTED_DATABASE", "unf");
                String[] bootstrap = Files.readString(schemaPath.resolveSibling("datagrip-bootstrap.sql")).split("\n-- next\n");
                long startupEpoch = -1;
                for (int attempt = 0; attempt < 2; attempt++) {
                    for (int index = 0; index < bootstrap.length; index++) {
                        try (PreparedStatement statement = connection.prepareStatement(bootstrap[index]);
                             ResultSet rows = statement.executeQuery()) {
                            check(rows.next(), "bootstrap row");
                            if (index == 0) check(!rows.getBoolean(1), "plaintext SSL status");
                            if (index == 1) check(expectedDatabase.equals(rows.getString(1)), "configured database identity");
                            if (index == 2) {
                                check(rows.getMetaData().getColumnType(1) == Types.NUMERIC, "startup numeric type");
                                if (startupEpoch == -1) startupEpoch = rows.getLong(1);
                                check(startupEpoch > 0 && rows.getLong(1) == startupEpoch, "stable server startup epoch");
                            }
                            check(!rows.next(), "one bootstrap row");
                        }
                    }
                }
                String databaseSql = Files.readString(schemaPath.resolveSibling("datagrip-list-databases.sql"));
                try (PreparedStatement statement = connection.prepareStatement(databaseSql);
                     ResultSet rows = statement.executeQuery()) {
                    check(rows.next() && expectedDatabase.equals(rows.getString("name")), "configured pg_database name");
                    check(rows.getString("description") == null, "database description");
                    check(!rows.next(), "one served database");
                }
                System.out.println("PASS DataGrip bootstrap and ListDatabases: configured database=" + expectedDatabase);
                for (int i = 0; i < preSchemas.length; i++) {
                    try (PreparedStatement statement = connection.prepareStatement(preSchemas[i]);
                         ResultSet rows = statement.executeQuery()) {
                        drainMetadata(rows);
                    }
                }
                System.out.println("PASS DataGrip pre-schema session: " + preSchemas.length + " queries, all JDBC metadata and objects");
                String schemaSql = Files.readString(Path.of(args[2]));
                java.util.LinkedHashMap<Long, String> schemas = new java.util.LinkedHashMap<>();
                long revision = -1;
                long firstMillis = 0, repeatedMillis = 0;
                for (int attempt = 0; attempt < 3; attempt++) {
                    long started = System.nanoTime();
                    try (PreparedStatement statement = connection.prepareStatement(schemaSql);
                         ResultSet rows = statement.executeQuery()) {
                        ResultSetMetaData metadata = rows.getMetaData();
                        check(metadata.getColumnCount() == 5, "five schema columns");
                        check(metadata.getColumnType(1) == Types.BIGINT, "schema id type");
                        check(metadata.getColumnTypeName(2).equals("xid"), "schema revision type");
                        check(rows.next() && "public".equals(rows.getString("name")), "public schema first");
                        if (revision == -1) revision = rows.getLong("state_number");
                        check(revision >= 3, "virtual revision");
                        int count = 0;
                        do {
                            check(rows.getLong("id") > 0, "schema id");
                            schemas.put(rows.getLong("id"), rows.getString("name"));
                            check(rows.getLong("state_number") == revision, "stable schema revision");
                            check(rows.getString("owner") != null, "schema owner");
                            count++;
                        } while (rows.next());
                        check(count > 3, "metadata schemas are visible");
                    }
                    long millis = (System.nanoTime() - started) / 1_000_000;
                    if (attempt == 0) firstMillis = millis;
                    else repeatedMillis = Math.max(repeatedMillis, millis);
                }
                System.out.println("PASS DataGrip ListSchemas: exact query, xid, metadata schemas; first=" + firstMillis + "ms, repeated<=" + repeatedMillis + "ms");
                String[] objects = Files.readString(schemaPath.resolveSibling("datagrip-schema-objects.sql")).split("\n-- next\n");
                for (int i = 0; i < objects.length; i++) {
                    try (PreparedStatement statement = connection.prepareStatement(objects[i]); ResultSet rows = statement.executeQuery()) {
                        drainMetadata(rows);
                    }
                }
                String[] full = Files.readString(schemaPath.resolveSibling("datagrip-full-schema.sql")).split("\n-- next\n");
                int nativeTables = 0;
                int nativeSystemTables = 0;
                int fullQueries = 0;
                for (var schema : schemas.entrySet()) {
                    boolean systemSchema = schema.getValue().equals("pg_catalog") || schema.getValue().equals("information_schema");
                    for (int i = 0; i < full.length; i++) {
                        // The log omits bound values; rebind each schema/revision from ListSchemas.
                        String fullSql = full[i];
                        try (PreparedStatement statement = connection.prepareStatement(fullSql)) {
                            int parameter = 0;
                            for (int offset = 0; (offset = fullSql.indexOf("?::", offset)) >= 0; offset += 3) {
                                statement.setString(++parameter, Long.toString(fullSql.startsWith("?::oid", offset) ? schema.getKey() : revision));
                            }
                            try (ResultSet rows = statement.executeQuery()) {
                                if (i == 3) {
                                    ResultSetMetaData metadata = rows.getMetaData();
                                    for (int col = 1; col <= metadata.getColumnCount(); col++) metadata.getColumnTypeName(col);
                                    while (rows.next()) {
                                        check(rows.getString("table_name") != null, "native table name");
                                        if (systemSchema) nativeSystemTables++; else nativeTables++;
                                        for (int col = 1; col <= metadata.getColumnCount(); col++) rows.getObject(col);
                                    }
                                } else drainMetadata(rows);
                            } catch (SQLException error) {
                                throw new SQLException("full schema query " + i + " for " + schema.getValue() + ": " + error.getMessage(), error);
                            }
                        }
                        fullQueries++;
                    }
                }
                check(nativeTables > 0, "native DataGrip table discovery returns rows");
                System.out.println("PASS DataGrip full schema session: " + fullQueries + " queries; native tables=" + nativeTables + "; system tables=" + nativeSystemTables);
                try (ResultSet tables = connection.getMetaData().getTables(expectedDatabase, null, "%", new String[]{"TABLE", "VIEW"})) {
                    int count = 0;
                    while (tables.next()) {
                        // pgjdbc 42.7.3 projects NULL AS TABLE_CAT in getTables.
                        String tableCatalog = tables.getString("TABLE_CAT");
                        check(tableCatalog == null || expectedDatabase.equals(tableCatalog), "table catalog identity");
                        check(tables.getString("TABLE_NAME") != null, "table name");
                        count++;
                    }
                    check(count > 0, "visible metadata tables");
                    System.out.println("PASS DataGrip schema-object session: " + objects.length + " queries; JDBC getTables=" + count);
                }
            }
            check(connection.isValid(2), "final JDBC isValid");
            System.out.println("PASS " + args[1] + ": login, isValid, keep-alive, prepared/binary text, rollback");
        }
    }
}
