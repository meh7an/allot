//! Reading the IOKit registry: the services of a class with chosen properties (and their
//! children's), and the storage driver behind a volume. Everything here works without root.

use std::ffi::{CStr, CString, c_char};

use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType,
};
use objc2_io_kit::{
    IOBSDNameMatching, IOIteratorNext, IOObjectConformsTo, IOObjectRelease,
    IORegistryEntryCreateCFProperty, IORegistryEntryGetChildIterator,
    IORegistryEntryGetParentEntry, IORegistryEntryGetRegistryEntryID, IOServiceGetMatchingService,
    IOServiceGetMatchingServices, IOServiceMatching, io_iterator_t, io_object_t,
    kIOMainPortDefault, kIOServicePlane,
};

use crate::macos::value::{Entry, Value};

/// How far up from a volume the storage driver is looked for. An APFS volume sits about ten
/// levels below its disk's driver; the limit only guards against a malformed registry.
const MAX_DRIVER_DEPTH: usize = 32;

/// The registered services of `class`, each with the properties `keys` and, when
/// `child_keys` is not empty, its children with the properties `child_keys`.
pub(crate) fn entries(class: &CStr, keys: &[&str], child_keys: &[&str]) -> Vec<Entry> {
    services(class)
        .iter()
        .map(|service| service.entry(keys, child_keys))
        .collect()
}

/// The registry ID of the `IOBlockStorageDriver` of the disk that holds the volume
/// `bsd_name` (for example `disk3s1s1`): the first driver above it in the service plane,
/// which for an APFS volume is the driver of the container's physical disk.
pub(crate) fn storage_driver_id(bsd_name: &str) -> Option<u64> {
    let name = CString::new(bsd_name).ok()?;
    // SAFETY: `name` is NUL-terminated and outlives the call.
    let matching = unsafe { IOBSDNameMatching(kIOMainPortDefault, 0, name.as_ptr()) }?;
    // SAFETY: a mutable dictionary is a dictionary; the call consumes this reference.
    let matching = unsafe { CFRetained::cast_unchecked::<CFDictionary>(matching) };
    // SAFETY: the matching dictionary is valid and consumed by the call.
    let media = unsafe { IOServiceGetMatchingService(kIOMainPortDefault, Some(matching)) };
    let mut entry = Object::new(media)?;
    for _ in 0..MAX_DRIVER_DEPTH {
        if entry.conforms_to(c"IOBlockStorageDriver") {
            return Some(entry.id());
        }
        entry = entry.parent()?;
    }
    None
}

/// A registry object, released when dropped.
#[derive(Debug)]
struct Object(io_object_t);

impl Object {
    /// `None` for the null object, which IOKit returns when nothing matched.
    fn new(object: io_object_t) -> Option<Self> {
        (object != 0).then_some(Self(object))
    }

    fn id(&self) -> u64 {
        let mut id = 0;
        // SAFETY: `id` is writable and outlives the call.
        unsafe { IORegistryEntryGetRegistryEntryID(self.0, &mut id) };
        id
    }

    fn conforms_to(&self, class: &CStr) -> bool {
        // SAFETY: IOKit only reads the NUL-terminated class name, despite the mutable type.
        unsafe { IOObjectConformsTo(self.0, name(class)) }
    }

    fn parent(&self) -> Option<Self> {
        let mut parent = 0;
        // SAFETY: the plane name is NUL-terminated and `parent` is writable.
        let result =
            unsafe { IORegistryEntryGetParentEntry(self.0, name(kIOServicePlane), &mut parent) };
        if result != 0 {
            return None;
        }
        Self::new(parent)
    }

    fn children(&self) -> Vec<Self> {
        let mut iterator: io_iterator_t = 0;
        // SAFETY: the plane name is NUL-terminated and `iterator` is writable.
        let result = unsafe {
            IORegistryEntryGetChildIterator(self.0, name(kIOServicePlane), &mut iterator)
        };
        if result != 0 {
            return Vec::new();
        }
        drain(iterator)
    }

