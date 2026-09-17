//! Execution-resource discovery and policy for scientific calculations.
//!
//! This module owns machine-dependent decisions used by the scientific core.
//! Configuration types remain independent of operating-system and container
//! resource discovery.

use bytesize::ByteSize;
use sysinfo::{
    get_current_pid, MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System,
};

use crate::config::MemoryLimit;

const FALLBACK_MP2_MEMORY: u64 = 512 * 1024 * 1024;

/// Resolve an MP2 memory policy once before transformation buffers are allocated.
pub(crate) fn resolve_mp2_memory_limit(limit: MemoryLimit) -> ByteSize {
    match limit {
        MemoryLimit::Auto => automatic_mp2_memory_limit(),
        MemoryLimit::Fixed(value) => value,
    }
}

fn automatic_mp2_memory_limit() -> ByteSize {
    from_available_memory(available_memory())
}

fn available_memory() -> Option<u64> {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        return None;
    }

    let mut system = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    let host_available = system.available_memory();

    let cgroup_available = get_current_pid().ok().and_then(|pid| {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            false,
            ProcessRefreshKind::nothing(),
        );
        system
            .process(pid)
            .and_then(|process| process.cgroup_limits())
            .map(|limits| limits.free_memory)
    });

    Some(cgroup_available.map_or(host_available, |available| host_available.min(available)))
}

fn from_available_memory(available: Option<u64>) -> ByteSize {
    let bytes = available
        .map(|bytes| bytes / 2)
        .unwrap_or(FALLBACK_MP2_MEMORY);
    ByteSize::b(bytes.min(isize::MAX as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_memory_uses_half_available_bytes_and_safe_fallback() {
        for (input, expected) in [
            (Some(4 * 1024 * 1024), 2 * 1024 * 1024),
            (Some(1), 0),
            (Some(0), 0),
            (None, FALLBACK_MP2_MEMORY),
            (Some(u64::MAX), isize::MAX as u64),
        ] {
            assert_eq!(from_available_memory(input).as_u64(), expected);
        }
    }

    #[test]
    fn fixed_memory_limit_is_preserved_exactly() {
        assert_eq!(
            resolve_mp2_memory_limit(MemoryLimit::Fixed(ByteSize::b(513))).as_u64(),
            513
        );
    }
}
