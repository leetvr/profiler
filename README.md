# Profiler
> A really bad profiler for the Quest 2

## Getting started
1. Install `redis`, `rust`, `node` and `npm`.
1. Do a profiling run with `cargo run --bin client` (`RUST_LOG=debug` might be helpful. Also everything is hard coded to run The Station, so you'll want to change that. Sorry!)
1. Start the server with `cargo run --bin server`
1. Start the app with `cd app && npm install && npm start`
