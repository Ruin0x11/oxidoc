use std::env;
use std::char;
use std::fs;
use std::path::{PathBuf};
use std::io;
use std::io::{BufRead, Cursor, Write};
use io_support::{self, CharsError, write_char};
use self::DecodeState::*;

error_chain! {
    errors {
        /// A numerical escape sequence (&# or &#x) resolved to an invalid unicode code point.
        /// Example: `&#xffffff`
        InvalidCharacter {
            description("Invalid unicode code point")
        }

        /// A numerical escape sequence (&# or &#x) containing an invalid character.
        /// Examples: `&#32a`, `&#xfoo`
        MalformedNumEscape {
            description("Invalid numerical escape sequence")
        }

        /// The input ended prematurely (ie. inside an unterminated named entity sequence).
        PrematureEnd {
            description("Input ended prematurely")
        }

        /// An IO error occured.
        IoError(err: io::Error) {
            description("IO error occurred")
            display("error: {}", err)
        }

        /// The supplied Reader produces invalid UTF-8.
        EncodingError {
            description("Invalid UTF-8 produced by Reader")
        }
    }
}

#[derive(PartialEq, Eq)]
enum DecodeState {
    Normal,
    Numeric,
    Hex,
}

macro_rules! try_parse(
    ($parse:expr, $pos:expr) => (
        match $parse {
            Err(reason) => bail!(reason),
            Ok(res) => res
        }
    ););

macro_rules! try_dec_io(
    ($io:expr, $pos:expr) => (
        match $io {
            Err(e) => bail!(ErrorKind::IoError(e)),
            Ok(res) => res
        }
    ););

pub fn src_iter(system: bool, cargo: bool) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    if system {
        // TODO: push system .odoc directory
    }

    if cargo {
        let cargo_src_path: PathBuf;
        if let Some(x) = env::home_dir() {
            cargo_src_path = x.join(".cargo/registry/src");
        } else {
            bail!("Could not get home directory");
        }

        let mut repo_paths = fs::read_dir(cargo_src_path.as_path())
            .chain_err(|| "Couldn't read cargo source path")?;

        // TODO: unsure what format the github-xxxx directories follow.
        let first = repo_paths.next().unwrap().unwrap();
        let meta = first.metadata();
        assert!(meta.is_ok());
        assert!(meta.unwrap().is_dir());
        let repo_path = first.path();

        let crate_src_paths = fs::read_dir(repo_path)
            .chain_err(|| "Couldn't read cargo repo path")?;

        for src in crate_src_paths {
            if let Ok(src_dir) = src {
                if let Ok(metadata) = src_dir.metadata() {
                    if metadata.is_dir() {
                        paths.push(src_dir.path());
                    }
                }
            }
        }
    }

    Ok(paths)
}

pub fn doc_iter(system: bool, cargo: bool) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    if system {
        // TODO: push system .odoc directory
    }

    if cargo {
        let cargo_doc_path: PathBuf;
        if let Some(x) = env::home_dir() {
            cargo_doc_path = x.join(".cargo/registry/doc");
        } else {
            bail!("Could not get home directory");
        }

        fs::create_dir_all(&cargo_doc_path.as_path())
            .chain_err(|| format!("Failed to create doc dir {}", &cargo_doc_path.display()))?;

        let doc_paths = fs::read_dir(cargo_doc_path.as_path())
            .chain_err(|| "Couldn't read cargo doc path")?;

        for doc in doc_paths {
            if let Ok(doc_dir) = doc {
                if let Ok(metadata) = doc_dir.metadata() {
                    if metadata.is_dir() {
                        paths.push(doc_dir.path());
                    }
                }
            }
        }
    }

    Ok(paths)
}

pub fn encode_doc_filename_w<W: Write>(s: &str, writer: &mut W) -> io::Result<()> {
    // NOTE: This will probably have to handle utf-8 in the future. (rust issue #28979)
    for c in s.chars() {
        let byte = c as usize;
        let res = if byte < 256 && (byte > 127 || !is_ascii_alnum(c)) {
            write_hex(writer, c)
        } else {
            write_char(writer, c)
        };
        try!(res);
    }
    Ok(())
}

