/// Custom compression method
///
/// ```
/// # use rusty_leveldb::{Compressor, CompressorId};
///
/// #[derive(Debug, Clone, Copy, Default)]
/// pub struct CustomCompressor;
///
/// impl CompressorId for CustomCompressor {
///     // a unique id to identify what compressor should DB use
///     const ID: u8 = 42;
/// }
///
/// impl Compressor for CustomCompressor {
///     fn encode(&self, block: Vec<u8>) -> rusty_leveldb::Result<Vec<u8>> {
///         // Do something
///         Ok(block)
///     }
///
///     fn decode(&self, block: Vec<u8>) -> rusty_leveldb::Result<Vec<u8>> {
///         // Do something
///         Ok(block)
///     }
/// }
/// ```
///
/// See [crate::CompressorList] for usage
pub trait Compressor {
    fn encode(&self, block: Vec<u8>) -> crate::Result<Vec<u8>>;

    fn decode(&self, block: Vec<u8>) -> crate::Result<Vec<u8>>;
}

/// Set default compressor id
pub trait CompressorId {
    const ID: u8;
}

/// A compressor that do **Nothing**
///
/// It default id is `0`
#[derive(Debug, Clone, Copy, Default)]
pub struct NoneCompressor;

impl CompressorId for NoneCompressor {
    const ID: u8 = 0;
}

impl Compressor for NoneCompressor {
    fn encode(&self, block: Vec<u8>) -> crate::Result<Vec<u8>> {
        Ok(block)
    }

    fn decode(&self, block: Vec<u8>) -> crate::Result<Vec<u8>> {
        Ok(block)
    }
}

/// A compressor that compress data with Google's Snappy
///
/// It default id is `1`
#[derive(Debug, Clone, Copy, Default)]
pub struct SnappyCompressor;

impl CompressorId for SnappyCompressor {
    const ID: u8 = 1;
}

impl Compressor for SnappyCompressor {
    fn encode(&self, block: Vec<u8>) -> crate::Result<Vec<u8>> {
        Ok(snap::raw::Encoder::new().compress_vec(&block)?)
    }

    fn decode(&self, block: Vec<u8>) -> crate::Result<Vec<u8>> {
        // The migration reader accepts at most 64 MiB of decoded data per block.
        if snap::raw::decompress_len(&block)? > 64 * 1024 * 1024 {
            return crate::error::err(crate::StatusCode::Corruption, "Snappy block exceeds 64 MiB");
        }
        Ok(snap::raw::Decoder::new().decompress_vec(&block)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_snappy_declaration_is_rejected_before_allocation() {
        let error = SnappyCompressor
            .decode(vec![0x81, 0x80, 0x80, 0x20])
            .unwrap_err();
        assert!(error.to_string().contains("exceeds 64 MiB"), "{error}");
    }
}
