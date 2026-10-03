//! Bounded ZIP indexing before handing the archive to the container library.
//! `zip` indexes by name and can hide duplicate central-directory records, so
//! validate the original records before constructing its index.
use super::{validate_path, StorageError};
use relative_path::RelativePath;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom},
    path::Path,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

pub(super) const MAX_MEMBERS: usize = 4096;
const MAX_DIRECTORY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_END_RECORD_BYTES: u64 = 65536;

pub(super) fn invalid(message: impl Into<String>) -> StorageError {
    StorageError::Archive(message.into())
}
pub(super) fn zip_error(error: zip::result::ZipError) -> StorageError {
    invalid(error.to_string())
}

pub(super) fn open(path: &Path) -> Result<ZipArchive<File>, StorageError> {
    let mut file = File::open(path)?;
    let (count, directory_start) = preflight(&mut file)?;
    file.rewind()?;
    let mut archive = ZipArchive::new(file).map_err(zip_error)?;
    if archive.len() != count
        || archive.central_directory_start() != directory_start
        || archive.offset() != 0
    {
        return Err(invalid("inconsistent archive directory"));
    }
    if archive.has_overlapping_files().map_err(zip_error)? {
        return Err(invalid("overlapping ZIP members"));
    }
    for index in 0..archive.len() {
        let member = archive.by_index_raw(index).map_err(zip_error)?;
        if member
            .data_start()
            .and_then(|start| start.checked_add(member.compressed_size()))
            .is_none_or(|end| end > directory_start)
        {
            return Err(invalid("member extends into the central directory"));
        }
    }
    Ok(archive)
}

pub(super) fn options(path: &RelativePath) -> SimpleFileOptions {
    let metadata = matches!(
        path.as_str(),
        "manifest.json" | "calculation.json" | "request.json"
    );
    SimpleFileOptions::default()
        .compression_method(if metadata {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        })
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644)
        // Streaming artifacts have no known size when their local header is emitted.
        // Reserve ZIP64 fields even for small arrays, without buffering the payload.
        .large_file(!metadata)
}

