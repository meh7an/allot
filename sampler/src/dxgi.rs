//! Adapter names and VRAM sizes, which PDH does not expose.

use rh_shared::{Error, Result};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};

use crate::gpu::format_luid;

const BYTES_PER_MB: u64 = 1024 * 1024;

/// A hardware GPU adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdapterInfo {
    pub(crate) name: String,
    pub(crate) luid: String,
    pub(crate) dedicated_total_mb: u64,
}

/// Lists hardware adapters, skipping software renderers such as the Basic Render Driver.
pub(crate) fn hardware_adapters() -> Result<Vec<AdapterInfo>> {
    // SAFETY: CreateDXGIFactory1 has no preconditions; the interface type is checked by
    // the `windows` crate through its IID.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
        .map_err(|error| dxgi_error("CreateDXGIFactory1", &error))?;
    let mut adapters = Vec::new();
    for index in 0u32.. {
        // SAFETY: `factory` is a live interface; out-of-range indexes return an error.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(dxgi_error("EnumAdapters1", &error)),
        };
        // SAFETY: `adapter` is a live interface returned above.
        let desc = unsafe { adapter.GetDesc1() }.map_err(|error| dxgi_error("GetDesc1", &error))?;
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let name_len = desc
            .Description
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(desc.Description.len());
        adapters.push(AdapterInfo {
            name: String::from_utf16_lossy(&desc.Description[..name_len]),
            luid: format_luid(desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart),
            dedicated_total_mb: desc.DedicatedVideoMemory as u64 / BYTES_PER_MB,
        });
    }
    Ok(adapters)
}

fn dxgi_error(what: &str, error: &windows::core::Error) -> Error {
    Error::Sampler(format!("{what} failed: {error}"))
}
