package io.open_sdbl.trino;

import com.fasterxml.jackson.databind.JsonNode;
import io.airlift.slice.Slice;
import io.airlift.slice.Slices;
import io.trino.spi.TrinoException;
import io.trino.spi.connector.ColumnHandle;
import io.trino.spi.connector.ConnectorRecordSetProvider;
import io.trino.spi.connector.ConnectorSession;
import io.trino.spi.connector.ConnectorSplit;
import io.trino.spi.connector.ConnectorTableHandle;
import io.trino.spi.connector.ConnectorTransactionHandle;
import io.trino.spi.connector.RecordCursor;
import io.trino.spi.connector.RecordSet;
import io.trino.spi.type.DecimalType;
import io.trino.spi.type.Decimals;
import io.trino.spi.type.Type;
import io.trino.spi.type.TypeManager;
import io.trino.spi.type.UuidType;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.ZoneOffset;
import java.util.Base64;
import java.util.List;
import java.util.UUID;

import static io.trino.spi.StandardErrorCode.GENERIC_INTERNAL_ERROR;

/** Creates pull-based Trino record cursors over the service's NDJSON row streams. */
final class OpenSdblRecordSetProvider
        implements ConnectorRecordSetProvider
{
    private final OpenSdblClient client;
    private final TypeManager types;

    OpenSdblRecordSetProvider(OpenSdblClient client, TypeManager types)
    {
        this.client = client;
        this.types = types;
    }

    @Override
    public RecordSet getRecordSet(
            ConnectorTransactionHandle transaction,
            ConnectorSession session,
            ConnectorSplit split,
            ConnectorTableHandle handle,
            List<? extends ColumnHandle> columns)
    {
        Model.TableHandle table = (Model.TableHandle) handle;
        if (table.isSdblQuery()) {
            List<Model.SdblColumnHandle> selected = columns.stream()
                    .map(Model.SdblColumnHandle.class::cast)
                    .toList();
            List<Type> columnTypes = selected.stream()
                    .map(column -> Types.resolve(types, column.type()))
                    .toList();
            return sdblRecordSet(table, selected, columnTypes);
        }

        List<Model.Column> selected = columns.stream()
                .map(Model.Column.class::cast)
                .toList();
        List<Type> columnTypes = selected.stream()
                .map(column -> Types.resolve(types, column.type()))
                .toList();
        return tableRecordSet(table, selected, columnTypes);
    }

    private RecordSet sdblRecordSet(
            Model.TableHandle table,
            List<Model.SdblColumnHandle> selected,
            List<Type> columnTypes)
    {
        return new RecordSet()
        {
            @Override
            public List<Type> getColumnTypes()
            {
                return columnTypes;
            }

            @Override
            public RecordCursor cursor()
            {
                Model.SdblScanRequest request = new Model.SdblScanRequest(
                        table.sdblQuery(),
                        table.sdblColumns(),
                        selected.stream()
                                .map(Model.SdblColumnHandle::index)
                                .toList(),
                        table.limit());
                return new Cursor(client, client.scanSdbl(request), columnTypes);
            }
        };
    }

    private RecordSet tableRecordSet(
            Model.TableHandle table,
            List<Model.Column> selected,
            List<Type> columnTypes)
    {
        return new RecordSet()
        {
            @Override
            public List<Type> getColumnTypes()
            {
                return columnTypes;
            }

            @Override
            public RecordCursor cursor()
            {
                Model.ScanRequest request = new Model.ScanRequest(
                        table.schema(),
                        table.table(),
                        selected.stream()
                                .map(Model.Column::name)
                                .toList(),
                        table.filters(),
                        table.limit());
                return new Cursor(client, client.scan(request), columnTypes);
            }
        };
    }

    /**
     * Decodes one NDJSON message at a time, preserving backpressure through Trino's pull model.
     *
     * <p>Rows stay as strings until Trino requests a typed field. This mirrors the Rust wire
     * representation and avoids materializing an additional typed row object.
     */
    private static final class Cursor
            implements RecordCursor
    {
        private final OpenSdblClient client;
        private final BufferedReader input;
        private final List<Type> types;
        private String[] row;

        Cursor(OpenSdblClient client, InputStream input, List<Type> types)
        {
            this.client = client;
            this.input = new BufferedReader(new InputStreamReader(input, StandardCharsets.UTF_8));
            this.types = types;
        }

        @Override
        public long getCompletedBytes()
        {
            return 0;
        }

        @Override
        public long getReadTimeNanos()
        {
            return 0;
        }

        @Override
        public Type getType(int field)
        {
            return types.get(field);
        }

        @Override
        public boolean advanceNextPosition()
        {
            try {
                for (String line; (line = input.readLine()) != null; ) {
                    JsonNode message = client.json().readTree(line);
                    String kind = message.path("kind").asText();
                    if (kind.equals("stats")) {
                        return false;
                    }
                    if (kind.equals("error")) {
                        String code = message.path("code").asText("internal");
                        throw new TrinoException(
                                GENERIC_INTERNAL_ERROR,
                                "open-sdbl " + code + ": " + message.path("message").asText());
                    }
                    if (kind.equals("row")) {
                        JsonNode values = message.path("row");
                        row = new String[values.size()];
                        for (int index = 0; index < row.length; index++) {
                            row[index] = values.get(index).isNull() ? null : values.get(index).asText();
                        }
                        return true;
                    }
                }
                return false;
            }
            catch (IOException error) {
                throw new TrinoException(GENERIC_INTERNAL_ERROR, "open-sdbl row stream failed", error);
            }
        }

        @Override
        public boolean getBoolean(int field)
        {
            return Boolean.parseBoolean(value(field));
        }

        @Override
        public long getLong(int field)
        {
            Type type = types.get(field);
            String value = value(field);
            String base = type.getTypeSignature().getBase();
            if (base.equals("date")) {
                return LocalDate.parse(value).toEpochDay();
            }
            if (base.equals("timestamp")) {
                LocalDateTime dateTime = LocalDateTime.parse(value.replace(' ', 'T'));
                return dateTime.toEpochSecond(ZoneOffset.UTC) * 1_000_000 + dateTime.getNano() / 1_000;
            }
            if (type instanceof DecimalType decimal) {
                return new BigDecimal(value)
                        .setScale(decimal.getScale())
                        .unscaledValue()
                        .longValueExact();
            }
            return Long.parseLong(value);
        }

        @Override
        public double getDouble(int field)
        {
            return Double.parseDouble(value(field));
        }

        @Override
        public Slice getSlice(int field)
        {
            Type type = types.get(field);
            String value = value(field);
            String base = type.getTypeSignature().getBase();
            if (base.equals("varbinary")) {
                return Slices.wrappedBuffer(Base64.getDecoder().decode(value));
            }
            if (base.equals("uuid")) {
                return UuidType.javaUuidToTrinoUuid(parseUuid(value));
            }
            return Slices.utf8Slice(value);
        }

        @Override
        public Object getObject(int field)
        {
            Type type = types.get(field);
            if (type instanceof DecimalType decimal && !decimal.isShort()) {
                return Decimals.encodeScaledValue(new BigDecimal(value(field)), decimal.getScale());
            }
            throw new UnsupportedOperationException("object value for " + type);
        }

        @Override
        public boolean isNull(int field)
        {
            return row[field] == null;
        }

        @Override
        public void close()
        {
            try {
                input.close();
            }
            catch (IOException ignored) {
                // There is no recovery action after Trino has stopped consuming the cursor.
            }
        }

        private String value(int field)
        {
            if (isNull(field)) {
                throw new IllegalStateException("field is null");
            }
            return row[field];
        }

        private static UUID parseUuid(String value)
        {
            String hex = value.replace("-", "");
            String canonical = hex.substring(0, 8) + "-" +
                    hex.substring(8, 12) + "-" +
                    hex.substring(12, 16) + "-" +
                    hex.substring(16, 20) + "-" +
                    hex.substring(20);
            return UUID.fromString(canonical);
        }
    }
}
