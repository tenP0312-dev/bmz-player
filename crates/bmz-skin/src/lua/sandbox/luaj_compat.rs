//! LuaJ compatibility layer for skin Lua (spec chapter 8, L-1).
//!
//! BMZ runs skins on Lua 5.4 (mlua); the original players run LuaJ 3.0.2
//! (Lua 5.2 semantics, every number a double). A skin written for the original
//! players must see the same values here, so the differences that skins can
//! observe are wrapped instead of changing the skin:
//!
//! * `tostring` (and `%s`, `table.concat`) write numbers the LuaJ way.
//! * `string.format` converts `%d` / `%x` / ... arguments by truncating toward
//!   zero, rounds `%f` half up and follows the other LuaJ quirks.
//!
//! The `..` operator on numbers is converted inside the Lua VM and cannot be
//! wrapped; `1.5 .. 2.0` stays `1.52.0` here. Functions that exist only in
//! Lua 5.4 are left available (a superset is harmless for skins that work in
//! LuaJ).

use super::*;

#[cfg(test)]
#[path = "luaj_compat_tests.rs"]
mod tests;

/// Writes a number the way LuaJ does (spec 8.2).
pub(super) fn luaj_number_string(number: f64) -> String {
    if number.is_nan() {
        return "nan".to_string();
    }
    if number.is_infinite() {
        return if number > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    // LuaDouble.valueOf: a double equal to its saturated 64-bit integer is an integer.
    let truncated = number as i64;
    if truncated as f64 == number {
        return truncated.to_string();
    }
    java_float_string(number as f32)
}

/// `Float.toString`: shortest digits, positional between 1e-3 and 1e7.
fn java_float_string(value: f32) -> String {
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value == 0.0 {
        return "0.0".to_string();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if (-3..7).contains(&exponent) {
        if exponent >= 0 {
            let split = exponent as usize + 1;
            let mut integer = digits.chars().take(split).collect::<String>();
            while integer.len() < split {
                integer.push('0');
            }
            let fraction: String = digits.chars().skip(split).collect();
            let fraction = if fraction.is_empty() { "0".to_string() } else { fraction };
            format!("{sign}{integer}.{fraction}")
        } else {
            format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize))
        }
    } else {
        let mut chars = digits.chars();
        let first = chars.next().unwrap_or('0');
        let rest: String = chars.collect();
        let rest = if rest.is_empty() { "0".to_string() } else { rest };
        format!("{sign}{first}.{rest}E{exponent}")
    }
}

/// Rounds `value` to `precision` decimals, ties away from zero, using the
/// shortest decimal text of the double (what Java's formatter sees).
fn round_half_up(value: f64, precision: usize) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let text = format!("{}", value.abs());
    let (integer, fraction) = text.split_once('.').unwrap_or((&text, ""));
    if fraction.len() <= precision {
        return value;
    }
    let round_up = fraction.as_bytes()[precision] >= b'5';
    let mut digits: Vec<u8> = integer.bytes().chain(fraction.bytes().take(precision)).collect();
    if round_up {
        let mut index = digits.len();
        loop {
            if index == 0 {
                digits.insert(0, b'1');
                break;
            }
            index -= 1;
            if digits[index] == b'9' {
                digits[index] = b'0';
            } else {
                digits[index] += 1;
                break;
            }
        }
    }
    let integer_len = digits.len() - precision;
    let mut rounded = String::from_utf8_lossy(&digits[..integer_len]).into_owned();
    if precision > 0 {
        rounded.push('.');
        rounded.push_str(&String::from_utf8_lossy(&digits[integer_len..]));
    }
    let parsed = rounded.parse::<f64>().unwrap_or(value.abs());
    if value.is_sign_negative() { -parsed } else { parsed }
}

fn luaj_tostring_value(value: &Value, original: &Function) -> mlua::Result<String> {
    match value {
        Value::Integer(integer) => Ok(integer.to_string()),
        Value::Number(number) => Ok(luaj_number_string(*number)),
        other => original.call::<mlua::String>(other.clone()).map(|s| s.to_string_lossy()),
    }
}

