use crate::error::{Result, UpdateError};

const SHA256_HEX_LEN: usize = 64;

/// The hash in the `.sha256` file `file`, which must cover `asset`.
///
/// The release workflows write one line, `<hash>  <name>`, the way `sha256sum` and `shasum`
/// print it (the Windows jobs leave off the newline). `install.sh` and `install.ps1` read the
/// first two fields and accept a `*` before the name, which marks binary mode; this does too,
/// and also refuses a second line, so a file that lists several files is never half read.
///
/// The hash comes back in lower case.
pub(crate) fn listed_hash(file: &str, text: &str, asset: &str) -> Result<String> {
    let bad = |reason| UpdateError::BadChecksumFile {
        file: file.to_owned(),
        reason,
    };
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next().ok_or_else(|| bad("it is empty"))?;
    if lines.next().is_some() {
        return Err(bad("it lists more than one file"));
    }
    let mut fields = line.split_whitespace();
    let (Some(hash), Some(name), None) = (fields.next(), fields.next(), fields.next()) else {
        return Err(bad("it is not a hash followed by a file name"));
    };
    if hash.len() != SHA256_HEX_LEN || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad("it does not start with a SHA-256 hash"));
    }
    let name = name.strip_prefix('*').unwrap_or(name);
    if name != asset {
        return Err(UpdateError::ChecksumNamesOther {
            checksum_file: file.to_owned(),
            listed: name.to_owned(),
            expected: asset.to_owned(),
        });
    }
    Ok(hash.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12";
    const ASSET: &str = "allot-0.7.0-x86_64-unknown-linux-gnu.tar.gz";

    fn parse(text: &str) -> Result<String> {
        listed_hash("allot.sha256", text, ASSET)
    }

    #[test]
    fn test_listed_hash_reads_what_the_workflows_write() {
        assert_eq!(parse(&format!("{HASH}  {ASSET}\n")).unwrap(), HASH);
        assert_eq!(parse(&format!("{HASH}  {ASSET}")).unwrap(), HASH);
        assert_eq!(parse(&format!("{HASH}  {ASSET}\r\n")).unwrap(), HASH);
    }

    #[test]
    fn test_listed_hash_accepts_binary_marker_and_upper_case() {
        let upper = HASH.to_ascii_uppercase();
        assert_eq!(parse(&format!("{upper} *{ASSET}\n")).unwrap(), HASH);
    }

    #[test]
    fn test_listed_hash_other_name_is_reported_with_both_names() {
        let result = parse(&format!("{HASH}  allot-0.7.0-setup.exe\n"));
        assert!(matches!(
            result,
            Err(UpdateError::ChecksumNamesOther { listed, expected, .. })
                if listed == "allot-0.7.0-setup.exe" && expected == ASSET
        ));
    }

    #[test]
    fn test_listed_hash_refuses_files_that_are_not_one_checksum() {
        let short = &HASH[..63];
        let not_hex = HASH.replace('a', "g");
        for text in [
            String::new(),
            "\n\n".to_owned(),
            HASH.to_owned(),
            format!("{short}  {ASSET}"),
            format!("{not_hex}  {ASSET}"),
            format!("{HASH}  {ASSET}  extra"),
            format!("{HASH}  {ASSET}\n{HASH}  {ASSET}\n"),
            "<html>not found</html>".to_owned(),
        ] {
            assert!(
                matches!(parse(&text), Err(UpdateError::BadChecksumFile { .. })),
                "{text:?}"
            );
        }
    }
}
