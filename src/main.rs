#[rustfmt::skip]
fn main() { match wtw::run_cli_env() { Ok(code) => std::process::exit(code), Err(error) => { eprintln!("{error:#}"); std::process::exit(2); } } }
