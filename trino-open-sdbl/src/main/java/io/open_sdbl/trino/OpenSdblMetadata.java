package io.open_sdbl.trino;

import io.airlift.slice.Slice;
import io.trino.spi.connector.Assignment;
import io.trino.spi.connector.ColumnHandle;
import io.trino.spi.connector.ColumnMetadata;
import io.trino.spi.connector.ConnectorMetadata;
import io.trino.spi.connector.ConnectorSession;
import io.trino.spi.connector.ConnectorTableHandle;
import io.trino.spi.connector.ConnectorTableMetadata;
import io.trino.spi.connector.ConnectorTableVersion;
import io.trino.spi.connector.Constraint;
import io.trino.spi.connector.ConstraintApplicationResult;
import io.trino.spi.connector.LimitApplicationResult;
import io.trino.spi.connector.ProjectionApplicationResult;
import io.trino.spi.connector.SchemaTableName;
import io.trino.spi.connector.TableFunctionApplicationResult;
import io.trino.spi.connector.TableNotFoundException;
import io.trino.spi.expression.ConnectorExpression;
import io.trino.spi.expression.Variable;
import io.trino.spi.function.table.ConnectorTableFunctionHandle;
import io.trino.spi.predicate.Domain;
import io.trino.spi.predicate.Range;
import io.trino.spi.predicate.TupleDomain;
import io.trino.spi.type.DecimalType;
import io.trino.spi.type.Type;
import io.trino.spi.type.TypeManager;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

import static io.trino.spi.type.UuidType.trinoUuidToJavaUuid;

/**
 * Adapts remote 1C metadata and accepted Trino pushdowns to connector handles.
 *
 * <p>Planning is deliberately side-effect free: every method returns a new immutable handle, and
 * workers later serialize that handle into a request for the Rust service.
 */
