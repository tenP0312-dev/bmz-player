//! The examples of spec chapter 8 (`docs/bmsir/SKIN_BEHAVIOR_SPEC.md`), run
//! against the skin sandbox. They need no skin assets.
//!
//! Not asserted on purpose: the `..` operator on floats (the VM converts the
//! number itself), Lua 5.4-only functions (left available) and `os.date`
//! formats LuaJ rejects (accepted here), `os.date("!*t")` (a table here) and
//! integer `x % 0` (an error here, `nan` in LuaJ).

use super::*;

fn luaj_vm() -> (Lua, crate::tests::TestDir) {
    let dir = crate::tests::TestDir::created("luaj-compat");
    let entry = dir.join("skin.luaskin");
    fs::write(&entry, "return {}").unwrap();
    let context = SkinPathContext::new(&entry, [dir.to_path_buf()]).unwrap();
    let lua = Lua::new();
    install_sandbox(
        &lua,
        &context,
        &BTreeMap::new(),
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        &BTreeMap::new(),
        None,
    )
    .unwrap();
    (lua, dir)
}

/// Evaluates `expr` and returns `tostring` of it, or `error` when it raises.
fn show(lua: &Lua, expr: &str) -> String {
    let chunk = format!(
        "local ok, v = pcall(function() return {expr} end) return ok and tostring(v) or 'error'"
    );
    lua.load(&chunk).eval::<String>().unwrap()
}

#[test]
fn tostring_follows_luaj_number_formatting() {
    let (lua, _dir) = luaj_vm();
    for (expr, expected) in [
        ("3", "3"),
        ("3.0", "3"),
        ("10/2", "5"),
        ("7/2", "3.5"),
        ("-7/2", "-3.5"),
        ("1/3", "0.33333334"),
        ("100/3", "33.333332"),
        ("123456.789", "123456.79"),
        ("12345678.9", "1.2345679E7"),
        ("0.000123", "1.23E-4"),
        ("0.001", "0.001"),
        ("1.1+2.2", "3.3"),
        ("math.pi", "3.1415927"),
        ("2^31", "2147483648"),
        ("2^53", "9007199254740992"),
        ("2^63", "9223372036854775807"),
        ("1e20", "1.0E20"),
        ("10^100", "Infinity"),
        ("-0.0", "0"),
        ("0/0", "nan"),
        ("1/0", "inf"),
        ("-1/0", "-inf"),
        (r#"table.concat({1.5, 2.0, 3}, "|")"#, "1.5|2|3"),
        (r#"("%s|%s"):format(1.5, 2.0)"#, "1.5|2"),
    ] {
        assert_eq!(show(&lua, expr), expected, "tostring({expr})");
    }
}

#[test]
fn string_format_follows_luaj() {
    let (lua, _dir) = luaj_vm();
    for (expr, expected) in [
        (r#"("%d"):format(3)"#, "3"),
        (r#"("%d"):format(3.0)"#, "3"),
        (r#"("%d"):format(3.7)"#, "3"),
        (r#"("%d"):format(-3.7)"#, "-3"),
        (r#"("%d"):format(0.99999)"#, "0"),
        (r#"("%d"):format("12")"#, "12"),
        (r#"("%d"):format("3.5")"#, "3"),
        (r#"("%d"):format(2^31)"#, "2147483648"),
        (r#"("%d"):format(1e20)"#, "9223372036854775807"),
        (r#"("%d"):format(nil)"#, "error"),
        (r#"("%d"):format(true)"#, "error"),
        (r#"("%d"):format("x")"#, "error"),
        (r#"("%03d"):format(7)"#, "007"),
        (r#"("%-5d"):format(3)"#, "3    "),
        (r#"("%+d"):format(3)"#, "+3"),
        (r#"("%x"):format(255.5)"#, "ff"),
        (r#"("%5.2f"):format(3.14159)"#, " 3.14"),
        (r#"("%.0f"):format(0.5)"#, "1"),
        (r#"("%.0f"):format(2.5)"#, "3"),
        (r#"("%s"):format(3.0)"#, "3"),
        (r#"("%s"):format(7/2)"#, "3.5"),
        (r#"("%s"):format(nil)"#, "error"),
        (r#"("%s"):format({})"#, "error"),
        (r#"("%i"):format(3)"#, "3"),
        (r#"("%u"):format(3)"#, "3"),
        (r#"("%e"):format(12345.678)"#, "1.234568e+04"),
        (r#"("%c"):format(65)"#, "A"),
        (r#"("%o"):format(8)"#, "10"),
        (r#"("%X"):format(255)"#, "FF"),
        (r#"("%a"):format(1)"#, "error"),
    ] {
        assert_eq!(show(&lua, expr), expected, "{expr}");
    }
}

#[test]
fn os_date_follows_luaj() {
    let (lua, _dir) = luaj_vm();
    for (expr, expected) in [
        (r#"os.date("!%Y-%m-%d %H:%M:%S", 0)"#, "1970-01-01 00:00:00"),
        (r#"os.date("!%c", 0)"#, "Thu Jan 01 00:00:00 1970"),
        (r#"os.date("!%A %B %p %j %w %y", 1767539045)"#, "Sunday January PM 004 0 26"),
        (r#"os.date("!%x %X %z", 1767539045)"#, "01/04/26 15:04:05 +0000"),
        (r#"os.date("!%U|%W", 1767539045)"#, "1|53"),
        (r#"os.date("!%Y", 0.9)"#, "1970"),
        (r#"os.date("*t", 0).isdst"#, "false"),
        (r#"os.execute"#, "nil"),
        (r#"os.getenv"#, "nil"),
        (r#"os.exit"#, "nil"),
    ] {
        assert_eq!(show(&lua, expr), expected, "{expr}");
    }
}

#[test]
fn global_functions_follow_luaj_availability() {
    let (lua, _dir) = luaj_vm();
    for (expr, expected) in [
        ("loadstring", "nil"),
        ("setfenv", "nil"),
        ("unpack", "nil"),
        ("type(table.unpack)", "function"),
        ("table.getn", "nil"),
        ("type(math.pow)", "function"),
        ("type(math.atan2)", "function"),
        ("math.log(8, 2)", "3"),
        ("math.fmod(-7, 3)", "-1"),
        ("7 % 3 .. ''", "1"),
        ("-7 % 3", "2"),
        ("math.floor(-3.5)", "-4"),
        ("math.atan(1, 1) == math.atan2(1, 1)", "true"),
    ] {
        assert_eq!(show(&lua, expr), expected, "{expr}");
    }
}
