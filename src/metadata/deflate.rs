use super::MetadataError;

/// Default maximum decoded size for one platform metadata resource (256 MiB).
pub const DEFAULT_OUTPUT_LIMIT: usize = 256 * 1024 * 1024;

const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Inflates an RFC 1951 raw-DEFLATE stream using [`DEFAULT_OUTPUT_LIMIT`].
///
/// # Errors
///
/// Returns [`MetadataError`] when the bit stream or Huffman trees are invalid,
/// a back-reference is out of range, or the decoded output exceeds the limit.
pub fn inflate_raw_deflate(input: &[u8]) -> Result<Vec<u8>, MetadataError> {
    inflate_raw_deflate_bounded(input, DEFAULT_OUTPUT_LIMIT)
}

/// Inflates an RFC 1951 raw-DEFLATE stream with an explicit decoded-size limit.
///
/// # Errors
///
/// Returns [`MetadataError`] for invalid data or when `output_limit` is
/// exceeded.
pub fn inflate_raw_deflate_bounded(
    input: &[u8],
    output_limit: usize,
) -> Result<Vec<u8>, MetadataError> {
    if input.is_empty() {
        return Err(MetadataError::deflate(0, "empty raw-DEFLATE resource"));
    }
    let mut bits = BitReader::new(input);
    let mut output = Vec::new();
    let mut huffman_table = Vec::new();

    loop {
        let final_block = bits.read_bits(1)? != 0;
        match bits.read_bits(2)? {
            0 => stored_block(&mut bits, &mut output, output_limit)?,
            1 => {
                let (literal_lengths, distance_lengths) = fixed_lengths();
                compressed_block(
                    &mut bits,
                    &mut output,
                    &literal_lengths,
                    &distance_lengths,
                    &mut huffman_table,
                    output_limit,
                )?;
            }
            2 => {
                let (literal_lengths, distance_lengths) =
                    dynamic_lengths(&mut bits, &mut huffman_table)?;
                compressed_block(
                    &mut bits,
                    &mut output,
                    &literal_lengths,
                    &distance_lengths,
                    &mut huffman_table,
                    output_limit,
                )?;
            }
            _ => return Err(bits.error("reserved DEFLATE block type")),
        }
        if final_block {
            return Ok(output);
        }
    }
}

fn stored_block(
    bits: &mut BitReader<'_>,
    output: &mut Vec<u8>,
    limit: usize,
) -> Result<(), MetadataError> {
    bits.align_byte();
    let length = usize::from(bits.read_u16()?);
    let complement = bits.read_u16()?;
    if (length as u16) != !complement {
        return Err(bits.error("invalid stored-block length complement"));
    }
    reserve_output(output, length, limit, bits.position())?;
    let bytes = bits.read_bytes(length)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn fixed_lengths() -> (Vec<u8>, Vec<u8>) {
    let mut literal = vec![0; 288];
    literal[..=143].fill(8);
    literal[144..=255].fill(9);
    literal[256..=279].fill(7);
    literal[280..=287].fill(8);
    (literal, vec![5; 32])
}

fn dynamic_lengths(
    bits: &mut BitReader<'_>,
    huffman_table: &mut Vec<HuffmanEntry>,
) -> Result<(Vec<u8>, Vec<u8>), MetadataError> {
    let literal_count = bits.read_bits(5)? as usize + 257;
    let distance_count = bits.read_bits(5)? as usize + 1;
    let code_count = bits.read_bits(4)? as usize + 4;
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut code_lengths = vec![0; 19];
    for index in 0..code_count {
        code_lengths[ORDER[index]] = bits.read_bits(3)? as u8;
    }
    huffman_table.clear();
    let code_tree = Huffman::build(&code_lengths, bits.position(), huffman_table, false)?
        .expect("a required Huffman tree cannot be empty");
    let total = literal_count + distance_count;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        let symbol = code_tree.decode(huffman_table, bits)?;
        append_dynamic_length(symbol, bits, &mut lengths, total)?;
    }
    if lengths.get(256).copied().unwrap_or_default() == 0 {
        return Err(bits.error("literal Huffman tree has no end-of-block symbol"));
    }
    let distance = lengths.split_off(literal_count);
    Ok((lengths, distance))
}

