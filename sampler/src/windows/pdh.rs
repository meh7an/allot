//! A thin safe wrapper over a Windows PDH query.

use std::mem;

use allot_shared::{Error, Result};
use windows::{
    Win32::System::Performance::{
        PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE,
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhGetFormattedCounterValue, PdhOpenQueryW,
    },
    core::{HSTRING, PCWSTR},
};

const SUCCESS: u32 = 0;

// Instances can appear between the size probe and the read; retrying a few times covers the
// rare race without looping forever.
const MAX_ARRAY_ATTEMPTS: usize = 4;

/// An open PDH query. Counters added to it are read on every [`Query::collect`].
#[derive(Debug)]
pub(crate) struct Query {
    handle: PDH_HQUERY,
}

/// A counter inside a [`Query`].
#[derive(Debug)]
pub(crate) struct Counter {
    handle: PDH_HCOUNTER,
}

impl Query {
    /// Opens a query on live data.
    pub(crate) fn open() -> Result<Self> {
        let mut handle = PDH_HQUERY::default();
        // SAFETY: `handle` is a valid out-pointer for the duration of the call.
        check(
            unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut handle) },
            "PdhOpenQueryW",
        )?;
        Ok(Self { handle })
    }

    /// Adds a counter by its English path, which works on every display language.
    pub(crate) fn add(&self, path: &str) -> Result<Counter> {
        let mut handle = PDH_HCOUNTER::default();
        // SAFETY: the query handle is open, and `path` and `handle` outlive the call.
        let status =
            unsafe { PdhAddEnglishCounterW(self.handle, &HSTRING::from(path), 0, &mut handle) };
        check(status, path)?;
        Ok(Counter { handle })
    }

    /// Takes a new reading of every counter in the query.
    pub(crate) fn collect(&self) -> Result<()> {
        // SAFETY: the query handle is open.
        check(
            unsafe { PdhCollectQueryData(self.handle) },
            "PdhCollectQueryData",
        )
    }

    /// Reads a single-instance counter. Returns `None` until it has valid data, which for
    /// rate counters takes two collections.
    pub(crate) fn value(&self, counter: &Counter) -> Result<Option<f64>> {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: the counter belongs to this open query and `value` is a valid out-pointer.
        let status = unsafe {
            PdhGetFormattedCounterValue(counter.handle, PDH_FMT_DOUBLE, None, &mut value)
        };
        if status != SUCCESS || !is_valid(value.CStatus) {
            return Ok(None);
        }
        // SAFETY: PDH_FMT_DOUBLE makes PDH fill the `doubleValue` member.
        Ok(Some(unsafe { value.Anonymous.doubleValue }))
    }

    /// Reads every instance of a wildcard counter as `(instance name, value)` pairs, skipping
    /// instances without valid data.
    pub(crate) fn values(&self, counter: &Counter) -> Result<Vec<(String, f64)>> {
        for _ in 0..MAX_ARRAY_ATTEMPTS {
            let mut size = 0;
            let mut count = 0;
            // SAFETY: a null buffer with size 0 asks PDH only for the required size.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter.handle,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    None,
                )
            };
            if status == SUCCESS {
                return Ok(Vec::new());
            }
            if status != PDH_MORE_DATA {
                return Err(pdh_error("PdhGetFormattedCounterArrayW", status));
            }
            // The buffer holds the items followed by their names. Allocating it as items keeps
            // the `f64` values aligned.
            let item_size = mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let mut items =
                vec![PDH_FMT_COUNTERVALUE_ITEM_W::default(); (size as usize).div_ceil(item_size)];
            // SAFETY: `items` spans at least `size` bytes and outlives the call.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter.handle,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    Some(items.as_mut_ptr()),
                )
            };
            if status == PDH_MORE_DATA {
                continue;
            }
            check(status, "PdhGetFormattedCounterArrayW")?;
            let read = items
                .iter()
                .take(count as usize)
                .filter(|item| is_valid(item.FmtValue.CStatus))
                .filter_map(|item| {
                    // SAFETY: PDH points `szName` at a NUL-terminated string inside `items`,
                    // and PDH_FMT_DOUBLE fills the `doubleValue` member.
                    let name = unsafe { item.szName.to_string() }.ok()?;
                    Some((name, unsafe { item.FmtValue.Anonymous.doubleValue }))
                })
                .collect();
            return Ok(read);
        }
        Err(Error::Sampler(
            "PdhGetFormattedCounterArrayW kept growing between calls".into(),
        ))
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `Query::open` and is closed exactly once here.
        unsafe { PdhCloseQuery(self.handle) };
    }
}

fn is_valid(status: u32) -> bool {
    status == PDH_CSTATUS_VALID_DATA || status == PDH_CSTATUS_NEW_DATA
}

fn check(status: u32, what: &str) -> Result<()> {
    if status == SUCCESS {
        Ok(())
    } else {
        Err(pdh_error(what, status))
    }
}

fn pdh_error(what: &str, status: u32) -> Error {
    Error::Sampler(format!("{what} failed with PDH status 0x{status:08X}"))
}