/// LuaJ `checkdouble` / `checklong`: numbers and numeric strings only.
fn luaj_arg_number(lua: &Lua, args: &[Value], index: usize) -> mlua::Result<f64> {
    let missing = || {
        mlua::Error::runtime(format!("bad argument #{} to 'format' (number expected)", index + 1))
    };
    let value = args.get(index).ok_or_else(missing)?;
    match value {
        Value::Integer(integer) => Ok(*integer as f64),
        Value::Number(number) => Ok(*number),
        Value::String(_) => lua.coerce_number(value.clone())?.ok_or_else(missing),
        _ => Err(missing()),
    }
}

fn luaj_arg_integer(lua: &Lua, args: &[Value], index: usize) -> mlua::Result<i64> {
    if let Some(Value::Integer(integer)) = args.get(index) {
        return Ok(*integer);
    }
    // Rust's float-to-integer cast truncates toward zero and saturates.
    Ok(luaj_arg_number(lua, args, index)? as i64)
}

struct FormatSpec<'a> {
    flags: &'a str,
    width: &'a str,
    precision: Option<&'a str>,
    conversion: u8,
}

fn parse_format(format: &[u8]) -> mlua::Result<Vec<Result<&[u8], FormatSpec<'_>>>> {
    // Ok(literal) or Err(spec) pieces.
    let mut pieces = Vec::new();
    let mut index = 0;
    let mut literal_start = 0;
    while index < format.len() {
        if format[index] != b'%' {
            index += 1;
            continue;
        }
        if format.get(index + 1) == Some(&b'%') {
            pieces.push(Ok(&format[literal_start..=index]));
            index += 2;
            literal_start = index;
            continue;
        }
        pieces.push(Ok(&format[literal_start..index]));
        let start = index + 1;
        let mut cursor = start;
        while matches!(format.get(cursor), Some(b'-' | b'+' | b' ' | b'#' | b'0')) {
            cursor += 1;
        }
        let flags_end = cursor;
        while matches!(format.get(cursor), Some(b'0'..=b'9')) {
            cursor += 1;
        }
        let width_end = cursor;
        let mut precision = None;
        if format.get(cursor) == Some(&b'.') {
            let precision_start = cursor + 1;
            cursor = precision_start;
            while matches!(format.get(cursor), Some(b'0'..=b'9')) {
                cursor += 1;
            }
            precision = Some(std::str::from_utf8(&format[precision_start..cursor]).unwrap_or(""));
        }
        let Some(&conversion) = format.get(cursor) else {
            return Err(mlua::Error::runtime("invalid option '%' to 'format'"));
        };
        pieces.push(Err(FormatSpec {
            flags: std::str::from_utf8(&format[start..flags_end]).unwrap_or(""),
            width: std::str::from_utf8(&format[flags_end..width_end]).unwrap_or(""),
            precision,
            conversion,
        }));
        index = cursor + 1;
        literal_start = index;
    }
    pieces.push(Ok(&format[literal_start..]));
    Ok(pieces)
}