fn append_dynamic_length(
    symbol: u16,
    bits: &mut BitReader<'_>,
    lengths: &mut Vec<u8>,
    total: usize,
) -> Result<(), MetadataError> {
    match symbol {
        symbol @ 0..=15 => lengths.push(symbol as u8),
        16 => {
            let Some(previous) = lengths.last().copied() else {
                return Err(bits.error("repeat code has no previous Huffman length"));
            };
            let repeat = bits.read_bits(2)? as usize + 3;
            append_repeated(lengths, previous, repeat, total, bits.position())?;
        }
        17 => {
            let repeat = bits.read_bits(3)? as usize + 3;
            append_repeated(lengths, 0, repeat, total, bits.position())?;
        }
        18 => {
            let repeat = bits.read_bits(7)? as usize + 11;
            append_repeated(lengths, 0, repeat, total, bits.position())?;
        }
        _ => return Err(bits.error("invalid code-length symbol")),
    }
    Ok(())
}

fn append_repeated(
    lengths: &mut Vec<u8>,
    value: u8,
    repeat: usize,
    total: usize,
    position: usize,
) -> Result<(), MetadataError> {
    if lengths.len().saturating_add(repeat) > total {
        return Err(MetadataError::deflate(
            position,
            "Huffman length repeat exceeds declared tree size",
        ));
    }
    lengths.resize(lengths.len() + repeat, value);
    Ok(())
}

fn compressed_block(
    bits: &mut BitReader<'_>,
    output: &mut Vec<u8>,
    literal_lengths: &[u8],
    distance_lengths: &[u8],
    huffman_table: &mut Vec<HuffmanEntry>,
    limit: usize,
) -> Result<(), MetadataError> {
    huffman_table.clear();
    let literal_tree = Huffman::build(literal_lengths, bits.position(), huffman_table, false)?
        .expect("a required Huffman tree cannot be empty");
    let distance_tree = Huffman::build(
        distance_lengths,
        bits.position(),
        huffman_table,
        distance_lengths.len() == 1,
    )?;
    loop {
        let symbol = literal_tree.decode(huffman_table, bits)?;
        match symbol {
            0..=255 => {
                reserve_output(output, 1, limit, bits.position())?;
                output.push(symbol as u8);
            }
            256 => return Ok(()),
            257..=285 => {
                let length_index = symbol as usize - 257;
                let length = LENGTH_BASE[length_index]
                    + bits.read_bits(LENGTH_EXTRA[length_index])? as usize;
                let distance_tree = distance_tree
                    .ok_or_else(|| bits.error("length symbol requires a distance Huffman tree"))?;
                let distance_symbol = distance_tree.decode(huffman_table, bits)? as usize;
                if distance_symbol >= DISTANCE_BASE.len() {
                    return Err(bits.error("invalid DEFLATE distance symbol"));
                }
                let distance = DISTANCE_BASE[distance_symbol]
                    + bits.read_bits(DISTANCE_EXTRA[distance_symbol])? as usize;
                if distance == 0 || distance > output.len() {
                    return Err(bits.error("DEFLATE back-reference is out of range"));
                }
                reserve_output(output, length, limit, bits.position())?;
                copy_back_reference(output, distance, length);
            }
            _ => return Err(bits.error("invalid DEFLATE literal/length symbol")),
        }
    }
}

fn reserve_output(
    output: &mut Vec<u8>,
    additional: usize,
    limit: usize,
    position: usize,
) -> Result<(), MetadataError> {
    if additional > limit.saturating_sub(output.len()) {
        return Err(MetadataError::deflate(
            position,
            format!("decoded metadata exceeds {limit} byte limit"),
        ));
    }
    output.try_reserve(additional).map_err(|_| {
        MetadataError::deflate(
            position,
            format!("decoded metadata exceeds {limit} byte limit"),
        )
    })?;
    Ok(())
}

fn copy_back_reference(output: &mut Vec<u8>, distance: usize, length: usize) {
    let start = output.len() - distance;
    let mut remaining = length;
    while remaining != 0 {
        let available = output.len() - start;
        let copied = remaining.min(available);
        output.extend_from_within(start..start + copied);
        remaining -= copied;
    }
}

#[derive(Debug, Clone, Copy)]
struct Huffman {
    start: usize,
    table_length: usize,
    maximum_length: u8,
}

#[derive(Clone, Copy, Default)]
struct HuffmanEntry {
    length: u8,
    symbol: u16,
}

