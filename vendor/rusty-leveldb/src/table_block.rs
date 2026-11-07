use crate::block::Block;
use crate::blockhandle::BlockHandle;
use crate::crc;
use crate::env::RandomAccess;
use crate::error::{err, Result, StatusCode};
use crate::filter;
use crate::filter_block::FilterBlockReader;
use crate::log::unmask_crc;
use crate::options::Options;
use crate::table_builder;

use integer_encoding::{FixedInt, VarInt};

/// Reads a serialized filter block from a file and returns a FilterBlockReader.
pub fn read_filter_block(
    src: &dyn RandomAccess,
    location: &BlockHandle,
    policy: filter::BoxedFilterPolicy,
    file_size: usize,
) -> Result<FilterBlockReader> {
    if location.size() == 0 {
        return err(
            StatusCode::InvalidArgument,
            "no filter block in empty location",
        );
    }
    let (buf, compression) = read_block_bytes(src, location, file_size)?;
    if compression != 0 || buf.len() < 5 || buf[buf.len() - 1] as u32 >= usize::BITS {
        return err(StatusCode::Corruption, "Invalid filter block");
    }
    let offsets = u32::decode_fixed(&buf[buf.len() - 5..buf.len() - 1]) as usize;
    if offsets > buf.len() - 5 || (buf.len() - 5 - offsets) % 4 != 0 {
        return err(StatusCode::Corruption, "Invalid filter offsets");
    }
    let mut previous = 0;
    for bytes in buf[offsets..buf.len() - 5].chunks_exact(4) {
        let offset = u32::decode_fixed(bytes) as usize;
        if offset < previous || offset > offsets {
            return err(StatusCode::Corruption, "Invalid filter range");
        }
        previous = offset;
    }
    Ok(FilterBlockReader::new_owned(policy, buf))
}

/// Reads a table block from a random-access source.
/// A table block consists of [bytes..., compress (1B), checksum (4B)]; the handle only refers to
/// the location and length of [bytes...].
pub fn read_table_block(
    opt: Options,
    f: &dyn RandomAccess,
    location: &BlockHandle,
    file_size: usize,
) -> Result<Block> {
    let (data, compression) = read_block_bytes(f, location, file_size)?;
    let decoded_data = opt.compressor_list.get(compression)?.decode(data)?;
    if valid_block(&decoded_data).is_none() {
        return err(StatusCode::Corruption, "Invalid table block structure");
    }
    Ok(Block::new(opt, decoded_data.into()))
}

fn valid_block(data: &[u8]) -> Option<()> {
    let tail = data.len().checked_sub(4)?;
    let count = u32::decode_fixed(&data[tail..]) as usize;
    let entries_end = tail.checked_sub(count.checked_mul(4)?)?;
    let mut restarts = data[entries_end..tail]
        .chunks_exact(4)
        .map(|bytes| u32::decode_fixed(bytes) as usize);
    let mut next_restart = restarts.next();
    if next_restart != Some(0) {
        return None;
    }
    if entries_end == 0 {
        return (count == 1).then_some(());
    }
    let varint = |bytes: &mut &[u8]| {
        let (value, length) = usize::decode_var(bytes)?;
        *bytes = &bytes[length..];
        Some(value)
    };
    let mut entries = &data[..entries_end];
    let mut key_length = 0;
    while !entries.is_empty() {
        let offset = entries_end - entries.len();
        if next_restart.is_some_and(|restart| restart < offset) {
            return None;
        }
        let shared = varint(&mut entries)?;
        let non_shared = varint(&mut entries)?;
        let value = varint(&mut entries)?;
        if shared > key_length {
            return None;
        }
        if next_restart == Some(offset) {
            if shared != 0 {
                return None;
            }
            next_restart = restarts.next();
            if next_restart.is_some_and(|restart| restart <= offset) {
                return None;
            }
        }
        entries = entries.get(non_shared.checked_add(value)?..)?;
        key_length = shared.checked_add(non_shared)?;
    }
    next_restart.is_none().then_some(())
}

