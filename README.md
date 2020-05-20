# Tether
Tether is a simple tool that allows UPNP port-forwarding to be scripted.
```
tether 1.0.0
by77er@github
A scriptable tool for easy NAT port forwarding via UPNP

USAGE:
    tether.exe [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -t, --timeout <timeout>    UPNP device search timeout (seconds)

SUBCOMMANDS:
    add       Adds a UPNP port mapping
    help      Prints this message or the help of the given subcommand(s)
    remove    Removes one or multiple UPNP port mappings
    view      Lists UPNP port mappings already present on the device
```