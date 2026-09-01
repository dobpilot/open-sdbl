package io.open_sdbl.trino;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;
import io.trino.spi.connector.ColumnHandle;
import io.trino.spi.connector.ConnectorSplit;
import io.trino.spi.connector.ConnectorTableHandle;
import io.trino.spi.connector.ConnectorTransactionHandle;
import io.trino.spi.function.table.ConnectorTableFunctionHandle;

import java.util.List;
import java.util.Objects;

/**
 * Transport objects shared by the Trino SPI adapter and the open-sdbl HTTP service.
 *
 * <p>The records in this class form the connector's serialized protocol. Changes to their
 * properties must remain compatible with both coordinator-to-worker handle serialization and
 * the Rust service's JSON models.
 */
public final class Model
{
    private Model() {}

    /** Describes a logical 1C column exposed by the metadata service. */
    @JsonIgnoreProperties(ignoreUnknown = true)
    public record RemoteColumn(String name, String type, boolean nullable, boolean predicatePushdown, String comment) {}

    /** Describes a logical 1C table and its columns. */
    @JsonIgnoreProperties(ignoreUnknown = true)
    public record RemoteTable(
            String schema,
            String name,
            String logicalName,
            String objectGuid,
            List<RemoteColumn> columns) {}

    /** A serialized inclusive or exclusive predicate bound. */
    public record Bound(String value, boolean inclusive) {}

    /** A serialized range whose {@code null} endpoints represent unbounded sides. */
    public record Range(Bound low, Bound high) {}

    /** A domain accepted for pushdown by the structured table scan endpoint. */
    public record Filter(String column, boolean all, boolean nullAllowed, List<Range> ranges) {}

    /** Request sent to the structured table scan endpoint. */
    public record ScanRequest(String schema, String table, List<String> columns, List<Filter> filters, Long limit) {}

    /** Request used to analyze the result shape of an SDBL table function. */
    public record SdblPrepareRequest(String query) {}

    /** A column in the dynamically analyzed SDBL result. */
    @JsonIgnoreProperties(ignoreUnknown = true)
    public record SdblColumn(int index, String name, String type, boolean nullable) {}

    /** Response containing the dynamically analyzed SDBL result shape. */
    @JsonIgnoreProperties(ignoreUnknown = true)
    public record SdblPrepareResponse(List<SdblColumn> columns) {}

    /** Request sent by a worker to execute an analyzed SDBL scan. */
    public record SdblScanRequest(String query, List<SdblColumn> expectedColumns, List<Integer> columns, Long limit) {}

    /** Coordinator-to-worker handle for a polymorphic SDBL table function. */
    public record SdblFunctionHandle(String query, List<SdblColumn> columns)
            implements ConnectorTableFunctionHandle
    {
        public SdblFunctionHandle
        {
            columns = List.copyOf(columns);
        }
    }

