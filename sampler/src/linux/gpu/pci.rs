//! PCI addresses and adapter names from the `pci.ids` database.

use std::{fs, path::Path};

/// Where distributions install the PCI ID database, below the root.
const PCI_IDS: [&str; 2] = ["usr/share/hwdata/pci.ids", "usr/share/misc/pci.ids"];

/// A PCI address in the kernel's form, for example `0000:03:00.0`, from any spelling of it:
/// NVML writes the domain with eight digits and in upper case.
pub(crate) fn normalize_address(address: &str) -> Option<String> {
    let mut parts = address.trim().split(':');
    let (Some(domain), Some(bus), Some(slot), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let (device, function) = slot.split_once('.')?;
    let domain = u32::from_str_radix(domain, 16).ok()?;
    let bus = u8::from_str_radix(bus, 16).ok()?;
    let device = u8::from_str_radix(device, 16).ok()?;
    let function = u8::from_str_radix(function, 16).ok()?;
    Some(format!("{domain:04x}:{bus:02x}:{device:02x}.{function:x}"))
}

/// The short vendor name for the GPU vendors the UI shows most, where `pci.ids` spells
/// out the company, for example `Advanced Micro Devices, Inc. [AMD/ATI]`.
fn short_vendor(vendor: u16) -> Option<&'static str> {
    match vendor {
        0x1002 => Some("AMD"),
        0x8086 => Some("Intel"),
        0x10de => Some("NVIDIA"),
        _ => None,
    }
}

/// The adapter's display name, for example `AMD Radeon RX 6600/6600 XT/6600M`, or the vendor
/// and `address` when the database or its entry is missing.
pub(crate) fn adapter_name(root: &Path, vendor: u16, device: u16, address: &str) -> String {
    let entry = PCI_IDS
        .iter()
        .find_map(|path| fs::read_to_string(root.join(path)).ok())
        .and_then(|text| lookup(&text, vendor, device));
    let vendor_name = short_vendor(vendor).map(str::to_owned);
    match entry {
        Some((listed_vendor, Some(device_name))) => {
            let vendor_name = vendor_name.unwrap_or(listed_vendor);
            format!("{vendor_name} {}", marketing_name(&device_name))
        }
        Some((listed_vendor, None)) => {
            format!("{} GPU {address}", vendor_name.unwrap_or(listed_vendor))
        }
        None => match vendor_name {
            Some(vendor_name) => format!("{vendor_name} GPU {address}"),
            None => format!("GPU {address}"),
        },
    }
}

/// The bracketed product name of an entry such as `Navi 23 [Radeon RX 6600/6600 XT/6600M]`,
/// which names the chip first; entries without one are used whole.
fn marketing_name(device_name: &str) -> &str {
    device_name
        .strip_suffix(']')
        .and_then(|rest| rest.rsplit_once('['))
        .map_or(device_name, |(_, inside)| inside)
}

/// The vendor's name and, when listed, the device's name. Vendors start a line, devices are
/// indented by one tab and subsystems by two.
fn lookup(text: &str, vendor: u16, device: u16) -> Option<(String, Option<String>)> {
    let mut vendor_name = None;
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(entry) = line.strip_prefix('\t') {
            if vendor_name.is_none() || entry.starts_with('\t') {
                continue;
            }
            if let Some((id, name)) = parse_entry(entry)
                && id == device
            {
                return vendor_name.map(|vendor_name| (vendor_name, Some(name.to_owned())));
            }
        } else if vendor_name.is_some() {
            break;
        } else if let Some((id, name)) = parse_entry(line)
            && id == vendor
        {
            vendor_name = Some(name.to_owned());
        }
    }
    vendor_name.map(|vendor_name| (vendor_name, None))
}

/// A `<4 hex digits>  <name>` line.
fn parse_entry(entry: &str) -> Option<(u16, &str)> {
    let (id, name) = entry.split_once("  ")?;
    if id.len() != 4 {
        return None;
    }
    Some((u16::from_str_radix(id, 16).ok()?, name.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::tests::fixture;

    #[test]
    fn test_normalize_address_shortens_nvml_domain_and_lowercases() {
        assert_eq!(
            normalize_address("00000000:2B:00.0").as_deref(),
            Some("0000:2b:00.0")
        );
        assert_eq!(
            normalize_address("0000:03:00.0").as_deref(),
            Some("0000:03:00.0")
        );
        assert_eq!(normalize_address("ae00000.gpu"), None);
        assert_eq!(normalize_address("0000:03:00"), None);
        assert_eq!(normalize_address("0000:03:00.0:1"), None);
    }

    #[test]
    fn test_adapter_name_uses_the_bracketed_product_name() {
        let root = fixture("gpu/t0");
        assert_eq!(
            adapter_name(&root, 0x1002, 0x73ff, "0000:03:00.0"),
            "AMD Radeon RX 6600/6600 XT/6600M"
        );
        assert_eq!(
            adapter_name(&root, 0x8086, 0x46a6, "0000:00:02.0"),
            "Intel Iris Xe Graphics"
        );
        assert_eq!(
            adapter_name(&root, 0x13b5, 0x0001, "0000:01:00.0"),
            "ARM Mali Example"
        );
    }

    #[test]
    fn test_adapter_name_falls_back_to_vendor_and_address() {
        let root = fixture("gpu/t0");
        assert_eq!(
            adapter_name(&root, 0x1002, 0x9999, "0000:03:00.0"),
            "AMD GPU 0000:03:00.0"
        );
        assert_eq!(
            adapter_name(&root, 0x13b5, 0x9999, "0000:01:00.0"),
            "ARM GPU 0000:01:00.0"
        );
        let empty = fixture("wsl");
        assert_eq!(
            adapter_name(&empty, 0x8086, 0x46a6, "0000:00:02.0"),
            "Intel GPU 0000:00:02.0"
        );
        assert_eq!(
            adapter_name(&empty, 0x1234, 0x0001, "0000:05:00.0"),
            "GPU 0000:05:00.0"
        );
    }

    #[test]
    fn test_lookup_ignores_subsystems_and_stops_at_the_next_vendor() {
        let text = "# comment\n1002  AMD\n\t73ff  Navi 23\n\t\t1002 0e4d  Some board\n\
                    10de  NVIDIA Corporation\n\t13c2  GM204 [GeForce GTX 970]\n";
        assert_eq!(lookup(text, 0x1002, 0x0e4d), Some(("AMD".to_owned(), None)));
        assert_eq!(
            lookup(text, 0x10de, 0x13c2),
            Some((
                "NVIDIA Corporation".to_owned(),
                Some("GM204 [GeForce GTX 970]".to_owned())
            ))
        );
        assert_eq!(lookup(text, 0x8086, 0x0001), None);
    }
}
