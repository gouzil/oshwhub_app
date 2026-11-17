//! A log consists of a number of blocks.
//! A block consists of a number of records and an optional trailer (filler).
//! A record is a bytestring: [checksum: uint32, length: uint16, type: uint8, data: [u8]]
//! checksum is the crc32 sum of type and data; type is one of RecordType::{Full/First/Middle/Last}

use crate::crc;
use crate::error::{err, Result, StatusCode};

use std::io::{Read, Write};

use integer_encoding::FixedInt;
use integer_encoding::FixedIntWriter;

const BLOCK_SIZE: usize = 32 * 1024;
const HEADER_SIZE: usize = 4 + 2 + 1;

#[derive(Clone, Copy)]
pub enum RecordType {
    Full = 1,
    First = 2,
    Middle = 3,
    Last = 4,
}

pub struct LogWriter<W: Write> {
    dst: W,
    current_block_offset: usize,
    block_size: usize,
}

impl<W: Write> LogWriter<W> {
    pub fn new(writer: W) -> LogWriter<W> {
        LogWriter {
            dst: writer,
            current_block_offset: 0,
            block_size: BLOCK_SIZE,
        }
    }

    /// new_with_off opens a writer starting at some offset of an existing log file. The file must
    /// have the default block size.
    pub fn new_with_off(writer: W, off: usize) -> LogWriter<W> {
        let mut w = LogWriter::new(writer);
        w.current_block_offset = off % BLOCK_SIZE;
        w
    }

    pub fn add_record(&mut self, mut record: &[u8]) -> Result<usize> {
        let mut first_frag = true;
        let mut result = Ok(0);
        while result.is_ok() && !record.is_empty() {
            assert!(self.block_size > HEADER_SIZE);

            let space_left = self.block_size - self.current_block_offset;

            // Fill up block; go to next block.
            if space_left < HEADER_SIZE {
                self.dst.write_all(&vec![0, 0, 0, 0, 0, 0][0..space_left])?;
                self.current_block_offset = 0;
            }

            let avail_for_data = self.block_size - self.current_block_offset - HEADER_SIZE;

            let data_frag_len = if record.len() < avail_for_data {
                record.len()
            } else {
                avail_for_data
            };

            let recordtype = if first_frag && data_frag_len == record.len() {
                RecordType::Full
            } else if first_frag {
                RecordType::First
            } else if data_frag_len == record.len() {
                RecordType::Last
            } else {
                RecordType::Middle
            };

            result = self.emit_record(recordtype, record, data_frag_len);
            record = &record[data_frag_len..];
            first_frag = false;
        }
        result
    }

    fn emit_record(&mut self, t: RecordType, data: &[u8], len: usize) -> Result<usize> {
        assert!(len < 256 * 256);

        let mut digest = crc::digest();
        digest.update(&[t as u8]);
        digest.update(&data[0..len]);

        let chksum = mask_crc(digest.finalize());

        let mut s = 0;
        s += self.dst.write(&chksum.encode_fixed_vec())?;
        s += self.dst.write_fixedint(len as u16)?;
        s += self.dst.write(&[t as u8])?;
        s += self.dst.write(&data[0..len])?;

        self.current_block_offset += s;
        Ok(s)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.dst.flush()?;
        Ok(())
    }
}

pub struct LogReader<R: Read> {
    // TODO: Wrap src in a buffer to enhance read performance.
    src: R,
    blk_off: usize,
    blocksize: usize,
    head_scratch: [u8; 7],
    checksums: bool,
}

impl<R: Read> LogReader<R> {
    pub fn new(src: R, chksum: bool) -> LogReader<R> {
        LogReader {
            src,
            blk_off: 0,
            blocksize: BLOCK_SIZE,
            checksums: chksum,
            head_scratch: [0; 7],
        }
    }

    /// EOF is signalled by Ok(0)
    pub fn read(&mut self, dst: &mut Vec<u8>) -> Result<usize> {
        dst.clear();
        let mut fragmented = false;
        loop {
            let remaining = self.blocksize - self.blk_off;
            if remaining < HEADER_SIZE {
                let read = self.read_scratch(remaining)?;
                if self.head_scratch[..read].iter().any(|&byte| byte != 0) {
                    return err(StatusCode::Corruption, "Invalid log block trailer");
                }
                // A writer need not pad the final block before closing.
                if read < remaining {
                    return if fragmented {
                        err(StatusCode::Corruption, "Incomplete log record")
                    } else {
                        Ok(0)
                    };
                }
                self.blk_off = 0;
            }

            let bytes_read = self.read_scratch(HEADER_SIZE)?;
            if bytes_read == 0 {
                return if fragmented {
                    err(StatusCode::Corruption, "Incomplete log record")
                } else {
                    Ok(0)
                };
            }
            if bytes_read != HEADER_SIZE {
                return err(StatusCode::Corruption, "Truncated log header");
            }
            self.blk_off += bytes_read;

            let checksum = u32::decode_fixed(&self.head_scratch[..4]);
            let length = u16::decode_fixed(&self.head_scratch[4..6]) as usize;
            let typ = self.head_scratch[6];
            if length > self.blocksize - self.blk_off || dst.len() + length > 64 * 1024 * 1024 {
                return err(StatusCode::Corruption, "Invalid log record length");
            }
            let dst_offset = dst.len();
            dst.resize(dst_offset + length, 0);
            self.src.read_exact(&mut dst[dst_offset..])?;
            self.blk_off += length;

            if self.checksums && !self.check_integrity(typ, &dst[dst_offset..], checksum) {
                return err(StatusCode::Corruption, "Invalid Checksum");
            }

            match (typ, fragmented) {
                (1, false) | (4, true) => {
                    if dst.is_empty() {
                        return err(StatusCode::Corruption, "Empty log record");
                    }
                    return Ok(dst.len());
                }
                (2, false) => fragmented = true,
                (3, true) => {}
                _ => return err(StatusCode::Corruption, "Invalid log fragment type"),
            }
        }
    }