    /**
     * Immutable scan handle produced by connector planning.
     *
     * <p>A handle represents either a structured metadata table or an SDBL table-function result.
     * Derived handles carry accepted projection, predicate, and limit pushdowns to workers.
     */
    public static final class TableHandle
            implements ConnectorTableHandle
    {
        private final String schema;
        private final String table;
        private final List<Filter> filters;
        private final Long limit;
        private final List<String> projectedColumns;
        private final String sdblQuery;
        private final List<SdblColumn> sdblColumns;
        private final List<Integer> projectedSdblColumns;

        @JsonCreator
        public TableHandle(
                @JsonProperty("schema") String schema,
                @JsonProperty("table") String table,
                @JsonProperty("filters") List<Filter> filters,
                @JsonProperty("limit") Long limit,
                @JsonProperty("projectedColumns") List<String> projectedColumns,
                @JsonProperty("sdblQuery") String sdblQuery,
                @JsonProperty("sdblColumns") List<SdblColumn> sdblColumns,
                @JsonProperty("projectedSdblColumns") List<Integer> projectedSdblColumns)
        {
            this.schema = schema;
            this.table = table;
            this.filters = List.copyOf(filters == null ? List.of() : filters);
            this.limit = limit;
            this.projectedColumns = projectedColumns == null ? null : List.copyOf(projectedColumns);
            this.sdblQuery = sdblQuery;
            this.sdblColumns = sdblColumns == null ? null : List.copyOf(sdblColumns);
            this.projectedSdblColumns = projectedSdblColumns == null ? null : List.copyOf(projectedSdblColumns);
        }

        /** Creates a handle for a structured metadata table. */
        public static TableHandle table(String schema, String table)
        {
            return new TableHandle(schema, table, List.of(), null, null, null, null, null);
        }

        /** Creates a handle for an analyzed SDBL table function. */
        public static TableHandle sdbl(String query, List<SdblColumn> columns)
        {
            return new TableHandle("system", "sdbl", List.of(), null, null, query, columns, null);
        }

        @JsonProperty
        public String schema()
        {
            return schema;
        }

        @JsonProperty
        public String table()
        {
            return table;
        }

        @JsonProperty
        public List<Filter> filters()
        {
            return filters;
        }

        @JsonProperty
        public Long limit()
        {
            return limit;
        }

        @JsonProperty
        public List<String> projectedColumns()
        {
            return projectedColumns;
        }

        @JsonProperty
        public String sdblQuery()
        {
            return sdblQuery;
        }

        @JsonProperty
        public List<SdblColumn> sdblColumns()
        {
            return sdblColumns;
        }

        @JsonProperty
        public List<Integer> projectedSdblColumns()
        {
            return projectedSdblColumns;
        }

        public boolean isSdblQuery()
        {
            return sdblQuery != null;
        }

        TableHandle withLimit(long value)
        {
            return new TableHandle(
                    schema, table, filters, value, projectedColumns, sdblQuery, sdblColumns, projectedSdblColumns);
        }

        TableHandle withFilters(List<Filter> value)
        {
            return new TableHandle(
                    schema, table, value, limit, projectedColumns, sdblQuery, sdblColumns, projectedSdblColumns);
        }

        TableHandle withProjection(List<String> value)
        {
            return new TableHandle(schema, table, filters, limit, value, sdblQuery, sdblColumns, projectedSdblColumns);
        }

        TableHandle withSdblProjection(List<Integer> value)
        {
            return new TableHandle(schema, table, filters, limit, projectedColumns, sdblQuery, sdblColumns, value);
        }

        @Override
        public boolean equals(Object other)
        {
            return other instanceof TableHandle that &&
                    schema.equals(that.schema) &&
                    table.equals(that.table) &&
                    filters.equals(that.filters) &&
                    Objects.equals(limit, that.limit) &&
                    Objects.equals(projectedColumns, that.projectedColumns) &&
                    Objects.equals(sdblQuery, that.sdblQuery) &&
                    Objects.equals(sdblColumns, that.sdblColumns) &&
                    Objects.equals(projectedSdblColumns, that.projectedSdblColumns);
        }

        @Override
        public int hashCode()
        {
            return Objects.hash(
                    schema, table, filters, limit, projectedColumns, sdblQuery, sdblColumns, projectedSdblColumns);
        }

        @Override
        public String toString()
        {
            return schema + "." + table;
        }
    }

    /** Column handle used by structured metadata table scans. */
    public record Column(String name, String type, boolean predicatePushdown)
            implements ColumnHandle {}

    /** Ordinal column handle used by dynamically analyzed SDBL scans. */
    public record SdblColumnHandle(int index, String name, String type)
            implements ColumnHandle {}

    /** Single split used by the connector's current streaming scan implementation. */
    public enum Split
            implements ConnectorSplit
    {
        INSTANCE
    }

    /** Stateless transaction handle for this read-only connector. */
    public enum Transaction
            implements ConnectorTransactionHandle
    {
        INSTANCE
    }
}
