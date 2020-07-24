use clap::{App, Arg};

fn main() {
/*
-d --database <dbname>    select a database to search
-s --strategy <strategy>  strategy for matching or defining
-c --config <file>        specify configuration file
-C --nocorrect            disable attempted spelling correction
-D --dbs                  show available databases
-S --strats               show available search strategies
-H --serverhelp           show server help
-i --info <dbname>        show information about a database
-I --serverinfo           show information about the server
-a --noauth               disable authentication
-u --user <username>      username for authentication
-k --key <key>            shared secret for authentication
*/
    let _matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author("Jan Schreiber <jan@mecinus.com>")
        .about("Dictionary query client")
        .arg(Arg::with_name("license")
             .long("license")
             .short("L")
             .help("display copyright and license information"))
        .arg(Arg::with_name("host")
             .long("host")
             .short("h")
             .value_name("host")
             .help("specify server")
             .takes_value(true))
        .arg(Arg::with_name("port")
             .long("port")
             .short("p")
             .value_name("port")
             .help("specify port")
             .takes_value(true))
        .arg(Arg::with_name("match")
             .long("match")
             .short("m")
             .help("match instead of define"))
        .get_matches();
    println!("Not implemented!");
}