pub(super) fn writer(file: File) -> ZipWriter<BufWriter<File>> {
    ZipWriter::new(BufWriter::new(file))
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn preflight(file: &mut File) -> Result<(usize, u64), StorageError> {
    let length = file.metadata()?.len();
    let tail_len = length.min(22 + u64::from(u16::MAX));
    file.seek(SeekFrom::Start(length - tail_len))?;
    let mut tail =
        vec![0; usize::try_from(tail_len).map_err(|_| invalid("tail size exceeds host range"))?];
    file.read_exact(&mut tail)?;
    let end = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            tail[i..i + 4] == *b"PK\x05\x06"
                && i + 22 + usize::from(u16_at(&tail, i + 20)) == tail.len()
        })
        .ok_or_else(|| invalid("missing or truncated ZIP end record"))?;
    let eocd = &tail[end..];
    let end_offset = length - tail_len
        + u64::try_from(end).map_err(|_| invalid("end offset exceeds V1 range"))?;
    if u16_at(eocd, 4) != 0 || u16_at(eocd, 6) != 0 || u16_at(eocd, 8) != u16_at(eocd, 10) {
        return Err(invalid("multi-disk archives are unsupported"));
    }
    let mut count = u64::from(u16_at(eocd, 10));
    let mut size = u64::from(u32_at(eocd, 12));
    let mut offset = u64::from(u32_at(eocd, 16));
    let mut directory_end = end_offset;
    // A locator can exist even when classic EOCD fields have not overflowed.
    let mut locator = [0; 20];
    let has_locator = if end_offset >= 20 {
        file.seek(SeekFrom::Start(end_offset - 20))?;
        file.read_exact(&mut locator)?;
        locator[..4] == *b"PK\x06\x07"
    } else {
        false
    };
    if has_locator {
        if u32_at(&locator, 4) != 0 || u32_at(&locator, 16) != 1 {
            return Err(invalid("multi-disk ZIP64 archive"));
        }
        let record_offset = u64_at(&locator, 8);
        if record_offset
            .checked_add(56)
            .is_none_or(|end| end > end_offset - 20)
        {
            return Err(invalid("invalid ZIP64 end offset"));
        }
        file.seek(SeekFrom::Start(record_offset))?;
        let mut record = [0; 56];
        file.read_exact(&mut record)?;
        let record_size = u64_at(&record, 4);
        if record[..4] != *b"PK\x06\x06"
            || !(44..=MAX_END_RECORD_BYTES).contains(&record_size)
            || record_offset
                .checked_add(12)
                .and_then(|n| n.checked_add(record_size))
                != Some(end_offset - 20)
            || u32_at(&record, 16) != 0
            || u32_at(&record, 20) != 0
            || u64_at(&record, 24) != u64_at(&record, 32)
        {
            return Err(invalid("invalid ZIP64 end record"));
        }
        let (count64, size64, offset64) = (
            u64_at(&record, 32),
            u64_at(&record, 40),
            u64_at(&record, 48),
        );
        if (count != u64::from(u16::MAX) && count != count64)
            || (size != u64::from(u32::MAX) && size != size64)
            || (offset != u64::from(u32::MAX) && offset != offset64)
        {
            return Err(invalid("inconsistent ZIP64 directory fields"));
        }
        (count, size, offset) = (count64, size64, offset64);
        directory_end = record_offset;
    } else if count == u64::from(u16::MAX)
        || size == u64::from(u32::MAX)
        || offset == u64::from(u32::MAX)
    {
        return Err(invalid("missing ZIP64 end record"));
    }
    if count > u64::try_from(MAX_MEMBERS).map_err(|_| invalid("member limit exceeds V1 range"))?
        || size > MAX_DIRECTORY_BYTES
        || offset.checked_add(size) != Some(directory_end)
    {
        return Err(invalid("oversized or inconsistent ZIP directory"));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut directory =
        vec![0; usize::try_from(size).map_err(|_| invalid("directory size exceeds host range"))?];
    file.read_exact(&mut directory)?;
    let mut cursor = 0_usize;
    let mut paths = BTreeSet::new();
    for _ in 0..count {
        let header = directory
            .get(cursor..cursor + 46)
            .ok_or_else(|| invalid("truncated central header"))?;
        if header[..4] != *b"PK\x01\x02" {
            return Err(invalid("invalid central header"));
        }
        let flags = u16_at(header, 8);
        let method = u16_at(header, 10);
        if flags & !0x080e != 0 || !matches!(method, 0 | 8) {
            return Err(invalid("unsupported encryption, flags or compression"));
        }
        let attributes = u32_at(header, 38);
        let file_type = (attributes >> 16) & 0o170000;
        if attributes & 0x10 != 0 || !matches!(file_type, 0 | 0o100000) {
            return Err(invalid("only regular file members are supported"));
        }
        if u16_at(header, 34) != 0 {
            return Err(invalid("multi-disk member"));
        }
        let name_len = usize::from(u16_at(header, 28));
        let extra_len = usize::from(u16_at(header, 30));
        let comment_len = usize::from(u16_at(header, 32));
        let name_start = cursor + 46;
        let next = name_start + name_len + extra_len + comment_len;
        if next > directory.len() {
            return Err(invalid("truncated directory member"));
        }
        let name_bytes = &directory[name_start..name_start + name_len];
        if !name_bytes.is_ascii() && flags & 0x0800 == 0 {
            return Err(invalid("non-ASCII member names must use the UTF-8 flag"));
        }
        validate_extras(&directory[name_start + name_len..name_start + name_len + extra_len])?;
        let name = std::str::from_utf8(name_bytes).map_err(|_| invalid("non-UTF-8 member path"))?;
        validate_path(RelativePath::new(name))?;
        let key = name.to_lowercase();
        if !paths.insert(key.clone()) {
            return Err(invalid("duplicate member path"));
        }
        for (index, _) in key.match_indices('/') {
            if paths.contains(&key[..index]) {
                return Err(invalid("file/directory path conflict"));
            }
        }
        if paths
            .range(format!("{key}/")..)
            .next()
            .is_some_and(|p| p.starts_with(&format!("{key}/")))
        {
            return Err(invalid("file/directory path conflict"));
        }
        // Resolve only the local offset from ZIP64; the library validates full extras.
        let mut local_offset = u64::from(u32_at(header, 42));
        if local_offset == u64::from(u32::MAX) {
            let extra = &directory[name_start + name_len..name_start + name_len + extra_len];
            let skip = 8
                * (usize::from(u32_at(header, 24) == u32::MAX)
                    + usize::from(u32_at(header, 20) == u32::MAX));
            local_offset = zip64_offset(extra, skip)?;
        }
        if local_offset.checked_add(30).is_none_or(|end| end > offset) {
            return Err(invalid("invalid local header offset"));
        }
        file.seek(SeekFrom::Start(local_offset))?;
        let mut local = [0; 30];
        file.read_exact(&mut local)?;
        let local_name_len = usize::from(u16_at(&local, 26));
        let local_extra_len = u64::from(u16_at(&local, 28));
        if local[..4] != *b"PK\x03\x04"
            || u16_at(&local, 6) != flags
            || u16_at(&local, 8) != method
            || local_name_len != name_len
            || local_offset
                .checked_add(30 + u64::from(u16_at(header, 28)) + local_extra_len)
                .is_none_or(|end| end > offset)
        {
            return Err(invalid("inconsistent local header"));
        }
        let mut local_name = vec![0; name_len];
        file.read_exact(&mut local_name)?;
        if local_name != name_bytes {
            return Err(invalid("local/central member names differ"));
        }
        let mut local_extra = vec![
            0;
            usize::try_from(local_extra_len)
                .map_err(|_| invalid("extra field exceeds host range"))?
        ];
        file.read_exact(&mut local_extra)?;
        validate_extras(&local_extra)?;
        cursor = next;
    }
    if cursor != directory.len() {
        return Err(invalid("unindexed central-directory records"));
    }
    Ok((
        usize::try_from(count).map_err(|_| invalid("member count exceeds host range"))?,
        offset,
    ))
}
fn validate_extras(mut extra: &[u8]) -> Result<(), StorageError> {
    let mut kinds = BTreeSet::new();
    while !extra.is_empty() {
        if extra.len() < 4 {
            return Err(invalid("truncated extra field"));
        }
        let kind = u16_at(extra, 0);
        let len = usize::from(u16_at(extra, 2));
        if kind == 0x7075 || !kinds.insert(kind) {
            return Err(invalid(
                "alternate Unicode paths or duplicate extra fields are unsupported",
            ));
        }
        extra = extra
            .get(4 + len..)
            .ok_or_else(|| invalid("truncated extra field"))?;
    }
    Ok(())
}

