# dictr

A rust implementation of RFC2229

To run the client:

    cargo run --bin dictr (not implemented)

To run th server:

    cargo run --bin dictrd

Default port for testing is 2628 (will become 2628).

    telnet localhost 2628

Run tests:

    RUST_BACKTRACE=1 RUST_LOG=yourlogger=debug cargo test  -- --nocapture


Currently work in progress(tm).
Implemented commands:

 - DEFINE database word         -- look up word in database
 - MATCH database strategy word -- match word in database using strategy
 - SHOW DB                      -- list all accessible databases
 - SHOW DATABASES               -- list all accessible databases
 - SHOW STRAT                   -- list available matching strategies
 - SHOW STRATEGIES              -- list available matching strategies
 - SHOW INFO database           -- provide information about the database
 - SHOW SERVER                  -- provide site-specific information
 - HELP                         -- display this help information
 - XRANDOM                      -- return a random definition
 - QUIT                         -- terminate connection

Not implemented:
 - OPTION MIME                  -- use MIME headers
 - STATUS                       -- display timing information
 - DEFINE will only work on the first dictionary. This will be fixed soon.
 - MATCH ! is not implemented (only * and DICTNAME)
 - No auth implemented, but this is not required by RFC2229.
