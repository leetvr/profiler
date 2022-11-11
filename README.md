# Profiler
> A really bad profiler for the Quest 2

![image](https://user-images.githubusercontent.com/2022375/201298536-8561ed26-43a1-41f5-8c53-2b842f7e352e.png)


## Getting started
1. Install `redis`, `rust`, `node` and `npm`.
1. Have your Quest 2 plugged in, preferably with the proximity sensor turned off.
1. Do a profiling run with `cargo run --bin client` (`RUST_LOG=debug` might be helpful. Also everything is hard coded to run The Station, so you'll want to change that. Sorry!)
1. Start the server with `cargo run --bin server`
1. Start the app with `cd app && npm install && npm start`