    fn property(&self, key: &str) -> Option<Value> {
        let key = CFString::from_str(key);
        // SAFETY: the key is a valid CF string; no allocator and no options are documented
        // defaults.
        let value = unsafe { IORegistryEntryCreateCFProperty(self.0, Some(&key), None, 0) }?;
        Some(convert(&value))
    }

    /// This object as an [`Entry`] with the properties `keys` (missing ones left out), and
    /// its children with `child_keys` when that is not empty.
    fn entry(&self, keys: &[&str], child_keys: &[&str]) -> Entry {
        let properties = keys
            .iter()
            .filter_map(|&key| Some((key.to_owned(), self.property(key)?)))
            .collect();
        let children = if child_keys.is_empty() {
            Vec::new()
        } else {
            self.children()
                .iter()
                .map(|child| child.entry(child_keys, &[]))
                .collect()
        };
        Entry {
            id: self.id(),
            properties: Value::Dict(properties),
            children,
        }
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        IOObjectRelease(self.0);
    }
}

/// IOKit takes plane and class names as `io_name_t`, a mutable fixed-size array, but only
/// reads them up to their NUL.
fn name(text: &CStr) -> *mut [c_char; 128] {
    text.as_ptr().cast_mut().cast()
}

fn services(class: &CStr) -> Vec<Object> {
    // SAFETY: `class` is NUL-terminated.
    let Some(matching) = (unsafe { IOServiceMatching(class.as_ptr()) }) else {
        return Vec::new();
    };
    // SAFETY: a mutable dictionary is a dictionary; the call consumes this reference.
    let matching = unsafe { CFRetained::cast_unchecked::<CFDictionary>(matching) };
    let mut iterator: io_iterator_t = 0;
    // SAFETY: the matching dictionary is valid and consumed, and `iterator` is writable.
    let result =
        unsafe { IOServiceGetMatchingServices(kIOMainPortDefault, Some(matching), &mut iterator) };
    if result != 0 {
        return Vec::new();
    }
    drain(iterator)
}

/// Every object of `iterator`, which is released afterwards.
fn drain(iterator: io_iterator_t) -> Vec<Object> {
    let iterator = Object(iterator);
    std::iter::from_fn(|| Object::new(IOIteratorNext(iterator.0))).collect()
}

/// A CF property value as a [`Value`]. Types the sampler never reads become
/// [`Value::Other`].
fn convert(value: &CFType) -> Value {
    if let Some(number) = value.downcast_ref::<CFNumber>() {
        return number
            .as_i64()
            .map(Value::Int)
            .or_else(|| number.as_f64().map(Value::Float))
            .unwrap_or(Value::Other);
    }
    if let Some(flag) = value.downcast_ref::<CFBoolean>() {
        return Value::Bool(flag.as_bool());
    }
    if let Some(text) = value.downcast_ref::<CFString>() {
        return Value::Str(text.to_string());
    }
    if let Some(array) = value.downcast_ref::<CFArray>() {
        // SAFETY: every element of a property array is a CF object.
        let array = unsafe { array.cast_unchecked::<CFType>() };
        return Value::Array(array.to_vec().iter().map(|item| convert(item)).collect());
    }
    if let Some(dictionary) = value.downcast_ref::<CFDictionary>() {
        // SAFETY: property dictionaries map CF strings to CF objects.
        let dictionary = unsafe { dictionary.cast_unchecked::<CFType, CFType>() };
        let (keys, values) = dictionary.to_vecs();
        let entries = keys
            .iter()
            .zip(&values)
            .filter_map(|(key, value)| {
                let key = key.downcast_ref::<CFString>()?.to_string();
                Some((key, convert(value)))
            })
            .collect();
        return Value::Dict(entries);
    }
    Value::Other
}