/// Escapes non-alphanumeric characters inside names that could be used by oxidoc to create documentation.
pub fn encode_doc_filename(name: &str) -> Result<String> {
    let mut writer = Vec::with_capacity(name.len() * 3);
    match encode_doc_filename_w(name, &mut writer) {
        Err(_) => bail!("Failed to encode doc filename"),
        Ok(_) => Ok(String::from_utf8(writer).unwrap())
    }
}

/// Decodes a documentation filename.
pub fn decode_doc_filename(name: &str) -> Result<String> {
    let mut writer = Vec::with_capacity(name.len());
    let bytes = name.as_bytes();
    let mut reader = Cursor::new(bytes);
    let res = decode_doc_filename_rw(&mut reader, &mut writer);
    match res {
        Ok(_) => Ok(String::from_utf8(writer).unwrap()),
        Err(err) => Err(err)
    }
}

pub fn decode_doc_filename_rw<R: BufRead, W: Write>(reader: R, writer: &mut W) -> Result<()> {
    let mut state: DecodeState = Normal;
    let mut pos = 0;
    let mut buf = String::with_capacity(8);
    for c in io_support::chars(reader) {
        let c = match c {
            Err(e) => {
                let kind = match e {
                    CharsError::NotUtf8   => ErrorKind::EncodingError,
                    CharsError::Other(io) => ErrorKind::IoError(io)
                };
                bail!(kind)
            }
            Ok(c) => c
        };
        match state {
            Normal if c == '%' => state = Numeric,
            Normal => try_dec_io!(write_char(writer, c), good_pos),
            Numeric if c == 'x' => state = Hex,
            Hex if c == ';' => {
                state = Normal;
                let ch = try_parse!(decode_numeric(&buf, 16), good_pos);
                try_dec_io!(write_char(writer, ch), good_pos);
                buf.clear();
            }
            Hex if is_hex_digit(c) => buf.push(c),
            Numeric | Hex => bail!(ErrorKind::MalformedNumEscape),
        }
        pos += 1;
    }
    if state != Normal {
        bail!(ErrorKind::PrematureEnd)
    } else {
        Ok(())
    }
}

fn write_hex<W: Write>(writer: &mut W, c: char) -> io::Result<()> {
    let hex = b"0123456789ABCDEF";
    try!(writer.write(b"%x"));
    let n = c as u8;
    let bytes = [hex[((n & 0xF0) >> 4) as usize],
                 hex[(n & 0x0F) as usize],
                 b';'];
    writer.write_all(&bytes)
}

fn decode_numeric(esc: &str, radix: u32) -> Result<char> {
    match u32::from_str_radix(esc, radix) {
        Ok(n) => match char::from_u32(n) {
            Some(c) => Ok(c),
            None => bail!(ErrorKind::InvalidCharacter)
        },
        Err(..) => bail!(ErrorKind::MalformedNumEscape)
    }
}

fn is_ascii_alnum(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')
}

fn is_digit(c: char) -> bool { c >= '0' && c <= '9' }

fn is_hex_digit(c: char) -> bool {
    is_digit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_doc_filename() {
        let method_name = &"the-unescaped_method%name";
        let encoded = encode_doc_filename(method_name).unwrap();
        assert_eq!(encoded, "the%x2D;unescaped%x5F;method%x25;name");
    }

    #[test]
    fn test_decode_doc_filename() {
        let encoded_name = &"the%x2D;escaped%x5F;method%x25;name";
        let decoded = decode_doc_filename(encoded_name).unwrap();
        assert_eq!(decoded, "the-escaped_method%name");
    }

    #[test]
    #[should_panic]
    fn test_invalid_hex_sequence() {
        let encoded_name = &"bad%xZZ;name";
        let decoded = encode_doc_filename(encoded_name).err().unwrap();
    }

    #[test]
    #[should_panic]
    fn test_premature_end() {
        let encoded_name = &"bad%xAAname";
        let err = encode_doc_filename(encoded_name).err().unwrap();
    }
}
