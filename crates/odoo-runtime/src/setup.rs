//! One-time preparation of an Odoo version: the source code and a Python
//! virtualenv with its requirements. Everything lives in the runtime folder,
//! independent of any developer setup on the machine.

/// Odoo's `requirements.txt`, adjusted so it installs from pre-built wheels on a
/// machine without compilers or system development libraries:
/// - `psycopg2` becomes `psycopg2-binary` (same version, no PostgreSQL headers needed);
/// - `python-ldap` is dropped (needs LDAP system libraries; LDAP login is not used).
///
/// Version pins and environment markers are kept as they are.
pub fn adjust_requirements(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| package_name(line) != Some("python-ldap"))
        .map(|line| match package_name(line) {
            Some("psycopg2") => line.replacen("psycopg2", "psycopg2-binary", 1),
            _ => line.to_string(),
        })
        .collect()
}

/// The package a requirement line installs (lowercase), e.g. `psycopg2` for
/// `psycopg2==2.9.9 ; marker`. `None` for comments and blank lines.
fn package_name(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let end = line
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
        .unwrap_or(line.len());
    Some(&line[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psycopg2_uses_the_binary_wheel_with_the_same_pin_and_marker() {
        let out = adjust_requirements(
            "psycopg2==2.9.9 ; python_version >= '3.12' and python_version < '3.13' # (Noble)\n",
        );
        assert_eq!(
            out,
            "psycopg2-binary==2.9.9 ; python_version >= '3.12' and python_version < '3.13' # (Noble)\n"
        );
    }

    #[test]
    fn python_ldap_is_dropped() {
        let out = adjust_requirements(
            "lxml==5.2.1\npython-ldap==3.4.4 ; sys_platform != 'win32' and python_version >= '3.12'\nPillow==10.2.0\n",
        );
        assert_eq!(out, "lxml==5.2.1\nPillow==10.2.0\n");
    }

    #[test]
    fn everything_else_is_untouched() {
        let text = "# comment\n\ngevent==24.2.1 ; sys_platform != 'win32'\nreportlab==4.1.0\n";
        assert_eq!(adjust_requirements(text), text);
    }

    #[test]
    fn already_binary_psycopg2_is_left_alone() {
        let text = "psycopg2-binary==2.9.9\n";
        assert_eq!(adjust_requirements(text), text);
    }
}
