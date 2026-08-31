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
        return Err(MetadataError::new("empty raw-DEFLATE resource"));
    }
    let mut bits = BitReader::new(input);
    let mut output = Vec::new();

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
                    output_limit,
                )?;
            }
            2 => {
                let (literal_lengths, distance_lengths) = dynamic_lengths(&mut bits)?;
                compressed_block(
                    &mut bits,
                    &mut output,
                    &literal_lengths,
                    &distance_lengths,
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
    ensure_capacity(output.len(), length, limit, bits.position())?;
    for _ in 0..length {
        output.push(bits.read_byte()?);
    }
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

fn dynamic_lengths(bits: &mut BitReader<'_>) -> Result<(Vec<u8>, Vec<u8>), MetadataError> {
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
    let code_tree = Huffman::new(&code_lengths, bits.position())?;
    let total = literal_count + distance_count;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        match code_tree.decode(bits)? {
            symbol @ 0..=15 => lengths.push(symbol as u8),
            16 => {
                let Some(previous) = lengths.last().copied() else {
                    return Err(bits.error("repeat code has no previous Huffman length"));
                };
                let repeat = bits.read_bits(2)? as usize + 3;
                append_repeated(&mut lengths, previous, repeat, total, bits.position())?;
            }
            17 => {
                let repeat = bits.read_bits(3)? as usize + 3;
                append_repeated(&mut lengths, 0, repeat, total, bits.position())?;
            }
            18 => {
                let repeat = bits.read_bits(7)? as usize + 11;
                append_repeated(&mut lengths, 0, repeat, total, bits.position())?;
            }
            _ => return Err(bits.error("invalid code-length symbol")),
        }
    }
    if lengths.get(256).copied().unwrap_or_default() == 0 {
        return Err(bits.error("literal Huffman tree has no end-of-block symbol"));
    }
    let distance = lengths.split_off(literal_count);
    Ok((lengths, distance))
}

fn append_repeated(
    lengths: &mut Vec<u8>,
    value: u8,
    repeat: usize,
    total: usize,
    position: usize,
) -> Result<(), MetadataError> {
    if lengths.len().saturating_add(repeat) > total {
        return Err(MetadataError::at(
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
    limit: usize,
) -> Result<(), MetadataError> {
    let literal_tree = Huffman::new(literal_lengths, bits.position())?;
    let distance_tree = Huffman::new(distance_lengths, bits.position())?;
    loop {
        let symbol = literal_tree.decode(bits)?;
        match symbol {
            0..=255 => {
                ensure_capacity(output.len(), 1, limit, bits.position())?;
                output.push(symbol as u8);
            }
            256 => return Ok(()),
            257..=285 => {
                let length_index = symbol as usize - 257;
                let length = LENGTH_BASE[length_index]
                    + bits.read_bits(LENGTH_EXTRA[length_index])? as usize;
                let distance_symbol = distance_tree.decode(bits)? as usize;
                if distance_symbol >= DISTANCE_BASE.len() {
                    return Err(bits.error("invalid DEFLATE distance symbol"));
                }
                let distance = DISTANCE_BASE[distance_symbol]
                    + bits.read_bits(DISTANCE_EXTRA[distance_symbol])? as usize;
                if distance == 0 || distance > output.len() {
                    return Err(bits.error("DEFLATE back-reference is out of range"));
                }
                ensure_capacity(output.len(), length, limit, bits.position())?;
                for _ in 0..length {
                    let byte = output[output.len() - distance];
                    output.push(byte);
                }
            }
            _ => return Err(bits.error("invalid DEFLATE literal/length symbol")),
        }
    }
}

fn ensure_capacity(
    current: usize,
    additional: usize,
    limit: usize,
    position: usize,
) -> Result<(), MetadataError> {
    if additional > limit.saturating_sub(current) {
        return Err(MetadataError::at(
            position,
            format!("decoded metadata exceeds {limit} byte limit"),
        ));
    }
    Ok(())
}

struct Huffman {
    table: Vec<HuffmanEntry>,
    maximum_length: u8,
}

#[derive(Clone, Copy, Default)]
struct HuffmanEntry {
    length: u8,
    symbol: u16,
}

impl Huffman {
    fn new(lengths: &[u8], position: usize) -> Result<Self, MetadataError> {
        let mut counts = [0u16; 16];
        for &length in lengths {
            if length > 15 {
                return Err(MetadataError::at(
                    position,
                    "Huffman code is longer than 15 bits",
                ));
            }
            if length != 0 {
                counts[usize::from(length)] += 1;
            }
        }
        if counts[1..].iter().all(|count| *count == 0) {
            return Err(MetadataError::at(position, "empty Huffman tree"));
        }

        let mut left = 1i32;
        for &count in &counts[1..] {
            left = left * 2 - i32::from(count);
            if left < 0 {
                return Err(MetadataError::at(position, "oversubscribed Huffman tree"));
            }
        }

        let mut next_code = [0u16; 16];
        let mut code = 0u16;
        for bits in 1..=15 {
            code = (code + counts[bits - 1]) << 1;
            next_code[bits] = code;
        }

        let mut codes = Vec::new();
        let mut maximum_length = 0;
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let canonical = next_code[usize::from(length)];
            next_code[usize::from(length)] += 1;
            codes.push((
                reverse_bits(canonical, length),
                HuffmanEntry {
                    length,
                    symbol: symbol as u16,
                },
            ));
            maximum_length = maximum_length.max(length);
        }
        let mut table = vec![HuffmanEntry::default(); 1usize << maximum_length];
        for (reversed_code, entry) in codes {
            let step = 1usize << entry.length;
            for index in (usize::from(reversed_code)..table.len()).step_by(step) {
                debug_assert_eq!(table[index].length, 0);
                table[index] = entry;
            }
        }
        Ok(Self {
            table,
            maximum_length,
        })
    }

    fn decode(&self, bits: &mut BitReader<'_>) -> Result<u16, MetadataError> {
        let index = bits.peek_bits_padded(self.maximum_length) as usize;
        let entry = self.table[index];
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

    const fn position(&self) -> usize {
        self.bit
    }

    fn error(&self, message: impl Into<String>) -> MetadataError {
        MetadataError::at(self.bit, message)
    }
}

#[cfg(test)]
mod tests {
    use super::{BitReader, Huffman, inflate_raw_deflate, inflate_raw_deflate_bounded};

    fn hex(input: &str) -> Vec<u8> {
        input
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let text = std::str::from_utf8(pair).unwrap();
                u8::from_str_radix(text, 16).unwrap()
            })
            .collect()
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
    fn rejects_truncated_invalid_and_oversized_data() {
        assert!(inflate_raw_deflate(&[0x03]).is_err());
        let stored = hex("011100eeff73746f72656420626c6f636b2064617461");
        assert!(inflate_raw_deflate_bounded(&stored, 16).is_err());
    }

    #[test]
    fn decodes_a_short_final_code_and_rejects_an_incomplete_tree_hole() {
        let tree = Huffman::new(&[1, 15], 0).unwrap();
        let mut final_bit = BitReader {
            input: &[0],
            bit: 7,
        };
        assert_eq!(tree.decode(&mut final_bit).unwrap(), 0);
        assert_eq!(final_bit.position(), 8);

        let incomplete = Huffman::new(&[1], 0).unwrap();
        let mut invalid = BitReader::new(&[1]);
        assert!(
            incomplete
                .decode(&mut invalid)
                .unwrap_err()
                .to_string()
                .contains("invalid Huffman code")
        );
    }
}
