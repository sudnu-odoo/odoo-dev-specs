//! Run the setup's checks on this machine and print what each found, for support:
//! `cargo run -p odoo-runtime --example doctor`. With `--script <user>` it prints the
//! fix script the app would run as root; with `--repair <user>`, PostgreSQL's repair.

use odoo_runtime::system::{plan, probe, repair_script, report, script};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let user = args.get(2).map(String::as_str).unwrap_or("nobody");
    match args.get(1).map(String::as_str) {
        Some("--script") => print!("{}", script(&plan(&probe()), user)),
        Some("--repair") => print!("{}", repair_script(user)),
        _ => {
            for r in report(&probe()).requirements {
                match r.cause {
                    None => println!("ok    {}", r.label),
                    Some(cause) => println!("FAIL  {}  {cause:?}  {:?}", r.label, r.fix),
                }
            }
        }
    }
}
