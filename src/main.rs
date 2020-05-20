use clap::{load_yaml, App, ArgMatches};
use igd::{
    search_gateway, Gateway, GetGenericPortMappingEntryError, PortMappingEntry,
    PortMappingProtocol, SearchOptions,
};

use core::time::Duration;
use std::net;
use std::process;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let args = App::from_yaml(yaml).get_matches();

    // Let the user choose a comfortable timeout
    let timeout: u64 = args
        .value_of("timeout")
        .unwrap_or("60")
        .parse()
        .unwrap_or_else(|_e| fail("Invalid timeout. Please choose an integer > 0."));

    let options = SearchOptions {
        timeout: Some(Duration::from_secs(timeout)),
        ..Default::default()
    };

    // Scan the network for the gateway
    let mut gateway =
        search_gateway(options).unwrap_or_else(|_e| fail("Failed to find UPNP-enabled gateway."));

    // Dispatch the appropriate subroutine
    match args.subcommand() {
        ("view", Some(matches)) => {
            view(&gateway, &matches, true);
        }
        ("add", Some(matches)) => add(&mut gateway, &matches),
        ("remove", Some(matches)) => remove(&mut gateway, &matches),
        _ => fail(args.usage()),
    }
}

// Supports the "view" subcommand functionality - "print" argument allows it to be used by other fns
fn view(gateway: &Gateway, matches: &ArgMatches, print_enable: bool) -> Vec<PortMappingEntry> {
    if matches.is_present("print_ip") {
        let ip = &gateway
            .get_external_ip()
            .unwrap_or_else(|_e| fail("Failed to get external IP address."));
        println!("Internal IP: {} | External IP: {}", &gateway.addr.ip(), ip);
    }

    if print_enable {
        println!(
            "{: <15} || {: <15} || {: <30} // {}",
            "Remote Host", "External Port", "Internal Host", "Description"
        );
    }

    let mut ports: Vec<PortMappingEntry> = Vec::new();

    // Print out all port mapping entries
    for index in 0..u32::MAX {
        match gateway.get_generic_port_mapping_entry(index) {
            Ok(entry) => {
                // Print port entry
                if print_enable {
                    println!(
                        "{: <15} -> {: <15} -> {: <30} // {}",
                        if entry.remote_host.len() > 0 {
                            &entry.remote_host
                        } else {
                            "*"
                        },
                        format!("{}/{}", entry.external_port, entry.protocol),
                        format!(
                            "{}:{}/{}",
                            entry.internal_client, entry.internal_port, entry.protocol
                        ),
                        entry.port_mapping_description
                    );
                }
                ports.push(entry);
            }
            Err(error) => match error {
                // Hit the end of the host list - stop printing and exit.
                GetGenericPortMappingEntryError::SpecifiedArrayIndexInvalid => break,
                // Permission issue - print an error and exit
                e => fail(&format!("Error getting port mapping: {}", e)),
            },
        }
    }

    // Return mappings
    ports
}

// Supports the "add" subcommand functionality
fn add(mut gateway: &mut Gateway, matches: &ArgMatches) {
    // So many arguments to match...
    let external_port: u16 = matches
        .value_of("external_port")
        .unwrap()
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid external port. Please choose an integer from 0 to 65535.")
        });

    let internal_port: u16 = matches
        .value_of("internal_port")
        .unwrap()
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid external port. Please choose an integer from 0 to 65535.")
        });

    let internal_host: net::Ipv4Addr = matches
        .value_of("internal_host")
        .unwrap()
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid internal host address. Please specify an IPv4 address.")
        });

    let lease_duration: u32 = match matches.value_of("lease_duration") {
        Some(d) => d
            .parse()
            .unwrap_or_else(|_e| fail("Invalid duration. Choose an integer >= 0")),
        None => 0,
    };

    let description: &str = matches.value_of("description").unwrap_or("");

    let proto_string = match matches.value_of("protocol") {
        Some(s) => s,
        None => fail("Please specify a protocol. TCP, UDP, and BOTH are valid."),
    };

    match proto_string.to_ascii_uppercase().as_str() {
        "UDP" => {
            add_port(
                &mut gateway,
                PortMappingProtocol::UDP,
                external_port,
                net::SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
        }
        "TCP" => {
            add_port(
                &mut gateway,
                PortMappingProtocol::TCP,
                external_port,
                net::SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
        }
        "BOTH" => {
            add_port(
                &mut gateway,
                PortMappingProtocol::UDP,
                external_port,
                net::SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
            add_port(
                &mut gateway,
                PortMappingProtocol::TCP,
                external_port,
                net::SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
        }
        _ => fail("Invalid protocol. TCP, UDP, and BOTH are valid."),
    }
}

fn add_port(
    gateway: &mut Gateway,
    protocol: PortMappingProtocol,
    external_port: u16,
    local_addr: net::SocketAddrV4,
    lease_duration: u32,
    description: &str,
) {
    match gateway.add_port(
        protocol,
        external_port,
        local_addr,
        lease_duration,
        description,
    ) {
        // Hooray, port forwarded
        Ok(()) => println!(
            "Added mapping: {} ext:{} -> {}",
            protocol, external_port, local_addr
        ),
        // Something went wrong
        Err(e) => fail(&format!("Error adding port mapping: {}", e)),
    }
}

// Supports the "remove" subcommand functionality
fn remove(mut gateway: &mut Gateway, matches: &ArgMatches) {
    // Clear case
    if matches.is_present("clear_all") {
        let entries = view(&gateway, &matches, false);
        // Notify when nothing happens
        if entries.len() == 0 {
            println!("No mappings to delete.");
        }
        for entry in entries {
            remove_port(&mut gateway, entry.protocol, entry.external_port);
        }
        // don't try to remove anything else
        return;
    }

    // Normal case
    let external_port: u16 = matches
        .value_of("external_port")
        .unwrap_or_else(|| fail("Please specify an external port."))
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid external port. Please choose an integer from 0 to 65535.")
        });

    // Validate protocol and remove (BOTH will still try to complete the other request if
    // the first one fails)
    let proto_string = match matches.value_of("protocol") {
        Some(s) => s,
        None => fail("Please specify a protocol. TCP, UDP, and BOTH are valid."),
    };
    match proto_string.to_ascii_uppercase().as_str() {
        "UDP" => {
            remove_port(&mut gateway, PortMappingProtocol::UDP, external_port);
        }
        "TCP" => {
            remove_port(&mut gateway, PortMappingProtocol::TCP, external_port);
        }
        "BOTH" => {
            remove_port(&mut gateway, PortMappingProtocol::UDP, external_port);
            remove_port(&mut gateway, PortMappingProtocol::TCP, external_port);
        }
        _ => fail("Invalid protocol. TCP, UDP, and BOTH are valid."),
    }
}

fn remove_port(gateway: &mut Gateway, protocol: PortMappingProtocol, external_port: u16) {
    match gateway.remove_port(protocol, external_port) {
        // Removed, yay
        Ok(()) => println!("Removed {}/{}", external_port, protocol),
        // Unauthorized.
        Err(e) => fail(&format!("Error removing port mapping: {}", e)),
    }
}

// Prints an error message and exits
fn fail(message: &str) -> ! {
    println!("{}", message);
    process::exit(1);
}