impl Huffman {
    fn build(
        lengths: &[u8],
        position: usize,
        table: &mut Vec<HuffmanEntry>,
        allow_empty: bool,
    ) -> Result<Option<Self>, MetadataError> {
        let mut counts = [0u16; 16];
        for &length in lengths {
            if length > 15 {
                return Err(MetadataError::deflate(
                    position,
                    "Huffman code is longer than 15 bits",
                ));
            }
            if length != 0 {
                counts[usize::from(length)] += 1;
            }
        }
        if counts[1..].iter().all(|count| *count == 0) {
            return if allow_empty {
                Ok(None)
            } else {
                Err(MetadataError::deflate(position, "empty Huffman tree"))
            };
        }

        let mut left = 1i32;
        for &count in &counts[1..] {
            left = left * 2 - i32::from(count);
            if left < 0 {
                return Err(MetadataError::deflate(
                    position,
                    "oversubscribed Huffman tree",
                ));
            }
        }

        let mut next_code = [0u16; 16];
        let mut code = 0u16;
        for bits in 1..=15 {
            code = (code + counts[bits - 1]) << 1;
            next_code[bits] = code;
        }

        let maximum_length = lengths.iter().copied().max().unwrap_or_default();
        let table_length = 1usize << maximum_length;
        let start = table.len();
        table.resize(start + table_length, HuffmanEntry::default());
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let canonical = next_code[usize::from(length)];
            next_code[usize::from(length)] += 1;
            let reversed_code = reverse_bits(canonical, length);
            let entry = HuffmanEntry {
                length,
                symbol: symbol as u16,
            };
            let step = 1usize << entry.length;
            for index in (usize::from(reversed_code)..table_length).step_by(step) {
                debug_assert_eq!(table[start + index].length, 0);
                table[start + index] = entry;
            }
        }
        Ok(Some(Self {
            start,
            table_length,
            maximum_length,
        }))
    }

    fn decode(
        &self,
        table: &[HuffmanEntry],
        bits: &mut BitReader<'_>,
    ) -> Result<u16, MetadataError> {
        let index = bits.peek_bits_padded(self.maximum_length) as usize;
        debug_assert!(index < self.table_length);
        let entry = table[self.start + index];
        if entry.length == 0 {
            return Err(bits.error("invalid Huffman code"));
        }
        bits.advance(entry.length)?;
        Ok(entry.symbol)
    }
}

fn reverse_bits(mut code: u16, length: u8) -> u16 {
    let mut reversed = 0;
    for _ in 0..length {
        reversed = (reversed << 1) | (code & 1);
        code >>= 1;
    }
    reversed
}

struct BitReader<'input> {
    input: &'input [u8],
    bit: usize,
}

impl<'input> BitReader<'input> {
    const fn new(input: &'input [u8]) -> Self {
        Self { input, bit: 0 }
    }

    fn read_bits(&mut self, count: u8) -> Result<u32, MetadataError> {
        if usize::from(count) > self.remaining_bits() {
            return Err(self.error("truncated raw-DEFLATE stream"));
        }
        let value = self.peek_bits_padded(count);
        self.bit += usize::from(count);
        Ok(value)
    }

    fn peek_bits_padded(&self, count: u8) -> u32 {
        if count == 0 {
            return 0;
        }
        debug_assert!(count <= 24);
        let byte_index = self.bit / 8;
        let mut window = 0u32;
        if let Some(remaining) = self.input.get(byte_index..) {
            for (index, &byte) in remaining.iter().take(3).enumerate() {
                window |= u32::from(byte) << (index * 8);
            }
        }
        let mask = (1u32 << count) - 1;
        (window >> (self.bit % 8)) & mask
    }

    fn advance(&mut self, count: u8) -> Result<(), MetadataError> {
        if usize::from(count) > self.remaining_bits() {
            return Err(self.error("truncated raw-DEFLATE stream"));
        }
        self.bit += usize::from(count);
        Ok(())
    }

    fn remaining_bits(&self) -> usize {
        self.input.len().saturating_mul(8).saturating_sub(self.bit)
    }

    fn align_byte(&mut self) {
        self.bit = self.bit.div_ceil(8) * 8;
    }

