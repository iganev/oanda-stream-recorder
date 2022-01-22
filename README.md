# oanda-stream-recorder
OANDA Pricing Stream Recorder in Rust  

This tool records OANDA's pricing stream in real-time and saves it in JSON files.  
The purpose is to collect data for back-testing and analytics.  
To organize the storage the tool allows the user to specify file naming rules based on date format.  
To minimize the storage the tool allows the user to enable gzip (external command needed) of old files.  

## Build

To build the tool you need to have Rust. Check [Rustup](https://rustup.rs/) for more info.  
Then all you need to do is run cargo:  

```
cargo build --release
```

The resulting binary is located in `target/release`.  

Only UNIX systems are supported at the moment, as compressing old files requires `gzip` command.  

To cross-compile for ARM (Raspberry Pi boards) you can use the docker image `rustembedded/cross`.  

```
cargo install --version 0.1.16 cross

cross build --target armv7-unknown-linux-gnueabihf --release 
```

The resulting binary is located in `target/armv7-unknown-linux-gnueabihf/release`.  

## Running

To get help on how to configure the tool run:  

```
target/release/oanda-stream-recorder -c no
```

After creating an appropriate `config.toml` file you can run:

```
target/release/oanda-stream-recorder -c config.toml
```

It might be useful to `screen` the process:  

```
screen -S oanda-stream-recorder -dm target/release/oanda-stream-recorder -c config.toml
```

Then to retrieve the screened process type:  

```
screen -r oanda-stream-recorder
```

And minimize it again using `CTRL + A D`.  

Another approach would be to create a `systemd` unit file and run it as a service.  
This way your log output would be accessible by `journalctl`.  
