use std::env;
use self_update;
use multirust::Result;
use clap::*;

pub fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    let arg1 = args.get(1).map(|a| &**a);

    // Secret command used during self-update. Not for users.
    if arg1 == Some("--self-replace") {
        return self_update::self_replace();
    }

    let cli = App::new("multirust-setup")
        .version("0.0.5")
        .author("Diggory Blake")
        .about("The installer for multirust")
        .arg(Arg::with_name("verbose")
             .short("v")
             .long("verbose")
             .help("Enable verbose output"))
        .arg(Arg::with_name("no-prompt")
             .short("y")
             .help("Disable confirmation prompt."));

    let matches = cli.get_matches();
    let no_prompt = matches.is_present("no-prompt");
    let verbose = matches.is_present("verbose");

    try!(self_update::install(no_prompt, verbose));

    Ok(())
}