final class OpenSdblMetadata
        implements ConnectorMetadata
{
    private final OpenSdblClient client;
    private final TypeManager types;

    OpenSdblMetadata(OpenSdblClient client, TypeManager types)
    {
        this.client = client;
        this.types = types;
    }

    @Override
    public List<String> listSchemaNames(ConnectorSession session)
    {
        return client.trinoSchemas();
    }

    @Override
    public boolean schemaExists(ConnectorSession session, String schemaName)
    {
        return client.hasSchema(schemaName);
    }

    @Override
    public List<SchemaTableName> listTables(ConnectorSession session, Optional<String> schema)
    {
        return client.tables(schema.orElse(null)).stream()
                .map(table -> new SchemaTableName(table.schema(), table.name()))
                .toList();
    }

    @Override
    public Model.TableHandle getTableHandle(
            ConnectorSession session,
            SchemaTableName name,
            Optional<ConnectorTableVersion> startVersion,
            Optional<ConnectorTableVersion> endVersion)
    {
        if (startVersion.isPresent() || endVersion.isPresent()) {
            return null;
        }
        Model.RemoteTable remote = client.table(name.getSchemaName(), name.getTableName());
        return remote == null ? null : Model.TableHandle.table(remote.schema(), remote.name());
    }

    @Override
    public SchemaTableName getTableName(ConnectorSession session, ConnectorTableHandle handle)
    {
        Model.TableHandle table = (Model.TableHandle) handle;
        return new SchemaTableName(table.schema(), table.table());
    }

    @Override
    public ConnectorTableMetadata getTableMetadata(ConnectorSession session, ConnectorTableHandle handle)
    {
        Model.TableHandle table = (Model.TableHandle) handle;
        if (table.isSdblQuery()) {
            return new ConnectorTableMetadata(
                    new SchemaTableName("system", "sdbl"),
                    table.sdblColumns().stream()
                            .map(column -> ColumnMetadata.builder()
                                    .setName(column.name())
                                    .setType(type(column.type()))
                                    .setNullable(column.nullable())
                                    .build())
                            .toList());
        }
        Model.RemoteTable remote = requireTable(table);
        return new ConnectorTableMetadata(
                new SchemaTableName(remote.schema(), remote.name()),
                remote.columns().stream()
                        .map(this::metadata)
                        .toList());
    }

    @Override
    public Map<String, ColumnHandle> getColumnHandles(ConnectorSession session, ConnectorTableHandle handle)
    {
        Model.TableHandle table = (Model.TableHandle) handle;
        if (table.isSdblQuery()) {
            LinkedHashMap<String, ColumnHandle> result = new LinkedHashMap<>();
            for (Model.SdblColumn column : table.sdblColumns()) {
                result.put(column.name(), new Model.SdblColumnHandle(column.index(), column.name(), column.type()));
            }
            return Map.copyOf(result);
        }

        LinkedHashMap<String, ColumnHandle> result = new LinkedHashMap<>();
        for (Model.RemoteColumn column : requireTable(table).columns()) {
            result.put(column.name(), new Model.Column(column.name(), column.type(), column.predicatePushdown()));
        }
        return Map.copyOf(result);
    }

    @Override
    public ColumnMetadata getColumnMetadata(
            ConnectorSession session,
            ConnectorTableHandle table,
            ColumnHandle column)
    {
        if (column instanceof Model.SdblColumnHandle value) {
            return ColumnMetadata.builder()
                    .setName(value.name())
                    .setType(type(value.type()))
                    .setNullable(true)
                    .build();
        }
        Model.Column value = (Model.Column) column;
        return ColumnMetadata.builder()
                .setName(value.name())
                .setType(type(value.type()))
                .build();
    }

    /**
     * Pushes the smallest requested limit into the remote scan.
     *
     * <p>The service applies the limit in PostgreSQL and guarantees that at most the requested
     * number of rows is returned, so Trino does not need to retain a duplicate limit operator.
     */
    @Override
    public Optional<LimitApplicationResult<ConnectorTableHandle>> applyLimit(
            ConnectorSession session,
            ConnectorTableHandle handle,
            long limit)
    {
        Model.TableHandle table = (Model.TableHandle) handle;
        if (table.limit() != null && table.limit() <= limit) {
            return Optional.empty();
        }
        return Optional.of(new LimitApplicationResult<>(table.withLimit(limit), true, false));
    }

    /**
     * Converts supported structured-table domains into the connector's wire representation.
     *
     * <p>SDBL table-function predicates intentionally remain in Trino. Moving them inside a
     * slice or balance computation could change query semantics; a future implementation may
     * apply them to an outer PostgreSQL wrapper instead.
     */
    @Override
    public Optional<ConstraintApplicationResult<ConnectorTableHandle>> applyFilter(
            ConnectorSession session,
            ConnectorTableHandle handle,
            Constraint constraint)
    {
        Model.TableHandle table = (Model.TableHandle) handle;
        if (table.isSdblQuery()) {
            return Optional.empty();
        }

        Map<ColumnHandle, Domain> domains = constraint.getSummary().getDomains().orElse(Map.of());
        if (domains.isEmpty()) {
            return Optional.empty();
        }

        ArrayList<Model.Filter> pushed = new ArrayList<>(table.filters());
        LinkedHashMap<ColumnHandle, Domain> remaining = new LinkedHashMap<>();
        for (Map.Entry<ColumnHandle, Domain> entry : domains.entrySet()) {
            Model.Column column = (Model.Column) entry.getKey();
            Domain domain = entry.getValue();
            if (!column.predicatePushdown() || !domain.getType().isOrderable()) {
                remaining.put(column, domain);
                continue;
            }
            try {
                pushed.add(toFilter(column, domain));
            }
            catch (RuntimeException unsupported) {
                remaining.put(column, domain);
            }
        }

        if (pushed.size() == table.filters().size()) {
            return Optional.empty();
        }
        TupleDomain<ColumnHandle> remainder = remaining.isEmpty()
                ? TupleDomain.all()
                : TupleDomain.withColumnDomains(remaining);
        return Optional.of(new ConstraintApplicationResult<>(
                table.withFilters(pushed),
                remainder,
                constraint.getExpression(),
                false));
    }

    /** Pushes variable-only projections while leaving computed expressions in Trino. */
    @Override
    public Optional<ProjectionApplicationResult<ConnectorTableHandle>> applyProjection(
            ConnectorSession session,
            ConnectorTableHandle handle,
            List<ConnectorExpression> projections,
            Map<String, ColumnHandle> assignments)
    {
        if (projections.stream().anyMatch(expression -> !(expression instanceof Variable))) {
            return Optional.empty();
        }

        Model.TableHandle table = (Model.TableHandle) handle;
        if (table.isSdblQuery()) {
            List<Integer> indexes = projections.stream()
                    .map(Variable.class::cast)
                    .map(variable -> ((Model.SdblColumnHandle) assignments.get(variable.getName())).index())
                    .toList();
            if (indexes.equals(table.projectedSdblColumns())) {
                return Optional.empty();
            }
            List<Assignment> resultAssignments = projections.stream()
                    .map(Variable.class::cast)
                    .map(variable -> new Assignment(
                            variable.getName(),
                            assignments.get(variable.getName()),
                            variable.getType()))
                    .toList();
            return Optional.of(new ProjectionApplicationResult<>(
                    table.withSdblProjection(indexes),
                    projections,
                    resultAssignments,
                    false));
        }

        List<String> names = projections.stream()
                .map(Variable.class::cast)
                .map(variable -> ((Model.Column) assignments.get(variable.getName())).name())
                .toList();
        if (names.equals(table.projectedColumns())) {
            return Optional.empty();
        }
        List<Assignment> resultAssignments = projections.stream()
                .map(Variable.class::cast)
                .map(variable -> new Assignment(
                        variable.getName(),
                        assignments.get(variable.getName()),
                        variable.getType()))
                .toList();
        return Optional.of(new ProjectionApplicationResult<>(
                table.withProjection(names),
                projections,
                resultAssignments,
                false));
    }

    /** Converts an analyzed polymorphic SDBL invocation into a normal streaming table scan. */
    @Override
    public Optional<TableFunctionApplicationResult<ConnectorTableHandle>> applyTableFunction(
            ConnectorSession session,
            ConnectorTableFunctionHandle handle)
    {
        if (!(handle instanceof Model.SdblFunctionHandle function)) {
            return Optional.empty();
        }
        Model.TableHandle table = Model.TableHandle.sdbl(function.query(), function.columns());
        List<ColumnHandle> columns = function.columns().stream()
                .map(column -> (ColumnHandle) new Model.SdblColumnHandle(column.index(), column.name(), column.type()))
                .toList();
        return Optional.of(new TableFunctionApplicationResult<>(table, columns));
    }

    private Model.Filter toFilter(Model.Column column, Domain domain)
    {
        if (domain.getValues().isAll()) {
            return new Model.Filter(column.name(), true, domain.isNullAllowed(), List.of());
        }
        if (domain.getValues().isNone()) {
            return new Model.Filter(column.name(), false, domain.isNullAllowed(), List.of());
        }
        List<Model.Range> ranges = domain.getValues().getRanges().getOrderedRanges().stream()
                .map(range -> new Model.Range(bound(range, true), bound(range, false)))
                .toList();
        return new Model.Filter(column.name(), false, domain.isNullAllowed(), ranges);
    }

    private Model.Bound bound(Range range, boolean low)
    {
        if (low ? range.isLowUnbounded() : range.isHighUnbounded()) {
            return null;
        }
        Object value = low ? range.getLowBoundedValue() : range.getHighBoundedValue();
        boolean inclusive = low ? range.isLowInclusive() : range.isHighInclusive();
        return new Model.Bound(encodeValue(range.getType(), value), inclusive);
    }

    /** Encodes native Trino values in the lossless textual form expected by the Rust service. */
    private static String encodeValue(Type type, Object value)
    {
        String base = type.getTypeSignature().getBase();
        if (base.equals("uuid")) {
            return trinoUuidToJavaUuid((Slice) value).toString();
        }
        if (base.equals("varbinary")) {
            return Base64.getEncoder().encodeToString(((Slice) value).getBytes());
        }
        if (value instanceof Slice slice) {
            return slice.toStringUtf8();
        }
        if (type instanceof DecimalType decimal) {
            BigInteger unscaled = value instanceof Long number
                    ? BigInteger.valueOf(number)
                    : new BigInteger(value.toString());
            return new BigDecimal(unscaled, decimal.getScale()).toPlainString();
        }
        if (base.equals("date")) {
            return LocalDate.ofEpochDay((Long) value).toString();
        }
        if (base.equals("timestamp")) {
            long micros = (Long) value;
            return LocalDateTime.ofEpochSecond(
                            Math.floorDiv(micros, 1_000_000),
                            Math.floorMod(micros, 1_000_000) * 1_000,
                            ZoneOffset.UTC)
                    .toString();
        }
        return value.toString();
    }

    private Model.RemoteTable requireTable(Model.TableHandle table)
    {
        Model.RemoteTable remote = client.table(table.schema(), table.table());
        if (remote == null) {
            throw new TableNotFoundException(new SchemaTableName(table.schema(), table.table()));
        }
        return remote;
    }

    private ColumnMetadata metadata(Model.RemoteColumn column)
    {
        return ColumnMetadata.builder()
                .setName(column.name())
                .setType(type(column.type()))
                .setNullable(column.nullable())
                .setComment(Optional.ofNullable(column.comment()))
                .build();
    }

    private Type type(String signature)
    {
        return Types.resolve(types, signature);
    }
}
