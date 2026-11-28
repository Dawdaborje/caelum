// SPDX-License-Identifier: GPL-3.0-or-later
//! `caelum`: talks to the running shell over D-Bus.
use std::process::ExitCode;

const USAGE: &str =
    "usage: caelum (toggle|open|close) <launcher|session|dashboard|sidebar>\n       caelum reload";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (method, drawer) = match args.as_slice() {
        ["toggle", d] => ("Toggle", Some(*d)),
        ["open", d] => ("Open", Some(*d)),
        ["close", d] => ("Close", Some(*d)),
        ["reload"] => ("Reload", None),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let result = rt.block_on(async {
        let conn = zbus::Connection::session().await?;
        let (dest, path, iface) = ("org.caelum.Shell", "/org/caelum/Shell", "org.caelum.Shell1");
        match drawer {
            Some(d) => {
                conn.call_method(Some(dest), path, Some(iface), method, &(d,))
                    .await
            }
            None => {
                conn.call_method(Some(dest), path, Some(iface), method, &())
                    .await
            }
        }
    });
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(zbus::Error::MethodError(_, Some(msg), _)) => {
            eprintln!("caelum: {msg}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("caelum: {e}");
            ExitCode::FAILURE
        }
    }
}
