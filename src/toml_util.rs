use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use toml::Value;

use ::errors::*;

pub fn toml_string_from_file<T: AsRef<Path>>(path: T) -> Result<String> {
    let path_as = path.as_ref();
    let mut file = match File::open(path_as) {
        Ok(f) => f,
        Err(..) => bail!("Cannot open TOML file {}", path_as.display()),
    };
    let mut data = String::new();
    file.read_to_string(&mut data)
        .chain_err(|| format!("Can't read TOML file! {}", path_as.display()))?;
    Ok(data)
}

pub fn toml_value_from_string(data: &str) -> Result<Value> {
    data.parse::<Value>().chain_err(|| "Invalid TOML!")
}

pub fn toml_value_from_file<T: AsRef<Path>>(path: T) -> Result<Value> {
    let toml_str = toml_string_from_file(path)?;
    toml_value_from_string(&toml_str)
}

pub fn get_value_in_table<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    match *value {
        Value::Table(ref table) => {
            // 'toml' just panics upon reading an invalid key index, so this is needed.
            if !table.contains_key(key) {
                bail!("TOML Table did not contain key {}", key);
            } else {
                Ok(&table[key])
            }
        }
        _ => bail!("TOML value was not table!")
    }
}

/// Gets the value of the key in the given TOML table.
pub fn get_toml_value<T: Deserialize>(value: &Value, table_name: &str, key: &str) -> Result<T> {
    match get_value_in_table(value, table_name) {
        Ok(table) => match get_value_in_table(&table, key) {
            Ok(val) => val.clone().try_into::<T>().chain_err(|| format!("Could not parse value of {} in TOML table {}", key, table_name)),
            Err(_) => bail!("No such key {} in TOML table {}", key, table_name),
        },
        Err(_) => bail!("TOML table {} does not exist", table_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get() {
        let val = toml_value_from_string("
[table]
thing=1
dood=true").unwrap();
        let res: Result<i32> = get_toml_value(&val, "table", "thing");
        assert!(res.is_ok());
        let res: Result<bool> = get_toml_value(&val, "table", "none");
        assert!(res.is_err());
        let res: Result<i32> = get_toml_value(&val, "whee", "thing");
        assert!(res.is_err());
        let res: Result<bool> = get_toml_value(&val, "whee", "none");
        assert!(res.is_err());
    }
}