fn zip64_offset(mut extra: &[u8], skip: usize) -> Result<u64, StorageError> {
    while extra.len() >= 4 {
        let kind = u16_at(extra, 0);
        let len = usize::from(u16_at(extra, 2));
        let field = extra
            .get(4..4 + len)
            .ok_or_else(|| invalid("truncated extra field"))?;
        if kind == 1 {
            let value = field
                .get(skip..skip + 8)
                .ok_or_else(|| invalid("missing ZIP64 offset"))?;
            return Ok(u64_at(value, 0));
        }
        extra = &extra[4 + len..];
    }
    Err(invalid("missing ZIP64 extra field"))
}

/// Stops decompression at the declared size and checks actual EOF/CRC even when
/// a consumer (such as an NPY decoder) reads exactly the declared payload.
pub(super) fn with_member<T, E, F>(
    archive: &mut ZipArchive<File>,
    path: &RelativePath,
    read: F,
) -> Result<T, E>
where
    E: From<StorageError>,
    F: FnOnce(&mut dyn Read) -> Result<T, E>,
{
    let member = archive
        .by_name(path.as_str())
        .map_err(zip_error)
        .map_err(E::from)?;
    let size = member.size();
    let mut bounded = BufReader::new(member.take(size));
    let value = read(&mut bounded)?;
    io::copy(&mut bounded, &mut io::sink())
        .map_err(StorageError::from)
        .map_err(E::from)?;
    let bounded = bounded.into_inner();
    if bounded.limit() != 0 {
        return Err(E::from(invalid("truncated ZIP member")));
    }
    let mut member = bounded.into_inner();
    let mut trailing = [0; 1];
    if member
        .read(&mut trailing)
        .map_err(StorageError::from)
        .map_err(E::from)?
        != 0
    {
        return Err(E::from(invalid("member exceeds declared size")));
    }
    Ok(value)
}