    fn read_scratch(&mut self, length: usize) -> Result<usize> {
        let mut read = 0;
        while read < length {
            match self.src.read(&mut self.head_scratch[read..length]) {
                Ok(0) => break,
                Ok(size) => read += size,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(read)
    }

    fn check_integrity(&mut self, typ: u8, data: &[u8], expected: u32) -> bool {
        let mut digest = crc::digest();
        digest.update(&[typ]);
        digest.update(data);
        unmask_crc(expected) == digest.finalize()
    }
}

const MASK_DELTA: u32 = 0xa282ead8;

pub fn mask_crc(c: u32) -> u32 {
    (c.wrapping_shr(15) | c.wrapping_shl(17)).wrapping_add(MASK_DELTA)
}

pub fn unmask_crc(mc: u32) -> u32 {
    let rot = mc.wrapping_sub(MASK_DELTA);
    rot.wrapping_shr(17) | rot.wrapping_shl(15)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn empty_full_record_cannot_hide_the_rest_of_a_log() {
        let mut writer = LogWriter::new(Vec::new());
        writer.emit_record(RecordType::Full, &[], 0).unwrap();
        writer.add_record(b"record after empty entry").unwrap();
        let mut reader = LogReader::new(writer.dst.as_slice(), true);
        assert!(reader.read(&mut Vec::new()).is_err());
    }

    #[test]
    fn incomplete_logical_record_is_an_error() {
        let mut writer = LogWriter::new(Vec::new());
        writer.add_record(&vec![42; BLOCK_SIZE * 2]).unwrap();
        let mut reader = LogReader::new(&writer.dst[..BLOCK_SIZE], true);
        assert!(reader.read(&mut Vec::new()).is_err());
    }

    #[test]
    fn complete_record_at_unpadded_block_end_allows_eof() {
        let mut writer = LogWriter::new(Vec::new());
        writer
            .add_record(&vec![42; BLOCK_SIZE - HEADER_SIZE - 1])
            .unwrap();
        let mut reader = LogReader::new(writer.dst.as_slice(), true);
        assert_eq!(
            reader.read(&mut Vec::new()).unwrap(),
            BLOCK_SIZE - HEADER_SIZE - 1
        );
        assert_eq!(reader.read(&mut Vec::new()).unwrap(), 0);
    }

    #[test]
    fn test_crc_mask_crc() {
        let mut digest = crc::digest();
        digest.update("abcde".as_bytes());
        let sum = digest.finalize();
        assert_eq!(sum, unmask_crc(mask_crc(sum)));
        assert!(sum != mask_crc(sum));
    }

    #[test]
    fn test_crc_sanity() {
        assert_eq!(0x8a9136aa, crc::crc32([0_u8; 32]));
        assert_eq!(0x62a8ab43, crc::crc32([0xff_u8; 32]));
    }

    #[test]
    fn test_writer() {
        let data = &[
            "hello world. My first log entry.",
            "and my second",
            "and my third",
        ];
        let mut lw = LogWriter::new(Vec::new());
        let total_len = data.iter().fold(0, |l, d| l + d.len());

        for d in data {
            let _ = lw.add_record(d.as_bytes());
        }

        assert_eq!(lw.current_block_offset, total_len + 3 * super::HEADER_SIZE);
    }

    #[test]
    fn test_writer_append() {
        let data = &[
            "hello world. My first log entry.",
            "and my second",
            "and my third",
        ];

        let mut dst = vec![0_u8; 1024];

        {
            let mut lw = LogWriter::new(Cursor::new(dst.as_mut_slice()));
            for d in data {
                let _ = lw.add_record(d.as_bytes());
            }
        }

        let old = dst.clone();

        // Ensure that new_with_off positions the writer correctly. Some ugly mucking about with
        // cursors and stuff is required.
        {
            let offset = data[0].len() + super::HEADER_SIZE;
            let mut lw =
                LogWriter::new_with_off(Cursor::new(&mut dst.as_mut_slice()[offset..]), offset);
            for d in &data[1..] {
                let _ = lw.add_record(d.as_bytes());
            }
        }
        assert_eq!(old, dst);
    }

    #[test]
    fn test_reader() {
        let data = [
            "abcdefghi".as_bytes().to_vec(),    // fits one block of 17
            "123456789012".as_bytes().to_vec(), // spans two blocks of 17
            "0101010101010101010101".as_bytes().to_vec(),
        ]; // spans three blocks of 17
        let mut lw = LogWriter::new(Vec::new());
        lw.block_size = super::HEADER_SIZE + 10;

        for e in data.iter() {
            assert!(lw.add_record(e).is_ok());
        }

        assert_eq!(lw.dst.len(), 93);
        // Corrupt first record.
        lw.dst[2] += 1;

        let mut lr = LogReader::new(lw.dst.as_slice(), true);
        lr.blocksize = super::HEADER_SIZE + 10;
        let mut dst = Vec::with_capacity(128);

        // First record is corrupted.
        assert_eq!(
            err(StatusCode::Corruption, "Invalid Checksum"),
            lr.read(&mut dst)
        );

        let mut i = 1;
        loop {
            match lr.read(&mut dst) {
                Err(e) => panic!("{}", e),
                Ok(0) => break,
                _ => {}
            }

            assert_eq!(dst, data[i]);
            i += 1;
        }
        assert_eq!(i, data.len());
    }
}
