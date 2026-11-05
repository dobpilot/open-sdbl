import java.nio.charset.StandardCharsets;
import java.sql.*;
import java.util.Arrays;
import java.util.Properties;

// Uses a fake database engine through the real gateway; credentials come from the environment.
class DeferredPresentationProbe {
    private static final String EXPECTED = "Экземпляр бюджета 000000001 от 2023-12-14 14:57:01";
    private static final String SQL = "ВЫБРАТЬ ПЕРВЫЕ 10 ПредставлениеСсылки(Д.ДоговорКонтрагента) ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д";

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void checkRow(ResultSet rows) throws SQLException {
        ResultSetMetaData metadata = rows.getMetaData();
        check(metadata.getColumnCount() == 1, "one column");
        check(metadata.getColumnType(1) == Types.VARCHAR, "expected text, got " + metadata.getColumnTypeName(1));
        check("text".equals(metadata.getColumnTypeName(1)), "PostgreSQL text type");
        check(rows.next(), "one row");
        check(EXPECTED.equals(rows.getString(1)), "Unicode presentation differs");
        check(rows.getObject(1) instanceof String, "object is not a String");
        check(Arrays.equals(EXPECTED.getBytes(StandardCharsets.UTF_8), rows.getBytes(1)), "UTF-8 bytes differ");
        check(!rows.next(), "exactly one row");
    }

    public static void main(String[] args) throws Exception {
        Properties properties = new Properties();
        properties.setProperty("user", System.getenv("OPEN_SDBL_JDBC_USER"));
        properties.setProperty("password", System.getenv("OPEN_SDBL_JDBC_PASSWORD"));
        properties.setProperty("sslmode", "disable");
        properties.setProperty("connectTimeout", "5");
        properties.setProperty("socketTimeout", "10");
        properties.setProperty("preferQueryMode", args[1]);
        properties.setProperty("prepareThreshold", "1");
        properties.setProperty("binaryTransferEnable", "text");
        try (Connection connection = DriverManager.getConnection(args[0], properties)) {
            try (Statement statement = connection.createStatement(); ResultSet rows = statement.executeQuery(SQL)) {
                checkRow(rows);
            }
            try (PreparedStatement statement = connection.prepareStatement(SQL)) {
                for (int i = 0; i < 3; i++) {
                    try (ResultSet rows = statement.executeQuery()) { checkRow(rows); }
                }
            }
        }
        System.out.println("PASS " + args[1] + ": deferred presentation is UTF-8 text");
    }
}
