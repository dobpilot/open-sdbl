package io.open_sdbl.trino;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ModelTest
{
    @Test
    void serializesTheRustScanContract() throws Exception
    {
        Model.Filter filter = new Model.Filter(
                "ИНН",
                false,
                false,
                List.of(new Model.Range(
                        new Model.Bound("7701234567", true),
                        new Model.Bound("7701234567", true))));
        Model.ScanRequest request = new Model.ScanRequest(
                "Справочник",
                "Контрагенты",
                List.of("ИНН"),
                List.of(filter),
                10L);
        String json = new ObjectMapper().writeValueAsString(request);
        assertTrue(json.contains("\"nullAllowed\":false"));
        assertTrue(json.contains("\"limit\":10"));
    }

    @Test
    void keepsTheSmallestAppliedLimit()
    {
        Model.TableHandle original = Model.TableHandle.table("Справочник", "Контрагенты");
        assertEquals(10L, original.withLimit(10).limit());
        assertEquals(List.of("ИНН"), original.withProjection(List.of("ИНН")).projectedColumns());
    }

    @Test
    void tableHandleIsSerializableAcrossTrinoNodes() throws Exception
    {
        Model.TableHandle handle = Model.TableHandle.table("Справочник", "Контрагенты")
                .withLimit(10)
                .withProjection(List.of("ИНН"));
        String json = new ObjectMapper().writeValueAsString(handle);
        assertTrue(json.contains("\"schema\":\"Справочник\""));
        assertTrue(json.contains("\"projectedColumns\":[\"ИНН\"]"));
    }

    @Test
    void sdblHandleCarriesOnlySourceAndAnalyzedShape() throws Exception
    {
        Model.SdblColumn column = new Model.SdblColumn(0, "Представление", "varchar", true);
        Model.TableHandle handle = Model.TableHandle.sdbl(
                "SELECT RefPresentation(Item) FROM Catalog.Items",
                List.of(column));
        String json = new ObjectMapper().writeValueAsString(handle);
        assertTrue(json.contains("\"sdblQuery\""));
        assertTrue(json.contains("\"sdblColumns\""));
        assertEquals(List.of(0), handle.withSdblProjection(List.of(0)).projectedSdblColumns());
    }
}