fn read_block_bytes(
    f: &dyn RandomAccess,
    location: &BlockHandle,
    file_size: usize,
) -> Result<(Vec<u8>, u8)> {
    let data_size = location.size();
    let trailer_size =
        table_builder::TABLE_BLOCK_COMPRESS_LEN + table_builder::TABLE_BLOCK_CKSUM_LEN;
    let total_read_size = data_size
        .checked_add(trailer_size)
        .filter(|&size| size <= 64 * 1024 * 1024)
        .ok_or_else(|| crate::error::Status::new(StatusCode::Corruption, "Invalid block size"))?;
    if !location
        .offset()
        .checked_add(total_read_size)
        .is_some_and(|end| end <= file_size.saturating_sub(table_builder::FULL_FOOTER_LENGTH))
    {
        return err(StatusCode::Corruption, "Block extends beyond table data");
    }

    let mut combined_buf = vec![0; total_read_size];
    let bytes_read = f.read_at(location.offset(), &mut combined_buf)?;

    if bytes_read != total_read_size {
        return err(
            StatusCode::IOError,
            "Failed to read complete block and trailer",
        );
    }

    let block_data_slice = &combined_buf[0..data_size];
    let compress_type = combined_buf[data_size];
    let cksum_slice =
        &combined_buf[data_size + table_builder::TABLE_BLOCK_COMPRESS_LEN..total_read_size];

    let expected_cksum = unmask_crc(u32::decode_fixed(cksum_slice));

    if !verify_table_block(block_data_slice, compress_type, expected_cksum) {
        return err(
            StatusCode::Corruption,
            &format!(
                "checksum verification failed for block at {}",
                location.offset()
            ),
        );
    }
    combined_buf.truncate(data_size);
    Ok((combined_buf, compress_type))
}

/// Verify checksum of block
fn verify_table_block(data: &[u8], compression: u8, want: u32) -> bool {
    let mut digest = crc::digest();
    digest.update(data);
    digest.update(&[compression; 1]);
    digest.finalize() == want
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FourBytes;
    impl RandomAccess for FourBytes {
        fn read_at(&self, _: usize, dst: &mut [u8]) -> Result<usize> {
            Ok(dst.len().min(4))
        }
    }

    #[test]
    fn checksummed_malformed_block_structure_is_rejected() {
        struct Source(Vec<u8>);
        impl RandomAccess for Source {
            fn read_at(&self, offset: usize, dst: &mut [u8]) -> Result<usize> {
                let bytes = self.0.get(offset..).unwrap_or_default();
                let size = bytes.len().min(dst.len());
                dst[..size].copy_from_slice(&bytes[..size]);
                Ok(size)
            }
        }
        for data in [
            vec![0x80, 0, 0, 0, 0, 1, 0, 0, 0],
            vec![0, 1, 1, b'k', 0, 0, 0, 0, 1, 0, 0, 0],
            vec![1, 1, 0, b'k', 0, 0, 0, 0, 1, 0, 0, 0],
            vec![0, 1, 0, b'k', 1, 0, 0, 0, 1, 0, 0, 0],
            vec![0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff],
        ] {
            let length = data.len();
            let mut source = data;
            source.push(0);
            let checksum = crate::log::mask_crc(crate::crc::crc32(&source));
            source.extend_from_slice(&checksum.to_le_bytes());
            source.resize(source.len() + table_builder::FULL_FOOTER_LENGTH, 0);
            let size = source.len();
            assert!(read_table_block(
                crate::options::for_test(),
                &Source(source),
                &BlockHandle::new(0, length),
                size
            )
            .is_err());
        }
    }

    #[test]
    fn impossible_block_length_is_rejected_before_allocation() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            read_table_block(
                crate::options::for_test(),
                &FourBytes,
                &BlockHandle::new(0, usize::MAX),
                4,
            )
        }));
        assert!(matches!(result, Ok(Err(_))));
    }
}