fn luaj_format(
    lua: &Lua,
    original_format: &Function,
    original_tostring: &Function,
    args: Variadic<Value>,
) -> mlua::Result<mlua::String> {
    let args: Vec<Value> = args.into_iter().collect();
    let format_text = match args.first() {
        Some(Value::String(text)) => text.as_bytes().to_vec(),
        Some(value @ (Value::Integer(_) | Value::Number(_))) => {
            luaj_tostring_value(value, original_tostring)?.into_bytes()
        }
        _ => {
            return Err(mlua::Error::runtime("bad argument #1 to 'format' (string expected)"));
        }
    };
    let mut output: Vec<u8> = Vec::new();
    let mut next_arg = 1;
    for piece in parse_format(&format_text)? {
        let spec = match piece {
            Ok(literal) => {
                output.extend_from_slice(literal);
                continue;
            }
            Err(spec) => spec,
        };
        let index = next_arg;
        next_arg += 1;
        let precision = spec.precision.map(|p| format!(".{p}")).unwrap_or_default();
        let (pattern, value): (String, Value) = match spec.conversion {
            b'd' | b'i' | b'u' => (
                format!("%{}{}{}d", spec.flags, spec.width, precision),
                Value::Integer(luaj_arg_integer(lua, &args, index)?),
            ),
            b'x' | b'X' | b'o' | b'c' => (
                format!("%{}{}{}{}", spec.flags, spec.width, precision, spec.conversion as char),
                Value::Integer(luaj_arg_integer(lua, &args, index)?),
            ),
            b'f' | b'F' | b'g' | b'G' => {
                // LuaJ's %g prints like %f; ties of %f round up.
                let number = luaj_arg_number(lua, &args, index)?;
                let digits = spec.precision.map_or(6, |p| p.parse::<usize>().unwrap_or(0));
                (
                    format!("%{}{}{}f", spec.flags, spec.width, precision),
                    Value::Number(round_half_up(number, digits)),
                )
            }
            b'e' | b'E' => (
                format!("%{}{}{}{}", spec.flags, spec.width, precision, spec.conversion as char),
                Value::Number(luaj_arg_number(lua, &args, index)?),
            ),
            b's' => {
                // Width and precision are ignored; only strings and numbers pass.
                let text = match args.get(index) {
                    Some(Value::String(text)) => text.as_bytes().to_vec(),
                    Some(value @ (Value::Integer(_) | Value::Number(_))) => {
                        luaj_tostring_value(value, original_tostring)?.into_bytes()
                    }
                    _ => {
                        return Err(mlua::Error::runtime(format!(
                            "bad argument #{} to 'format' (string expected)",
                            index + 1
                        )));
                    }
                };
                output.extend_from_slice(&text);
                continue;
            }
            b'q' => {
                let value = args.get(index).cloned().unwrap_or(Value::Nil);
                ("%q".to_string(), value)
            }
            other => {
                return Err(mlua::Error::runtime(format!(
                    "invalid option '%{}' to 'format'",
                    other as char
                )));
            }
        };
        let piece: mlua::String = original_format.call((pattern, value))?;
        output.extend_from_slice(&piece.as_bytes());
    }
    lua.create_string(&output)
}

pub(super) fn install_luaj_compat(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();
    let original_tostring: Function = globals.get("tostring")?;
    let string: Table = globals.get("string")?;
    let original_format: Function = string.get("format")?;
    let table: Table = globals.get("table")?;
    let original_concat: Function = table.get("concat")?;

    let stock_tostring_key = lua.create_registry_value(original_tostring.clone())?;
    let tostring_key = lua.create_registry_value(original_tostring)?;
    let tostring = lua.create_function({
        let key = tostring_key;
        move |lua, value: Value| {
            let original: Function = lua.registry_value(&key)?;
            match value {
                Value::Integer(_) | Value::Number(_) => {
                    Ok(Value::String(lua.create_string(luaj_tostring_value(&value, &original)?)?))
                }
                other => original.call::<Value>(other),
            }
        }
    })?;
    globals.set("tostring", tostring)?;

    let format_key = lua.create_registry_value(original_format)?;
    string.set(
        "format",
        lua.create_function(move |lua, args: Variadic<Value>| {
            let original_format: Function = lua.registry_value(&format_key)?;
            let stock_tostring: Function = lua.registry_value(&stock_tostring_key)?;
            luaj_format(lua, &original_format, &stock_tostring, args)
        })?,
    )?;

    let concat_key = lua.create_registry_value(original_concat)?;
    table.set(
        "concat",
        lua.create_function(move |lua, args: Variadic<Value>| {
            let original: Function = lua.registry_value(&concat_key)?;
            let mut args: Vec<Value> = args.into_iter().collect();
            if let Some(Value::Table(list)) = args.first() {
                let start = match args.get(2) {
                    Some(Value::Integer(i)) => *i,
                    Some(Value::Number(n)) => *n as i64,
                    _ => 1,
                };
                let end = match args.get(3) {
                    Some(Value::Integer(i)) => *i,
                    Some(Value::Number(n)) => *n as i64,
                    _ => list.raw_len() as i64,
                };
                if end >= start
                    && end - start < 1_000_000
                    && (start..=end)
                        .any(|i| matches!(list.raw_get::<Value>(i), Ok(Value::Number(_))))
                {
                    let copy = lua.create_table()?;
                    for index in start..=end {
                        let value: Value = list.raw_get(index)?;
                        let value = match value {
                            Value::Number(number) => {
                                Value::String(lua.create_string(luaj_number_string(number))?)
                            }
                            other => other,
                        };
                        copy.raw_set(index, value)?;
                    }
                    args[0] = Value::Table(copy);
                }
            }
            original.call::<Value>(Variadic::from_iter(args))
        })?,
    )?;
    Ok(())
}
