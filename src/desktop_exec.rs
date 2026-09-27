//! Desktop-entry Exec parsing. This is not shell syntax: arguments are passed
//! directly to the program, and field substitutions are never parsed again.

pub fn parse(
    exec: &str,
    name: &str,
    icon: Option<&str>,
    path: &str,
) -> Result<Vec<String>, String> {
    // Desktop string escaping precedes Exec quoting (four backslashes in a
    // desktop file represent one literal backslash in a quoted argument).
    let mut decoded = String::new();
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            decoded.push(c);
            continue;
        }
        let next = chars.next().ok_or("Trailing escape in Exec")?;
        match next {
            's' => decoded.push(' '),
            'n' => decoded.push('\n'),
            't' => decoded.push('\t'),
            'r' => decoded.push('\r'),
            '\\' => decoded.push('\\'),
            _ => {
                decoded.push('\\');
                decoded.push(next);
            }
        }
    }
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = decoded.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => {
                let next = chars
                    .next()
                    .ok_or("Trailing escape in quoted Exec argument")?;
                if !matches!(next, '"' | '`' | '$' | '\\') {
                    return Err("Invalid Exec escape".into());
                }
                token.push(next);
                started = true;
            }
            c if c.is_ascii_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut token));
                    started = false;
                }
            }
            c => {
                token.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err("Unclosed quote in Exec".into());
    }
    if started {
        tokens.push(token);
    }
    let mut args = Vec::new();
    for token in tokens {
        if token == "%i" {
            if let Some(icon) = icon.filter(|s| !s.is_empty()) {
                args.extend(["--icon".into(), icon.to_string()]);
            }
            continue;
        }
        let mut arg = String::new();
        let mut chars = token.chars();
        let mut removed = false;
        while let Some(c) = chars.next() {
            if c != '%' {
                arg.push(c);
                continue;
            }
            match chars.next().ok_or("Trailing field code in Exec")? {
                '%' => arg.push('%'),
                'c' => arg.push_str(name),
                'k' => arg.push_str(path),
                'f' | 'u' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm' => removed = true,
                'F' | 'U' if token.len() == 2 => removed = true,
                code => return Err(format!("Invalid or misplaced Exec field code %{code}")),
            }
        }
        if !arg.is_empty() || !removed {
            args.push(arg);
        }
    }
    if args.first().is_none_or(|s| s.is_empty() || s.contains('=')) {
        return Err("Exec must begin with a nonempty executable".into());
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_executable_and_empty_arguments() {
        assert_eq!(
            parse(r#""/opt/My App/run" "hello  world" "" %U"#, "", None, "").unwrap(),
            ["/opt/My App/run", "hello  world", ""]
        );
    }
    #[test]
    fn fields_expand_once_without_splitting() {
        assert_eq!(
            parse(
                "app %c %i %k %% %f %F %u %U %d %D %n %N %v %m",
                "Name %f",
                Some("my icon"),
                "/my app.desktop"
            )
            .unwrap(),
            [
                "app",
                "Name %f",
                "--icon",
                "my icon",
                "/my app.desktop",
                "%"
            ]
        );
    }
    #[test]
    fn quoted_shell_command_remains_one_argument() {
        assert_eq!(
            parse(r#"sh -c "echo hello; echo world""#, "", None, "").unwrap(),
            ["sh", "-c", "echo hello; echo world"]
        );
    }
    #[test]
    fn desktop_and_exec_escapes_are_decoded_in_order() {
        assert_eq!(
            parse(r#"app "a\\\\b" "\\$HOME" "a\\"b""#, "", None, "").unwrap(),
            ["app", r"a\b", "$HOME", "a\"b"]
        );
    }
    #[test]
    fn rejects_invalid_commands() {
        for input in [
            "",
            "%f",
            "app %Z",
            "app %",
            "app a%F",
            "app a%i",
            "app \"oops",
            "A=B app",
        ] {
            assert!(parse(input, "", None, "").is_err(), "{input}");
        }
    }
    #[test]
    fn shell_metacharacters_are_literal_arguments() {
        assert_eq!(
            parse(r#"app "$(touch /tmp/never)""#, "", None, "").unwrap(),
            ["app", "$(touch /tmp/never)"]
        );
    }
}