    fn read_u16(&mut self) -> Result<u16, MetadataError> {
        let low = self.read_byte()?;
        let high = self.read_byte()?;
        Ok(u16::from_le_bytes([low, high]))
    }

    fn read_byte(&mut self) -> Result<u8, MetadataError> {
        if self.bit % 8 != 0 {
            return Err(self.error("unaligned DEFLATE byte read"));
        }
        let Some(byte) = self.input.get(self.bit / 8).copied() else {
            return Err(self.error("truncated raw-DEFLATE stream"));
        };
        self.bit += 8;
        Ok(byte)
    }

    fn read_bytes(&mut self, count: usize) -> Result<&'input [u8], MetadataError> {
        if self.bit % 8 != 0 {
            return Err(self.error("unaligned DEFLATE byte read"));
        }
        let start = self.bit / 8;
        let end = start
            .checked_add(count)
            .ok_or_else(|| self.error("truncated raw-DEFLATE stream"))?;
        let bytes = self
            .input
            .get(start..end)
            .ok_or_else(|| self.error("truncated raw-DEFLATE stream"))?;
        self.bit += count * 8;
        Ok(bytes)
    }

    const fn position(&self) -> usize {
        self.bit
    }

    fn error(&self, message: impl Into<String>) -> MetadataError {
        MetadataError::deflate(self.bit, message)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BitReader, Huffman, MetadataError, append_dynamic_length, append_repeated,
        compressed_block, copy_back_reference, fixed_lengths, inflate_raw_deflate,
        inflate_raw_deflate_bounded, reserve_output,
    };
    use crate::hex_test_support::hex;
    use crate::metadata::MetadataErrorKind;

    #[derive(Default)]
    struct TestBits {
        bytes: Vec<u8>,
        bit: usize,
    }

    impl TestBits {
        fn push(&mut self, value: u32, count: u8) {
            for offset in 0..count {
                if self.bit / 8 == self.bytes.len() {
                    self.bytes.push(0);
                }
                if value & (1 << offset) != 0 {
                    self.bytes[self.bit / 8] |= 1 << (self.bit % 8);
                }
                self.bit += 1;
            }
        }

        fn finish(self) -> Vec<u8> {
            self.bytes
        }
    }

    #[test]
    fn inflates_stored_fixed_and_dynamic_blocks() {
        let stored = hex("011100eeff73746f72656420626c6f636b2064617461");
        assert_eq!(inflate_raw_deflate(&stored).unwrap(), b"stored block data");

        let fixed = hex("4bcbac484d51c8284d4bcb4dcc53c84d2d494c492c49843300");
        assert_eq!(
            inflate_raw_deflate(&fixed).unwrap(),
            b"fixed huffman metadata metadata"
        );

        let dynamic = hex(
            "edc94b1a40201846e1b57e944b89f02bacde268c7ace99bc83231151eb75bdf3c338cd212e69ddf27e9c76957a3f2fa3d991bcc9c98488888888fff801",
        );
        let mut expected = vec![b'a'; 1000];
        expected.extend_from_slice(b"bcdefghijklmnopqrstuvwxyz".repeat(20).as_slice());
        expected.extend_from_slice(b"metadata".repeat(300).as_slice());
        assert_eq!(inflate_raw_deflate(&dynamic).unwrap(), expected);
    }

    #[test]
    fn inflates_a_literal_only_dynamic_block_with_an_empty_distance_tree() {
        let mut bits = TestBits::default();
        bits.push(1, 1); // BFINAL
        bits.push(2, 2); // dynamic Huffman block
        bits.push(0, 5); // 257 literal/length codes
        bits.push(0, 5); // one distance code
        bits.push(14, 4); // 18 code-length codes

        // Code-length alphabet: symbol 18 has length 1; symbols 0 and 1
        // have length 2. The remaining declared symbols are absent.
        for length in [0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2] {
            bits.push(length, 3);
        }

        // Literal lengths: 65 zeros, `A` with length 1, 190 zeros,
        // end-of-block with length 1. The sole distance length is zero.
        bits.push(0, 1); // repeat zero (18)
        bits.push(54, 7); // 65 zeros
        bits.push(3, 2); // code length 1
        bits.push(0, 1);
        bits.push(127, 7); // 138 zeros
        bits.push(0, 1);
        bits.push(41, 7); // 52 zeros
        bits.push(3, 2); // code length 1
        bits.push(1, 2); // final zero distance length

        bits.push(0, 1); // literal `A`
        bits.push(1, 1); // end of block
        assert_eq!(inflate_raw_deflate(&bits.finish()).unwrap(), b"A");
    }

    #[test]
    fn rejects_truncated_invalid_and_oversized_data() {
        assert!(inflate_raw_deflate(&[0x03]).is_err());
        let stored = hex("011100eeff73746f72656420626c6f636b2064617461");
        assert!(inflate_raw_deflate_bounded(&stored, 16).is_err());
    }

    fn dynamic_header(code_lengths: [u8; 4]) -> TestBits {
        let mut bits = TestBits::default();
        bits.push(1, 1);
        bits.push(2, 2);
        bits.push(0, 5);
        bits.push(0, 5);
        bits.push(0, 4);
        for length in code_lengths {
            bits.push(u32::from(length), 3);
        }
        bits
    }

    fn compressed_error(
        literal_symbol: usize,
        distance_symbol: Option<usize>,
        output: &mut Vec<u8>,
        limit: usize,
    ) -> MetadataError {
        let mut literal_lengths = vec![0; literal_symbol + 1];
        literal_lengths[literal_symbol] = 1;
        let mut distance_lengths = vec![0; distance_symbol.map_or(1, |symbol| symbol + 1)];
        if let Some(symbol) = distance_symbol {
            distance_lengths[symbol] = 1;
        }
        compressed_block(
            &mut BitReader::new(&[0]),
            output,
            &literal_lengths,
            &distance_lengths,
            &mut Vec::new(),
            limit,
        )
        .unwrap_err()
    }

    fn assert_deflate_error(error: &MetadataError, message: &str, offset: usize) {
        assert_eq!(error.kind(), MetadataErrorKind::Deflate);
        assert!(error.message().contains(message), "{error}");
        assert_eq!(error.offset(), Some(offset), "wrong bit offset for {error}");
    }

    #[test]
    fn covers_all_deflate_diagnostics_with_meaningful_offsets() {
        let mut errors = vec![
            (
                inflate_raw_deflate_bounded(&[], 8).unwrap_err(),
                "empty raw-DEFLATE resource",
                0,
            ),
            (
                inflate_raw_deflate_bounded(&[0], 8).unwrap_err(),
                "truncated raw-DEFLATE stream",
                8,
            ),
            (
                inflate_raw_deflate_bounded(&[7], 8).unwrap_err(),
                "reserved DEFLATE block type",
                3,
            ),
            (
                inflate_raw_deflate_bounded(&[1, 0, 0, 0, 0], 8).unwrap_err(),
                "invalid stored-block length complement",
                40,
            ),
        ];

        let mut no_previous = dynamic_header([1, 0, 0, 0]);
        no_previous.push(0, 1);
        errors.push((
            inflate_raw_deflate_bounded(&no_previous.finish(), 8).unwrap_err(),
            "repeat code has no previous Huffman length",
            30,
        ));

        let mut no_end = dynamic_header([0, 0, 0, 1]);
        for _ in 0..258 {
            no_end.push(0, 1);
        }
        errors.push((
            inflate_raw_deflate_bounded(&no_end.finish(), 8).unwrap_err(),
            "literal Huffman tree has no end-of-block symbol",
            287,
        ));

        let empty_code_tree = dynamic_header([0, 0, 0, 0]).finish();
        errors.push((
            inflate_raw_deflate_bounded(&empty_code_tree, 8).unwrap_err(),
            "empty Huffman tree",
            29,
        ));
        let oversubscribed_code_tree = dynamic_header([1, 1, 1, 0]).finish();
        errors.push((
            inflate_raw_deflate_bounded(&oversubscribed_code_tree, 8).unwrap_err(),
            "oversubscribed Huffman tree",
            29,
        ));

        errors.push((
            append_repeated(&mut vec![1], 1, 3, 3, 41).unwrap_err(),
            "Huffman length repeat exceeds declared tree size",
            41,
        ));
        errors.push((
            append_dynamic_length(19, &mut BitReader::new(&[0]), &mut Vec::new(), 1).unwrap_err(),
            "invalid code-length symbol",
            0,
        ));
        errors.push((
            compressed_error(257, None, &mut Vec::new(), 8),
            "length symbol requires a distance Huffman tree",
            1,
        ));
        errors.push((
            compressed_error(257, Some(30), &mut Vec::new(), 8),
            "invalid DEFLATE distance symbol",
            2,
        ));
        errors.push((
            compressed_error(257, Some(0), &mut Vec::new(), 8),
            "DEFLATE back-reference is out of range",
            2,
        ));
        errors.push((
            compressed_error(286, Some(0), &mut Vec::new(), 8),
            "invalid DEFLATE literal/length symbol",
            1,
        ));
        errors.push((
            compressed_error(65, Some(0), &mut Vec::new(), 0),
            "decoded metadata exceeds 0 byte limit",
            1,
        ));

        let mut table = Vec::new();
        errors.push((
            Huffman::build(&[16], 43, &mut table, false).unwrap_err(),
            "Huffman code is longer than 15 bits",
            43,
        ));
        table.clear();
        let incomplete = Huffman::build(&[1], 0, &mut table, false).unwrap().unwrap();
        errors.push((
            incomplete
                .decode(&table, &mut BitReader::new(&[1]))
                .unwrap_err(),
            "invalid Huffman code",
            0,
        ));
        errors.push((
            BitReader {
                input: &[0],
                bit: 1,
            }
            .read_byte()
            .unwrap_err(),
            "unaligned DEFLATE byte read",
            1,
        ));

        for (index, (error, message, offset)) in errors.iter().enumerate() {
            assert_deflate_error(error, message, *offset);
            assert!(
                errors[..index]
                    .iter()
                    .all(|(_, previous, _)| previous != message),
                "duplicate diagnostic in coverage table: {message}"
            );
        }
        assert_eq!(errors.len(), 18);
    }

    #[test]
    fn rejects_multiple_zero_length_distance_codes() {
        let mut literal_lengths = vec![0; 257];
        literal_lengths[256] = 1;
        let error = compressed_block(
            &mut BitReader::new(&[0]),
            &mut Vec::new(),
            &literal_lengths,
            &[0, 0],
            &mut Vec::new(),
            8,
        )
        .unwrap_err();
        assert_deflate_error(&error, "empty Huffman tree", 0);
    }

    #[test]
    fn reuses_huffman_storage_and_copies_back_references_by_slices() {
        let (literal_lengths, distance_lengths) = fixed_lengths();
        let mut table = Vec::new();
        Huffman::build(&literal_lengths, 0, &mut table, false).unwrap();
        Huffman::build(&distance_lengths, 0, &mut table, false).unwrap();
        let pointer = table.as_ptr();
        let capacity = table.capacity();
        table.clear();
        Huffman::build(&literal_lengths, 0, &mut table, false).unwrap();
        Huffman::build(&distance_lengths, 0, &mut table, false).unwrap();
        assert_eq!(table.as_ptr(), pointer);
        assert_eq!(table.capacity(), capacity);

        let mut non_overlapping = b"abcdef".to_vec();
        copy_back_reference(&mut non_overlapping, 6, 3);
        assert_eq!(non_overlapping, b"abcdefabc");
        let mut overlapping = b"ab".to_vec();
        copy_back_reference(&mut overlapping, 2, 7);
        assert_eq!(overlapping, b"ababababa");

        let mut bounded = Vec::with_capacity(1);
        bounded.push(1);
        let old_capacity = bounded.capacity();
        let error = reserve_output(&mut bounded, 1, 1, 17).unwrap_err();
        assert_eq!(bounded.capacity(), old_capacity);
        assert_eq!(error.offset(), Some(17));
    }

    #[test]
    fn decodes_a_short_final_code_and_rejects_an_incomplete_tree_hole() {
        let mut table = Vec::new();
        let tree = Huffman::build(&[1, 15], 0, &mut table, false)
            .unwrap()
            .unwrap();
        let mut final_bit = BitReader {
            input: &[0],
            bit: 7,
        };
        assert_eq!(tree.decode(&table, &mut final_bit).unwrap(), 0);
        assert_eq!(final_bit.position(), 8);

        table.clear();
        let incomplete = Huffman::build(&[1], 0, &mut table, false).unwrap().unwrap();
        let mut invalid = BitReader::new(&[1]);
        assert!(
            incomplete
                .decode(&table, &mut invalid)
                .unwrap_err()
                .to_string()
                .contains("invalid Huffman code")
        );
    }
}
